//! 异步浏览器连接与 Tab 管理

use crate::cdp::{AsyncCdpClient, CdpError};
use crate::page::AsyncPage;
use serde_json::json;
use std::process::Child;
use std::sync::Arc;

use super::{BrowserConfig, BrowserVersion, JsonVersion};

/// 异步浏览器实例：连接已有 Chrome，管理 Tab。
pub struct AsyncBrowser {
    client: Arc<AsyncCdpClient>,
    /// 仅 launch 时：子进程句柄，用于 close 时结束。
    child: Option<Child>,
    pub(crate) browser_endpoint: Option<String>,
}

impl AsyncBrowser {
    /// 连接已有 Chrome（需已用 --remote-debugging-port 启动）。
    pub async fn connect(endpoint: &str) -> Result<Self, CdpError> {
        let endpoint = super::normalize_endpoint(endpoint);
        let ws_url = fetch_ws_url_from_endpoint_async(&endpoint).await?;
        let client = AsyncCdpClient::connect(&ws_url).await?;
        Ok(Self {
            client: Arc::new(client),
            child: None,
            browser_endpoint: Some(endpoint),
        })
    }

    /// 启动新的 Chrome 进程并连接。
    pub async fn launch(config: BrowserConfig) -> Result<Self, CdpError> {
        let (endpoint, child) = super::launch_chrome(&config)?;
        let ws_url = fetch_ws_url_from_endpoint_async(&endpoint).await?;
        let client = AsyncCdpClient::connect(&ws_url).await?;
        Ok(Self {
            client: Arc::new(client),
            child: Some(child),
            browser_endpoint: Some(endpoint),
        })
    }

    /// 若指定地址已有浏览器则连接，否则启动新浏览器再连接。
    pub async fn connect_or_launch(config: BrowserConfig) -> Result<Self, CdpError> {
        let address = config
            .get_address()
            .map(String::from)
            .unwrap_or_else(|| format!("127.0.0.1:{}", config.get_remote_debugging_port()));
        let endpoint = format!("http://{}", address);

        if config.get_existing_only() {
            return Self::connect(&endpoint).await;
        }

        let (host, port_str) = address.split_once(':').unwrap_or(("127.0.0.1", "9222"));
        let port: u16 = port_str.parse().unwrap_or(9222);
        let in_use = tokio::net::TcpStream::connect((host, port)).await.is_ok();

        if host != "127.0.0.1" || in_use {
            return Self::connect(&endpoint).await;
        }

        Self::launch(config).await
    }

    /// 关闭浏览器（仅对 launch 的实例有效，会结束 Chrome 进程）。
    pub fn close(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }

    /// 新建一个 Tab（about:blank），返回 AsyncPage。
    pub async fn new_tab(&self) -> Result<AsyncPage, CdpError> {
        let params = json!({ "url": "about:blank" });
        let result = self
            .client
            .send("Target.createTarget", Some(params))
            .await?
            .get("targetId")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "Target.createTarget did not return targetId".into(),
            })?;
        let target_id = result;

        let params = json!({ "targetId": target_id, "flatten": true });
        let result = self
            .client
            .send("Target.attachToTarget", Some(params))
            .await?
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "Target.attachToTarget did not return sessionId".into(),
            })?;
        let session_id = result;

        Ok(AsyncPage::new(
            Arc::clone(&self.client),
            session_id,
            target_id,
            self.browser_endpoint.clone(),
        ))
    }

    /// 获取所有 Tab（page 类型 target），每个 attach 后返回 AsyncPage。
    pub async fn tabs(&self) -> Result<Vec<AsyncPage>, CdpError> {
        let result = self.client.send("Target.getTargets", None).await?;
        let list = result
            .get("targetInfos")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "Target.getTargets did not return targetInfos".into(),
            })?;
        let mut pages = Vec::new();
        for info in list {
            let typ = info
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if typ != "page" {
                continue;
            }
            let target_id = info
                .get("targetId")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
                .ok_or_else(|| CdpError::Protocol {
                    id: None,
                    code: -1,
                    message: "A target entry did not include targetId".into(),
                })?;
            let params = json!({ "targetId": target_id, "flatten": true });
            let res = self
                .client
                .send("Target.attachToTarget", Some(params))
                .await?;
            let session_id = res
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
                .ok_or_else(|| CdpError::Protocol {
                    id: None,
                    code: -1,
                    message: "Target.attachToTarget did not return sessionId".into(),
                })?;
            pages.push(AsyncPage::new(
                Arc::clone(&self.client),
                session_id,
                target_id,
                self.browser_endpoint.clone(),
            ));
        }
        Ok(pages)
    }

    /// 获取浏览器版本信息。
    pub async fn version(&self) -> Result<BrowserVersion, CdpError> {
        let endpoint = self
            .browser_endpoint
            .as_deref()
            .unwrap_or("http://127.0.0.1:9222");
        fetch_browser_version_async(endpoint).await
    }
}

pub(crate) async fn fetch_ws_url_from_endpoint_async(endpoint: &str) -> Result<String, CdpError> {
    let version = fetch_json_version_async(endpoint).await?;
    version.web_socket_debugger_url.ok_or_else(|| {
        CdpError::Http("The /json/version response did not include webSocketDebuggerUrl".into())
    })
}

async fn fetch_browser_version_async(endpoint: &str) -> Result<BrowserVersion, CdpError> {
    let v = fetch_json_version_async(endpoint).await?;
    Ok(BrowserVersion {
        browser: v.browser.unwrap_or_default(),
        protocol_version: v.protocol_version.unwrap_or_default(),
        user_agent: v.user_agent.unwrap_or_default(),
        web_socket_debugger_url: v.web_socket_debugger_url.unwrap_or_default(),
    })
}

async fn fetch_json_version_async(endpoint: &str) -> Result<JsonVersion, CdpError> {
    let url = format!("{}/json/version", endpoint.trim_end_matches('/'));
    let response = reqwest::get(&url)
        .await
        .map_err(|e| CdpError::Http(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(CdpError::HttpStatus {
            status: status.as_u16(),
            body,
        });
    }
    response
        .json::<JsonVersion>()
        .await
        .map_err(|e| CdpError::Http(e.to_string()))
}
