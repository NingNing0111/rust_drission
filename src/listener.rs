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

/// 过滤条件（内部使用）
enum PacketFilter {
    /// 不过滤，接收所有数据包
    None,
    /// 只保留 URL 包含指定字符串的数据包
    UrlContains(String),
    /// 只保留指定资源类型的数据包
    ResourceType(String),
}

/// 监听器：用于监听当前 Tab 的请求与响应，通过独立 CDP 连接接收 Network 事件
pub struct Listener {
    rx: Receiver<Result<DataPacket, CdpError>>,
    _join: Option<thread::JoinHandle<()>>,
    filter: PacketFilter,
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
    /// 创建并启动监听（需要 Page 的 browser_endpoint 与 target_id，通常通过 `page.listen()` 获取）。
    /// 阻塞直到后台线程完成 WebSocket 连接、附着 Tab 并启用 Network 域，确保不丢失事件。
    pub fn start(
        browser_endpoint: &str,
        target_id: &str,
    ) -> Result<Self, CdpError> {
        let ws_url = crate::browser::fetch_ws_url_from_endpoint(browser_endpoint)?;
        let (tx, rx) = std::sync::mpsc::channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();

        let target_id = target_id.to_string();
        let join = thread::spawn(move || {
            if let Err(e) = run_listener_loop(&ws_url, &target_id, &tx, &ready_tx) {
                let _ = tx.send(Err(e));
            }
        });

        // 等待后台线程完成初始化（连接 + attach + Network.enable）
        ready_rx
            .recv()
            .map_err(|_| CdpError::Connect("监听线程启动失败".into()))?;

        Ok(Self {
            rx,
            _join: Some(join),
            filter: PacketFilter::None,
        })
    }

    /// 阻塞直到收到一条符合过滤条件的数据包，或超时；超时返回 `Ok(None)`，错误返回 `Err`
    pub fn wait(&self, timeout: Duration) -> Result<Option<DataPacket>, CdpError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            match self.rx.recv_timeout(remaining) {
                Ok(Ok(packet)) => {
                    if self.matches_filter(&packet) {
                        return Ok(Some(packet));
                    }
                    // 不匹配过滤条件，继续等待
                    continue;
                }
                Ok(Err(e)) => return Err(e),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Ok(None),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
            }
        }
    }

    /// 阻塞直到收到一条符合过滤条件的数据包；无超时，连接断开则返回 `None`
    pub fn wait_one(&self) -> Result<Option<DataPacket>, CdpError> {
        loop {
            match self.rx.recv() {
                Ok(Ok(packet)) => {
                    if self.matches_filter(&packet) {
                        return Ok(Some(packet));
                    }
                    continue;
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => return Ok(None),
            }
        }
    }

    /// 非阻塞：尝试取一条已就绪且符合过滤条件的数据包
    pub fn try_recv(&self) -> Result<Option<DataPacket>, CdpError> {
        // 非阻塞模式下需要排空所有已就绪消息来找匹配的
        loop {
            match self.rx.try_recv() {
                Ok(Ok(packet)) => {
                    if self.matches_filter(&packet) {
                        return Ok(Some(packet));
                    }
                    continue;
                }
                Ok(Err(e)) => return Err(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(None),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(None),
            }
        }
    }

    /// 创建 URL 过滤监听器：只保留 URL 包含指定字符串的数据包
    pub fn filter_url(mut self, url_contains: String) -> Self {
        self.filter = PacketFilter::UrlContains(url_contains);
        self
    }

    /// 创建资源类型过滤监听器：只保留指定资源类型的数据包
    pub fn filter_resource_type(mut self, resource_type: String) -> Self {
        self.filter = PacketFilter::ResourceType(resource_type);
        self
    }

    /// 检查数据包是否匹配当前过滤条件
    fn matches_filter(&self, packet: &DataPacket) -> bool {
        match &self.filter {
            PacketFilter::None => true,
            PacketFilter::UrlContains(pattern) => {
                packet.request.url.contains(pattern)
                    || packet.response.url.contains(pattern)
            }
            PacketFilter::ResourceType(rt) => packet
                .resource_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case(rt))
                .unwrap_or(false),
        }
    }

    /// 持续收集数据包，每收到一个调用 `on_packet` 回调。
    /// 回调返回 `false` 停止收集；`timeout` 为单次等待超时。
    /// 返回所有已收集的数据包。
    pub fn collect<F>(
        &self,
        timeout: Duration,
        mut on_packet: F,
    ) -> Result<Vec<DataPacket>, CdpError>
    where
        F: FnMut(&DataPacket) -> bool,
    {
        let mut collected = Vec::new();
        loop {
            match self.wait(timeout) {
                Ok(Some(packet)) => {
                    let should_continue = on_packet(&packet);
                    collected.push(packet);
                    if !should_continue {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(collected)
    }
}

fn run_listener_loop(
    ws_url: &str,
    target_id: &str,
    tx: &Sender<Result<DataPacket, CdpError>>,
    ready_tx: &Sender<()>,
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

    // 通知主线程：监听已就绪
    let _ = ready_tx.send(());

    let session_id = Arc::new(session_id);
    let next_id = Arc::new(AtomicI64::new(3));
    let request_ids: Arc<Mutex<HashMap<String, InProgressPacket>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let pending_body: Arc<Mutex<HashMap<i64, (String, Request, Response, Option<String>)>>> =
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
                if let Some((_req_id, req, mut resp, resource_type)) = pending.remove(&id) {
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
                        resource_type,
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
                    let resource_type = in_progress.resource_type;
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
                        .insert(id, (request_id, req, resp, resource_type));
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
