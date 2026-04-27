//! Chrome DevTools Protocol 客户端
//! 通过 WebSocket 与 Chrome 通信

mod client;

pub use client::CdpClient;
pub use client::CdpError;
