//! 浏览器连接与 Tab 管理

mod async_browser;
mod config;

use crate::cdp::{CdpClient, CdpError};
use crate::page::Page;
use serde::Deserialize;
use serde_json::json;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

pub use async_browser::AsyncBrowser;
pub use config::BrowserConfig;

/// 浏览器版本信息（来自 /json/version）
#[derive(Debug, Clone)]
pub struct BrowserVersion {
    pub browser: String,
    pub protocol_version: String,
    pub user_agent: String,
    pub web_socket_debugger_url: String,
}

/// 浏览器实例：连接已有 Chrome 或启动新 Chrome，管理 Tab
pub struct Browser {
    /// CDP 客户端（连接 browser 的 WebSocket），多 Tab 共享
    client: Arc<CdpClient>,
    /// 仅 launch 时：子进程句柄，用于 close 时结束
    _child: Option<Child>,
    /// HTTP 调试地址，用于 Listener 建立独立连接（如 http://127.0.0.1:9222）
    pub(crate) browser_endpoint: Option<String>,
}

impl Browser {
    /// 连接已有 Chrome（需已用 --remote-debugging-port 启动）
    /// endpoint 支持："http://127.0.0.1:9222" 或 "127.0.0.1:9222"（与 DrissionPage 一致）
    pub fn connect(endpoint: &str) -> Result<Self, CdpError> {
        let endpoint = normalize_endpoint(endpoint);
        let ws_url = fetch_ws_url_from_endpoint(&endpoint)?;
        let client = CdpClient::connect(&ws_url)?;
        Ok(Self {
            client: Arc::new(client),
            _child: None,
            browser_endpoint: Some(endpoint),
        })
    }

    /// 启动新的 Chrome 进程并连接（与 DrissionPage 默认参数一致）
    pub fn launch(config: BrowserConfig) -> Result<Self, CdpError> {
        let (endpoint, child) = launch_chrome(&config)?;
        let mut browser = Self::connect(&endpoint)?;
        browser._child = Some(child);
        browser.browser_endpoint = Some(endpoint);
        Ok(browser)
    }

    /// 若指定地址已有浏览器则连接，否则启动新浏览器再连接（与 DrissionPage ChromiumPage(addr_or_opts) 一致）
    pub fn connect_or_launch(config: BrowserConfig) -> Result<Self, CdpError> {
        let address = config
            .get_address()
            .map(String::from)
            .unwrap_or_else(|| format!("127.0.0.1:{}", config.get_remote_debugging_port()));
        let endpoint = format!("http://{}", address);

        if config.get_existing_only() {
            return Self::connect(&endpoint);
        }

        let (host, port_str) = address.split_once(':').unwrap_or(("127.0.0.1", "9222"));
        let port: u16 = port_str.parse().unwrap_or(9222);
        let in_use = port_in_use(host, port);

        if host != "127.0.0.1" || in_use {
            return Self::connect(&endpoint);
        }

        Self::launch(config)
    }

