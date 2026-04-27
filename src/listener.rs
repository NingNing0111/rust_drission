//! 请求/响应监听：独立 CDP 连接接收 Network 事件，收集请求与响应数据

use crate::cdp::CdpError;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tungstenite::client::connect_with_config;
use tungstenite::Message;

/// 监听器：用于监听当前 Tab 的请求与响应，通过独立 CDP 连接接收 Network 事件
pub struct Listener {
    rx: Receiver<Result<DataPacket, CdpError>>,
    _join: Option<thread::JoinHandle<()>>,
}

/// 一次请求+响应的数据包（含可选的响应体）
#[derive(Debug, Clone)]
pub struct DataPacket {
    pub request: Request,
    pub response: Response,
    /// 响应体（部分类型可能无 body 或未获取）
    pub body: Option<Vec<u8>>,
    /// 是否加载失败（如 Network.loadingFailed）
    pub is_failed: bool,
    /// 资源类型：Document, XHR, Fetch, Script 等
    pub resource_type: Option<String>,
}

/// 请求信息（来自 CDP Network.requestWillBeSent）
#[derive(Debug, Clone)]
pub struct Request {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub post_data: Option<String>,
}

/// 响应信息（来自 CDP Network.responseReceived + getResponseBody）
#[derive(Debug, Clone)]
pub struct Response {
    pub url: String,
    pub status: Option<u32>,
    pub status_text: Option<String>,
    pub headers: HashMap<String, String>,
    /// 仅当 DataPacket.body 为 Some 时由监听器填充
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Deserialize)]
struct CdpMessage {
    id: Option<i64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
}

struct InProgressPacket {
    request: Request,
    response_url: String,
    response_status: Option<u32>,
    response_status_text: Option<String>,
    response_headers: HashMap<String, String>,
    resource_type: Option<String>,
}

impl Listener {
    /// 创建并启动监听（需要 Page 的 browser_endpoint 与 target_id，通常通过 `page.listen()` 获取）
    pub fn start(
        browser_endpoint: &str,
        target_id: &str,
    ) -> Result<Self, CdpError> {
        let ws_url = crate::browser::fetch_ws_url_from_endpoint(browser_endpoint)?;
        let (tx, rx) = std::sync::mpsc::channel();

        let target_id = target_id.to_string();
        let join = thread::spawn(move || {
            if let Err(e) = run_listener_loop(&ws_url, &target_id, &tx) {
                let _ = tx.send(Err(e));
            }
        });

        Ok(Self {
            rx,
            _join: Some(join),
        })
    }

    /// 阻塞直到收到一条数据包，或超时；超时返回 `Ok(None)`，错误返回 `Err`
    pub fn wait(&self, timeout: Duration) -> Result<Option<DataPacket>, CdpError> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(packet)) => Ok(Some(packet)),
            Ok(Err(e)) => Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    /// 阻塞直到收到一条数据包；无超时，连接断开则返回 `None`
    pub fn wait_one(&self) -> Result<Option<DataPacket>, CdpError> {
        match self.rx.recv() {
            Ok(Ok(packet)) => Ok(Some(packet)),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(None),
        }
    }

    /// 非阻塞：尝试取一条已就绪的数据包
    pub fn try_recv(&self) -> Result<Option<DataPacket>, CdpError> {
        match self.rx.try_recv() {
            Ok(Ok(packet)) => Ok(Some(packet)),
            Ok(Err(e)) => Err(e),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Ok(None),
        }
    }
}

