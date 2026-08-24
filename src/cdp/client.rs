//! CDP WebSocket 客户端：连接、发送命令、接收响应

use super::protocol::{CdpCommand, CdpMessage};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use tungstenite::client::connect_with_config;
use tungstenite::Message;

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("WebSocket connection failed: {0}")]
    Connect(String),
    #[error("Failed to send message: {0}")]
    Send(String),
    #[error("Failed to receive message: {0}")]
    Recv(String),
    #[error("CDP error: id={id:?}, code={code}, message={message}")]
    Protocol {
        id: Option<i64>,
        code: i64,
        message: String,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Operation timed out: {0}")]
    Timeout(String),
    #[error("Channel closed: {0}")]
    ChannelClosed(String),
    #[error("WebSocket error: {0}")]
    WebSocket(String),
    #[error("HTTP status error: status={status}, body={body}")]
    HttpStatus { status: u16, body: String },
}

impl CdpError {
    /// 为错误追加上下文信息（如定位器表达式），方便排查
    pub fn with_context(self, ctx: &str) -> Self {
        match self {
            CdpError::Protocol { id, code, message } => CdpError::Protocol {
                id,
                code,
                message: format!("{} (locator: {})", message, ctx),
            },
            other => other,
        }
    }
}

/// CDP WebSocket 客户端（同步、单连接）
pub struct CdpClient {
    stream: Mutex<
        tungstenite::protocol::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    >,
    next_id: AtomicI64,
}

impl CdpClient {
    /// 连接到 CDP WebSocket URL（如 ws://127.0.0.1:9222/devtools/browser/xxx）
    pub fn connect(ws_url: &str) -> Result<Self, CdpError> {
        let url = ws_url
            .parse::<url::Url>()
            .map_err(|e| CdpError::Connect(e.to_string()))?;
        let config = tungstenite::protocol::WebSocketConfig {
            max_message_size: None,
            max_frame_size: None,
            ..Default::default()
        };
        let (stream, _) = connect_with_config(url, Some(config), 3)
            .map_err(|e| CdpError::Connect(e.to_string()))?;
        Ok(Self {
            stream: Mutex::new(stream),
            next_id: AtomicI64::new(1),
        })
    }

    /// 发送命令（无 session），返回 result 的 JSON Value
    pub fn send(&self, method: &str, params: Option<Value>) -> Result<Value, CdpError> {
        self.send_with_session(method, params, None)
    }

    /// 发送命令（可带 sessionId，用于多 Tab）
    pub fn send_with_session(
        &self,
        method: &str,
        params: Option<Value>,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cmd = CdpCommand {
            id,
            method: method.to_string(),
            params,
            session_id: session_id.map(String::from),
        };
        let msg = serde_json::to_string(&cmd).map_err(CdpError::Json)?;
        self.write_message(&msg)?;
        self.read_response_until_id(id)
    }

    fn write_message(&self, text: &str) -> Result<(), CdpError> {
        let mut guard = self
            .stream
            .lock()
            .map_err(|e| CdpError::Send(e.to_string()))?;
        guard
            .send(Message::Text(text.into()))
            .map_err(|e| CdpError::Send(e.to_string()))?;
        Ok(())
    }

    /// 读取消息直到收到指定 id 的响应（中间可能有 event，需跳过）
    fn read_response_until_id(&self, expect_id: i64) -> Result<Value, CdpError> {
        let mut guard = self
            .stream
            .lock()
            .map_err(|e| CdpError::Recv(e.to_string()))?;
        loop {
            let msg = guard.read().map_err(|e| CdpError::Recv(e.to_string()))?;
            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => return Err(CdpError::Recv("Connection closed".into())),
                _ => continue,
            };
            let resp: CdpMessage = serde_json::from_str(&text).map_err(CdpError::Json)?;
            if resp.id == Some(expect_id) {
                if let Some(e) = resp.error {
                    return Err(CdpError::Protocol {
                        id: Some(expect_id),
                        code: e.code,
                        message: e.message,
                    });
                }
                return resp.result.ok_or_else(|| CdpError::Protocol {
                    id: Some(expect_id),
                    code: -1,
                    message: "CDP response did not include a result payload".into(),
                });
            }
            // 否则是 event，继续读
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdp_error_display() {
        let e = CdpError::Http("timeout".into());
        assert!(e.to_string().contains("timeout"));
        let e = CdpError::Protocol {
            id: Some(1),
            code: -32600,
            message: "Invalid".into(),
        };
        assert!(e.to_string().contains("Invalid"));
        assert!(e.to_string().contains("-32600"));
    }
}
