//! Rust DrissionPage：浏览器自动化库
//!
//! API 与 [DrissionPage](https://github.com/g1879/DrissionPage) Python 版尽可能一致。
//! 支持连接已有 Chrome、启动新 Chrome、CDP 控制、DOM/元素、等待等，以及请求/响应监听。
//!
//! # 快速开始（与 DrissionPage 一致）
//!
//! ```no_run
//! use rust_drission::ChromiumPage;
//! use rust_drission::BrowserConfig;
//!
//! let page = ChromiumPage::new(BrowserConfig::new()).unwrap();
//! page.get("https://www.example.com").unwrap();
//! let title = page.title().unwrap();
//! let el = page.ele("css:#kw").unwrap();
//! ```
//!
//! 也可使用 [Browser] + [Page] 分步操作；[Page] 提供 `get`/`refresh`/`run_js`/`ele`/`eles` 等与 DrissionPage 同名方法。

pub mod browser;
pub mod cdp;
pub mod chromium_page;
pub mod dom;
pub mod element;
pub mod frame;
pub mod listener;
pub mod locator;
pub mod page;
pub mod stealth;
pub mod utils;

pub use browser::{Browser, BrowserConfig, BrowserVersion};
pub use cdp::CdpError;
pub use chromium_page::ChromiumPage;
pub use element::Element;
pub use frame::Frame;
pub use listener::{DataPacket, Listener, Request, Response};
pub use locator::{Locator, LocatorParseError};
pub use page::{Cookie, Page};
pub use stealth::inject as stealth_inject;
