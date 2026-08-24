//! CDP 协议消息类型

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// CDP 命令请求（发送格式，字段名须为 CDP 规定的 id/method/sessionId/params）
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CdpCommand {
    pub id: i64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "sessionId")]
    pub session_id: Option<String>,
}

/// CDP 错误响应体。
#[derive(Debug, Clone, Deserialize)]
pub struct CdpErrorBody {
    pub code: i64,
    pub message: String,
}

/// CDP 输入消息：既可能是 response，也可能是 event。
#[derive(Debug, Clone, Deserialize)]
pub struct CdpMessage {
    pub id: Option<i64>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<CdpErrorBody>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
}

/// CDP 事件。
#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}