fn run_listener_loop(
    ws_url: &str,
    target_id: &str,
    tx: &Sender<Result<DataPacket, CdpError>>,
) -> Result<(), CdpError> {
    let url = ws_url
        .parse::<url::Url>()
        .map_err(|e| CdpError::Connect(e.to_string()))?;
    let config = tungstenite::protocol::WebSocketConfig {
        max_message_size: None,
        max_frame_size: None,
        ..Default::default()
    };
    let (mut stream, _) =
        connect_with_config(url, Some(config), 3)
            .map_err(|e| CdpError::Connect(e.to_string()))?;

    // 附着到目标 Tab，获取 sessionId
    let attach_id = 1i64;
    let attach_msg = serde_json::to_string(&json!({
        "id": attach_id,
        "method": "Target.attachToTarget",
        "params": { "targetId": target_id, "flatten": true }
    }))
    .map_err(CdpError::Json)?;
    stream
        .send(Message::Text(attach_msg))
        .map_err(|e| CdpError::Send(e.to_string()))?;

    let session_id = read_until_id(&mut stream, attach_id)?
        .get("sessionId")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| CdpError::Protocol {
            id: Some(attach_id),
            code: -1,
            message: "Target.attachToTarget 无 sessionId".into(),
        })?;

    // 启用 Network 域
    let enable_id = 2i64;
    let enable_msg = serde_json::to_string(&json!({
        "id": enable_id,
        "method": "Network.enable",
        "sessionId": session_id
    }))
    .map_err(CdpError::Json)?;
    stream
        .send(Message::Text(enable_msg))
        .map_err(|e| CdpError::Send(e.to_string()))?;
    read_until_id(&mut stream, enable_id)?;

    let session_id = Arc::new(session_id);
    let next_id = Arc::new(AtomicI64::new(3));
    let request_ids: Arc<Mutex<HashMap<String, InProgressPacket>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_body: Arc<Mutex<HashMap<i64, (String, Request, Response)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        let msg = match stream.read() {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => return Err(CdpError::Recv(e.to_string())),
        };

        let parsed: CdpMessage = serde_json::from_str(&msg).map_err(CdpError::Json)?;

        if let Some(id) = parsed.id {
            // 命令响应（如 getResponseBody）
            if let Some(result) = parsed.result {
                let mut pending = pending_body.lock().map_err(|e| CdpError::Recv(e.to_string()))?;
                if let Some((_req_id, req, mut resp)) = pending.remove(&id) {
                    let body_str = result.get("body").and_then(Value::as_str);
                    let body_b64 = result
                        .get("base64Encoded")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let body = match (body_str, body_b64) {
                        (Some(s), true) => base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            s,
                        ).ok(),
                        (Some(s), false) => Some(s.as_bytes().to_vec()),
                        (None, _) => None,
                    };
                    resp.body = body.clone();
                    let packet = DataPacket {
                        request: req,
                        response: resp,
                        body,
                        is_failed: false,
                        resource_type: None,
                    };
                    drop(pending);
                    let _ = tx.send(Ok(packet));
                }
            }
            continue;
        }

        if let (Some(method), Some(params)) = (parsed.method, parsed.params) {
            match method.as_str() {
                "Network.requestWillBeSent" => {
                    let request = params
                        .get("request")
                        .ok_or_else(|| CdpError::Protocol {
                            id: None,
                            code: -1,
                            message: "requestWillBeSent 无 request".into(),
                        })?;
                    let request_id = params
                        .get("requestId")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_default();
                    let url = request.get("url").and_then(Value::as_str).unwrap_or("").to_string();
                    let method = request
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or("GET")
                        .to_string();
                    let headers = parse_headers(request.get("headers"));
                    let post_data = request.get("postData").and_then(Value::as_str).map(String::from);
                    let resource_type = params.get("type").and_then(Value::as_str).map(String::from);
                    let req = Request {
                        url: url.clone(),
                        method,
                        headers,
                        post_data,
                    };
                    let in_progress = InProgressPacket {
                        request: req,
                        response_url: url,
                        response_status: None,
                        response_status_text: None,
                        response_headers: HashMap::new(),
                        resource_type,
                    };
                    request_ids
                        .lock()
                        .map_err(|e| CdpError::Recv(e.to_string()))?
                        .insert(request_id, in_progress);
                }
                "Network.responseReceived" => {
                    let request_id = params
                        .get("requestId")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_default();
                    let response = params.get("response").cloned().unwrap_or(Value::Object(serde_json::Map::new()));
                    let url = response.get("url").and_then(Value::as_str).unwrap_or("").to_string();
                    let status = response.get("status").and_then(Value::as_u64).map(|u| u as u32);
                    let status_text = response.get("statusText").and_then(Value::as_str).map(String::from);
                    let headers = parse_headers(response.get("headers"));
                    let resource_type = params.get("type").and_then(Value::as_str).map(String::from);

                    let mut ids = request_ids.lock().map_err(|e| CdpError::Recv(e.to_string()))?;
                    if let Some(p) = ids.get_mut(&request_id) {
                        p.response_url = url;
                        p.response_status = status;
                        p.response_status_text = status_text;
                        p.response_headers = headers;
                        if resource_type.is_some() {
                            p.resource_type = resource_type;
                        }
                    }
                }
                "Network.loadingFinished" => {
                    let request_id = params
                        .get("requestId")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_default();
                    let mut ids = request_ids.lock().map_err(|e| CdpError::Recv(e.to_string()))?;
                    let in_progress = match ids.remove(&request_id) {
                        Some(p) => p,
                        None => continue,
                    };
                    drop(ids);

                    let req = in_progress.request;
                    let resp = Response {
                        url: in_progress.response_url,
                        status: in_progress.response_status,
                        status_text: in_progress.response_status_text,
                        headers: in_progress.response_headers,
                        body: None,
                    };

                    let id = next_id.fetch_add(1, Ordering::SeqCst);
                    let get_body_msg = serde_json::to_string(&json!({
                        "id": id,
                        "method": "Network.getResponseBody",
                        "params": { "requestId": request_id },
                        "sessionId": session_id.as_str()
                    }))
                    .map_err(CdpError::Json)?;
                    stream
                        .send(Message::Text(get_body_msg))
                        .map_err(|e| CdpError::Send(e.to_string()))?;

                    pending_body
                        .lock()
                        .map_err(|e| CdpError::Recv(e.to_string()))?
                        .insert(id, (request_id, req, resp));
                }
                "Network.loadingFailed" => {
                    let request_id = params
                        .get("requestId")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .unwrap_or_default();
                    let resource_type = params.get("type").and_then(Value::as_str).map(String::from);
                    let mut ids = request_ids.lock().map_err(|e| CdpError::Recv(e.to_string()))?;
                    let in_progress = match ids.remove(&request_id) {
                        Some(p) => p,
                        None => continue,
                    };
                    drop(ids);
                    let resp = Response {
                        url: in_progress.response_url,
                        status: in_progress.response_status,
                        status_text: in_progress.response_status_text,
                        headers: in_progress.response_headers,
                        body: None,
                    };
                    let packet = DataPacket {
                        request: in_progress.request,
                        response: resp,
                        body: None,
                        is_failed: true,
                        resource_type,
                    };
                    let _ = tx.send(Ok(packet));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn read_until_id(
    stream: &mut tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    expect_id: i64,
) -> Result<Value, CdpError> {
    loop {
        let msg = stream
            .read()
            .map_err(|e| CdpError::Recv(e.to_string()))?;
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => return Err(CdpError::Recv("连接已关闭".into())),
            _ => continue,
        };
        let parsed: CdpMessage = serde_json::from_str(&text).map_err(CdpError::Json)?;
        if parsed.id == Some(expect_id) {
            if let Some(result) = parsed.result {
                return Ok(result);
            }
            return Err(CdpError::Protocol {
                id: Some(expect_id),
                code: -1,
                message: "响应无 result".into(),
            });
        }
    }
}

fn parse_headers(v: Option<&Value>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let obj = match v.and_then(Value::as_object) {
        Some(o) => o,
        None => return out,
    };
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            out.insert(k.clone(), s.to_string());
        }
    }
    out
}