    /// 新建一个 Tab（about:blank），返回 Page
    pub fn new_tab(&self) -> Result<Page, CdpError> {
        let params = json!({ "url": "about:blank" });
        let result = self
            .client
            .send("Target.createTarget", Some(params))?
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
            .send("Target.attachToTarget", Some(params))?
            .get("sessionId")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "Target.attachToTarget did not return sessionId".into(),
            })?;
        let session_id = result;

        Ok(Page::new(
            Arc::clone(&self.client),
            session_id,
            target_id,
            self.browser_endpoint.clone(),
        ))
    }

    /// 获取所有 Tab（page 类型 target），每个 attach 后返回 Page
    pub fn tabs(&self) -> Result<Vec<Page>, CdpError> {
        let result = self.client.send("Target.getTargets", None)?;
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
            let res = self.client.send("Target.attachToTarget", Some(params))?;
            let session_id = res
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .map(String::from)
                .ok_or_else(|| CdpError::Protocol {
                    id: None,
                    code: -1,
                    message: "Target.attachToTarget did not return sessionId".into(),
                })?;
            pages.push(Page::new(
                Arc::clone(&self.client),
                session_id,
                target_id,
                self.browser_endpoint.clone(),
            ));
        }
        Ok(pages)
    }

    /// 获取所有 Tab 的 target ID 列表（与 DrissionPage `tab_ids` 一致）
    pub fn tab_ids(&self) -> Result<Vec<String>, CdpError> {
        let result = self.client.send("Target.getTargets", None)?;
        let list = result
            .get("targetInfos")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "Target.getTargets did not return targetInfos".into(),
            })?;
        let ids: Vec<String> = list
            .iter()
            .filter(|info| info.get("type").and_then(serde_json::Value::as_str) == Some("page"))
            .filter_map(|info| {
                info.get("targetId")
                    .and_then(serde_json::Value::as_str)
                    .map(String::from)
            })
            .collect();
        Ok(ids)
    }

    /// Tab 总数（与 DrissionPage `tabs_count` 一致）
    pub fn tabs_count(&self) -> Result<usize, CdpError> {
        let ids = self.tab_ids()?;
        Ok(ids.len())
    }

    /// 最新打开的 Tab（与 DrissionPage `latest_tab` 一致）
    pub fn latest_tab(&self) -> Result<Page, CdpError> {
        let tabs = self.tabs()?;
        tabs.into_iter().last().ok_or_else(|| CdpError::Protocol {
            id: None,
            code: -1,
            message: "No open tabs were found".into(),
        })
    }

    /// 按条件获取单个 Tab（与 DrissionPage `get_tab` 一致）
    /// `id_or_num` - target ID 字符串或 1-based 序号
    /// `title` - 可选，标题模糊匹配
    /// `url` - 可选，URL 模糊匹配
    /// `tab_type` - 可选，Tab 类型（如 "page"）
    pub fn get_tab(
        &self,
        id_or_num: &str,
        title: Option<&str>,
        url: Option<&str>,
        tab_type: Option<&str>,
    ) -> Result<Option<Page>, CdpError> {
        let tabs = self.tabs()?;
        // 如果是数字，视为 1-based 序号
        if let Ok(num) = id_or_num.parse::<usize>() {
            if num == 0 {
                return Ok(None);
            }
            return Ok(tabs.into_iter().nth(num - 1));
        }
        for tab in tabs {
            // 按 targetId 精确匹配
            if tab.target_id == id_or_num {
                return Ok(Some(tab));
            }
            // 按标题/URL 模糊匹配
            let matches_title = title
                .map(|t| {
                    tab.title()
                        .map(|tab_title| tab_title.contains(t))
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            let matches_url = url
                .map(|u| {
                    tab.url()
                        .map(|tab_url| tab_url.contains(u))
                        .unwrap_or(false)
                })
                .unwrap_or(true);
            let matches_type = tab_type.map(|tp| tp == "page").unwrap_or(true);
            if matches_title && matches_url && matches_type {
                return Ok(Some(tab));
            }
        }
        Ok(None)
    }

    /// 按条件筛选 Tab 列表（与 DrissionPage `get_tabs` 一致）
    pub fn get_tabs(
        &self,
        title: Option<&str>,
        url: Option<&str>,
        tab_type: Option<&str>,
    ) -> Result<Vec<Page>, CdpError> {
        let tabs = self.tabs()?;
        let mut result = Vec::new();
        for tab in tabs {
            let matches_title = title
                .map(|t| tab.title().map(|title| title.contains(t)).unwrap_or(false))
                .unwrap_or(true);
            let matches_url = url
                .map(|u| tab.url().map(|url| url.contains(u)).unwrap_or(false))
                .unwrap_or(true);
            let matches_type = tab_type.map(|tp| tp == "page").unwrap_or(true);
            if matches_title && matches_url && matches_type {
                result.push(tab);
            }
        }
        Ok(result)
    }

    /// 激活/切换到指定 Tab（与 DrissionPage `activate_tab` 一致）
    pub fn activate_tab(&self, target_id: &str) -> Result<(), CdpError> {
        let params = json!({ "targetId": target_id });
        self.client.send("Target.activateTarget", Some(params))?;
        Ok(())
    }

    /// 关闭指定 Tab 或"其余"Tab（与 DrissionPage `close_tabs` 一致）
    /// `tabs_or_ids` - 要关闭的 Tab 列表（Page 或 target ID），传入空列表则关闭当前 Tab
    /// `others` - true=关闭除指定 Tab 外的所有 Tab，false=关闭指定的 Tab
    pub fn close_tabs(&self, tab_ids: &[String], others: bool) -> Result<(), CdpError> {
        let all_ids = self.tab_ids()?;
        let to_close: Vec<String> = if others {
            all_ids
                .into_iter()
                .filter(|id| !tab_ids.contains(id))
                .collect()
        } else {
            tab_ids.to_vec()
        };
        for id in to_close {
            let params = json!({ "targetId": id });
            let _ = self.client.send("Target.closeTarget", Some(params));
        }
        Ok(())
    }

    /// 获取浏览器版本信息
    pub fn version(&self) -> Result<BrowserVersion, CdpError> {
        let endpoint = "http://127.0.0.1:9222";
        let body = ureq::get(&format!("{}/json/version", endpoint))
            .call()
            .map_err(|e| CdpError::Http(e.to_string()))?
            .into_string()
            .map_err(|e| CdpError::Http(e.to_string()))?;
        let v: JsonVersion = serde_json::from_str(&body).map_err(CdpError::Json)?;
        Ok(BrowserVersion {
            browser: v.browser.unwrap_or_default(),
            protocol_version: v.protocol_version.unwrap_or_default(),
            user_agent: v.user_agent.unwrap_or_default(),
            web_socket_debugger_url: v.web_socket_debugger_url.unwrap_or_default(),
        })
    }

    /// 关闭浏览器（仅对 launch 的实例有效，会结束子进程）
    pub fn close(&mut self) {
        if let Some(mut child) = self._child.take() {
            let _ = child.kill();
        }
    }
}

/// 将 "127.0.0.1:9222" 规范为 "http://127.0.0.1:9222"
pub(crate) fn normalize_endpoint(endpoint: &str) -> String {
    let s = endpoint.trim();
    if s.is_empty() {
        return "http://127.0.0.1:9222".to_string();
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    format!("http://{}", s)
}

/// 检测 host:port 是否已被占用（用于决定连接已有浏览器还是启动新进程）
fn port_in_use(host: &str, port: u16) -> bool {
    let addr = match format!("{}:{}", host, port).parse::<std::net::SocketAddr>() {
        Ok(a) => a,
        Err(_) => return false,
    };
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(100)).is_ok()
}

/// 从 HTTP endpoint 获取 browser 的 WebSocket URL（供 Listener 等建立独立连接使用）
pub(crate) fn fetch_ws_url_from_endpoint(endpoint: &str) -> Result<String, CdpError> {
    let url = format!("{}/json/version", endpoint.trim_end_matches('/'));
    let body = ureq::get(&url)
        .call()
        .map_err(|e| CdpError::Http(e.to_string()))?
        .into_string()
        .map_err(|e| CdpError::Http(e.to_string()))?;
    let v: JsonVersion = serde_json::from_str(&body).map_err(CdpError::Json)?;
    v.web_socket_debugger_url.ok_or_else(|| {
        CdpError::Http("The /json/version response did not include webSocketDebuggerUrl".into())
    })
}

#[derive(Deserialize)]
pub(crate) struct JsonVersion {
    #[serde(rename = "Browser")]
    pub(crate) browser: Option<String>,
    #[serde(rename = "Protocol-Version")]
    pub(crate) protocol_version: Option<String>,
    #[serde(rename = "User-Agent")]
    pub(crate) user_agent: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub(crate) web_socket_debugger_url: Option<String>,
}

/// 启动 Chrome 进程，返回 (http_endpoint, child)。行为对齐 DrissionPage browser._run_browser + get_launch_args
pub(crate) fn launch_chrome(config: &BrowserConfig) -> Result<(String, Child), CdpError> {
    let port = config.get_remote_debugging_port();
    let host = config
        .get_address()
        .and_then(|a| a.split_once(':').map(|(h, _)| h.to_string()))
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let endpoint = format!("http://{}:{}", host, port);

    let chrome = resolve_chrome_path(config)?;

    let mut args = vec![format!("--remote-debugging-port={}", port)];

    // 与 DrissionPage 一致：未指定时使用临时目录 userData/{port}
    let user_data_dir = match config.get_user_data_dir() {
        Some(d) => {
            let path = std::path::Path::new(d);
            if let Err(e) = std::fs::create_dir_all(path) {
                return Err(CdpError::Http(format!(
                    "Failed to create user data directory '{}': {}",
                    d, e
                )));
            }
            d.to_string()
        }
        None => {
            let tmp = config
                .get_tmp_path()
                .map(std::path::Path::new)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(std::env::temp_dir);
            let dir = tmp
                .join("DrissionPage")
                .join("userData")
                .join(port.to_string());
            if let Some(p) = dir.to_str() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    return Err(CdpError::Http(format!(
                        "Failed to create default user data directory '{}': {}",
                        p, e
                    )));
                }
                p.to_string()
            } else {
                return Err(CdpError::Http(
                    "Failed to build the default user-data-dir path".into(),
                ));
            }
        }
    };
    args.push(format!("--user-data-dir={}", user_data_dir));

    // 清理 Singleton 锁文件，避免浏览器异常退出后无法重新启动
    for f in &["SingletonLock", "SingletonCookie", "SingletonSocket"] {
        let lock_path = std::path::Path::new(&user_data_dir).join(f);
        std::fs::remove_file(&lock_path).ok();
    }

    args.push("--window-size=1920,1080".to_string());
    if config.get_headless() {
        let has_headless = config
            .get_args()
            .iter()
            .any(|a| a.starts_with("--headless"));
        if !has_headless {
            args.push("--headless=new".to_string());
        }
    }
    args.extend(config.get_args().to_vec());

    let mut child = Command::new(&chrome)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| CdpError::Http(format!("Failed to launch Chrome: {}", e)))?;

    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if ureq::get(&format!("{}/json/version", endpoint))
            .call()
            .is_ok()
        {
            return Ok((endpoint, child));
        }
    }
    let _ = child.kill();
    Err(CdpError::Http(
        "Chrome did not become ready within 5 seconds after launch".into(),
    ))
}

