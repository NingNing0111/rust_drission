//! 异步 CDP WebSocket 客户端

use super::client::CdpError;
use super::protocol::{CdpCommand, CdpEvent, CdpMessage};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

struct OutboundCommand {
    command: CdpCommand,
    response_tx: oneshot::Sender<Result<Value, CdpError>>,
}

/// 基于 tokio 的 CDP 客户端：单 reader 路由响应，统一事件广播。
#[derive(Clone)]
pub struct AsyncCdpClient {
    tx: mpsc::Sender<OutboundCommand>,
    events: broadcast::Sender<CdpEvent>,
    pending: Arc<DashMap<i64, oneshot::Sender<Result<Value, CdpError>>>>,
    next_id: Arc<AtomicI64>,
    command_timeout: Duration,
}

impl AsyncCdpClient {
    /// 连接到 CDP WebSocket URL（如 ws://127.0.0.1:9222/devtools/browser/xxx）。
    pub async fn connect(ws_url: &str) -> Result<Self, CdpError> {
        let (stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| CdpError::Connect(e.to_string()))?;
        let (mut ws_writer, mut ws_reader) = stream.split();
        let (tx, mut rx) = mpsc::channel::<OutboundCommand>(1024);
        let (events, _) = broadcast::channel::<CdpEvent>(1024);
        let pending: Arc<DashMap<i64, oneshot::Sender<Result<Value, CdpError>>>> =
            Arc::new(DashMap::new());

        let writer_pending = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(outbound) = rx.recv().await {
                let id = outbound.command.id;
                let method = outbound.command.method.clone();
                let session_id = outbound.command.session_id.clone();
                let text = match serde_json::to_string(&outbound.command) {
                    Ok(text) => text,
                    Err(e) => {
                        let _ = outbound.response_tx.send(Err(CdpError::Json(e)));
                        continue;
                    }
                };
                writer_pending.insert(id, outbound.response_tx);
                tracing::debug!(id, method = %method, session_id = ?session_id, "send cdp command");
                if let Err(e) = ws_writer.send(Message::Text(text)).await {
                    if let Some((_, response_tx)) = writer_pending.remove(&id) {
                        let _ = response_tx.send(Err(CdpError::Send(e.to_string())));
                    }
                    break;
                }
            }
            drain_pending(
                &writer_pending,
                CdpError::ChannelClosed("CDP writer task stopped".into()),
            );
        });

        let reader_pending = Arc::clone(&pending);
        let reader_events = events.clone();
        tokio::spawn(async move {
            while let Some(item) = ws_reader.next().await {
                let message = match item {
                    Ok(Message::Text(text)) => text,
                    Ok(Message::Close(_)) => {
                        drain_pending(&reader_pending, CdpError::Recv("Connection closed".into()));
                        return;
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        drain_pending(&reader_pending, CdpError::Recv(e.to_string()));
                        return;
                    }
                };

                let parsed: CdpMessage = match serde_json::from_str(&message) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to parse cdp message");
                        continue;
                    }
                };

                if let Some(id) = parsed.id {
                    if let Some((_, response_tx)) = reader_pending.remove(&id) {
                        let response = if let Some(e) = parsed.error {
                            Err(CdpError::Protocol {
                                id: Some(id),
                                code: e.code,
                                message: e.message,
                            })
                        } else {
                            parsed.result.ok_or_else(|| CdpError::Protocol {
                                id: Some(id),
                                code: -1,
                                message: "CDP response did not include a result payload".into(),
                            })
                        };
                        let _ = response_tx.send(response);
                    } else {
                        tracing::warn!(id, "received response for unknown cdp command");
                    }
                    continue;
                }

                if let Some(method) = parsed.method {
                    let params = parsed.params.unwrap_or(Value::Null);
                    tracing::trace!(method = %method, session_id = ?parsed.session_id, "receive cdp event");
                    let _ = reader_events.send(CdpEvent {
                        method,
                        params,
                        session_id: parsed.session_id,
                    });
                }
            }
            drain_pending(
                &reader_pending,
                CdpError::Recv("CDP reader task stopped".into()),
            );
        });

        Ok(Self {
            tx,
            events,
            pending,
            next_id: Arc::new(AtomicI64::new(1)),
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
        })
    }

    /// 订阅 CDP 事件。
    pub fn subscribe_events(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// 发送命令（无 session），返回 result 的 JSON Value。
    pub async fn send(&self, method: &str, params: Option<Value>) -> Result<Value, CdpError> {
        self.send_with_session(method, params, None).await
    }

    /// 发送命令（可带 sessionId，用于多 Tab）。
    pub async fn send_with_session(
        &self,
        method: &str,
        params: Option<Value>,
        session_id: Option<&str>,
    ) -> Result<Value, CdpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let command = CdpCommand {
            id,
            method: method.to_string(),
            params,
            session_id: session_id.map(String::from),
        };
        let (response_tx, response_rx) = oneshot::channel();
        self.tx
            .send(OutboundCommand {
                command,
                response_tx,
            })
            .await
            .map_err(|_| CdpError::ChannelClosed("CDP send queue closed".into()))?;

        match timeout(self.command_timeout, response_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CdpError::ChannelClosed(format!(
                "CDP response channel closed for command id {}",
                id
            ))),
            Err(_) => {
                self.pending.remove(&id);
                tracing::warn!(id, method = %method, "cdp command timed out");
                Err(CdpError::Timeout(format!(
                    "CDP command '{}' timed out after {:?}",
                    method, self.command_timeout
                )))
            }
        }
    }
}

fn drain_pending(
    pending: &DashMap<i64, oneshot::Sender<Result<Value, CdpError>>>,
    error: CdpError,
) {
    let waiters: Vec<_> = pending.iter().map(|entry| *entry.key()).collect();
    let message = error.to_string();
    for id in waiters {
        if let Some((_, tx)) = pending.remove(&id) {
            let _ = tx.send(Err(CdpError::ChannelClosed(message.clone())));
        }
    }
}
