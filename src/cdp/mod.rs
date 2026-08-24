//! Chrome DevTools Protocol 客户端
//! 通过 WebSocket 与 Chrome 通信

mod async_client;
mod client;
mod protocol;

pub use async_client::AsyncCdpClient;
pub use client::CdpClient;
pub use client::CdpError;
pub use protocol::CdpEvent;