/// 解析 Chrome 可执行路径：配置 > 目录+chrome(.exe) > 自动查找
fn resolve_chrome_path(config: &BrowserConfig) -> Result<String, CdpError> {
    if let Some(p) = config.get_chrome_path() {
        let path = std::path::Path::new(p);
        if path.is_dir() {
            #[cfg(windows)]
            let exe = path.join("chrome.exe");
            #[cfg(not(windows))]
            let exe = path.join("chrome");
            if exe.exists() {
                return Ok(exe.to_string_lossy().into_owned());
            }
        }
        return Ok(p.to_string());
    }
    Ok(find_chrome_executable())
}

/// 与 DrissionPage get_chrome_path 一致：注册表(Portable) / PATH / 默认名
fn find_chrome_executable() -> String {
    #[cfg(windows)]
    {
        if let Some(p) = find_chrome_windows() {
            return p;
        }
    }
    #[cfg(target_os = "macos")]
    {
        const MAC_PATHS: &[&str] = &[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Google Chrome Canary.app/Contents/MacOS/Google Chrome Canary",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        ];
        for path in MAC_PATHS {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }
        for name in [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ] {
            if let Some(p) = which(name) {
                return p;
            }
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        for name in [
            "google-chrome",
            "google-chrome-stable",
            "chromium",
            "chromium-browser",
        ] {
            if which(name).is_some() {
                return name.to_string();
            }
        }
    }
    default_chrome_name().to_string()
}

#[cfg(windows)]
fn find_chrome_windows() -> Option<String> {
    use std::path::Path;
    let key_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe";
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok(path) = get_reg_string(hkcu, key_path, "") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    let hklm = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE);
    if let Ok(path) = get_reg_string(hklm, key_path, "") {
        if Path::new(&path).exists() {
            return Some(path);
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let exe = dir.join("chrome.exe");
        if exe.exists() {
            return Some(exe.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn get_reg_string(
    hkey: winreg::RegKey,
    subkey: &str,
    name: &str,
) -> Result<String, std::io::Error> {
    let key = hkey.open_subkey_with_flags(subkey, winreg::enums::KEY_READ)?;
    key.get_value(name)
}

#[cfg(unix)]
fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let exe = dir.join(name);
        if exe.exists() {
            return Some(exe.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(windows)]
fn default_chrome_name() -> &'static str {
    "chrome"
}

#[cfg(not(windows))]
fn default_chrome_name() -> &'static str {
    "google-chrome"
}
