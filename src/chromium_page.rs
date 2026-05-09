//! ChromiumPage：与 DrissionPage 一致的入口，整合浏览器与当前标签页
//!
//! 用法与 Python 版一致：
//! - `ChromiumPage::new(config)` 连接或启动浏览器并绑定当前标签页
//! - `page.get(url)` 访问网址
//! - `page.ele(locator)` / `page.eles(locator)` 查找元素
//! - `page.run_js(script)` 执行脚本

use crate::browser::{Browser, BrowserConfig};
use crate::cdp::CdpError;
use crate::element::Element;
use crate::frame::Frame;
use crate::listener::{DataPacket, Listener};
use crate::page::{Cookie, Page};
use crate::stealth;
use serde_json::Value;
use std::time::Duration;

/// 与 DrissionPage 一致的页面对象：同时持有浏览器与当前标签页。
/// 单标签场景下可直接 `ChromiumPage::new()` 后 `get()`、`ele()`、`eles()` 等。
pub struct ChromiumPage {
    /// 浏览器实例（保持连接）
    pub(crate) browser: Browser,
    /// 当前控制的标签页（对应 Python 的 self.tab）
    pub(crate) page: Page,
}

impl ChromiumPage {
    /// 连接已有浏览器或启动新浏览器，并绑定当前标签页（与 DrissionPage `ChromiumPage(addr_or_opts)` 一致）。
    /// 若有已存在标签页则使用第一个，否则新建 about:blank 标签页。
    /// 默认注入 stealth 反检测脚本；如需禁用请用 [`new_without_stealth`](Self::new_without_stealth)。
    pub fn new(config: BrowserConfig) -> Result<Self, CdpError> {
        Self::build(config, true)
    }

    /// 与 [`new`](Self::new) 相同，但跳过 stealth 反检测脚本注入。
    pub fn new_without_stealth(config: BrowserConfig) -> Result<Self, CdpError> {
        Self::build(config, false)
    }

    /// 共享构造逻辑：连接或启动浏览器，绑定标签页，按需注入 stealth。
    fn build(config: BrowserConfig, inject_stealth: bool) -> Result<Self, CdpError> {
        let browser = Browser::connect_or_launch(config)?;
        let page = browser.tabs()?.into_iter().next().unwrap_or_else(|| {
            browser
                .new_tab()
                .expect("Failed to create a new tab. The browser may have been closed")
        });
        if inject_stealth {
            stealth::inject(&page)?;
        }
        Ok(Self { browser, page })
    }

    /// 仅连接已有浏览器（不启动），绑定当前标签页。地址如 `"127.0.0.1:9222"` 或 `"http://127.0.0.1:9222"`。
    pub fn connect(endpoint: &str) -> Result<Self, CdpError> {
        let browser = Browser::connect(endpoint)?;
        let page = browser.tabs()?.into_iter().next().unwrap_or_else(|| {
            browser
                .new_tab()
                .expect("Failed to create a new tab. The browser may have been closed")
        });
        Ok(Self { browser, page })
    }

    /// 访问网址（与 DrissionPage `get(url)` 一致）
    pub fn get(&self, url: &str) -> Result<(), CdpError> {
        self.page.goto(url)
    }

    /// 刷新页面（与 DrissionPage `refresh()` 一致）
    pub fn refresh(&self) -> Result<(), CdpError> {
        self.page.reload()
    }

    /// 后退（与 DrissionPage `back()` 一致）
    pub fn back(&self) -> Result<(), CdpError> {
        self.page.back()
    }

    /// 前进（与 DrissionPage `forward()` 一致）
    pub fn forward(&self) -> Result<(), CdpError> {
        self.page.forward()
    }

    /// 页面标题（与 DrissionPage `title` 属性一致）
    pub fn title(&self) -> Result<String, CdpError> {
        self.page.title()
    }

    /// 当前 URL（与 DrissionPage `url` 属性一致）
    pub fn url(&self) -> Result<String, CdpError> {
        self.page.url()
    }

    /// 整页 HTML（与 DrissionPage `html` 属性一致）
    pub fn html(&self) -> Result<String, CdpError> {
        self.page.html()
    }

    /// 执行 JavaScript，返回 CDP 的 result（与 DrissionPage `run_js(script)` 一致）
    pub fn run_js(&self, script: &str) -> Result<Value, CdpError> {
        self.page.run_js(script)
    }

    /// 执行 JavaScript（可为 async），等待 Promise 解析后返回结果；适用于 fetch 等异步表达式
    pub fn run_js_await(&self, script: &str) -> Result<Value, CdpError> {
        self.page.run_js_await(script)
    }

    /// 按定位器查找单个元素，index 为 1-based（与 DrissionPage `ele(locator, index=1)` 一致）
    pub fn ele(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        self.page.ele(locator)
    }

    /// 按定位器查找多个元素（与 DrissionPage `eles(locator)` 一致）
    pub fn eles(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        self.page.eles(locator)
    }

    /// 点击定位器匹配的第一个元素（与 DrissionPage `click(locator)` 一致）
    pub fn click(&self, locator: &str) -> Result<(), CdpError> {
        self.page.click(locator)
    }

    /// 向定位器匹配的第一个元素输入文本（与 DrissionPage `input(locator, text)` 一致）
    pub fn input(&self, locator: &str, text: &str) -> Result<(), CdpError> {
        self.page.input(locator, text)
    }

    /// 等待定位器匹配到元素（与 DrissionPage `wait.ele_loaded()` 一致）
    pub fn wait(&self, locator: &str, timeout: Duration) -> Result<Element, CdpError> {
        self.page.wait(locator, timeout)
    }

    /// 获取当前页 cookies（与 DrissionPage `cookies()` 一致）
    pub fn cookies(&self, urls: Option<&[String]>) -> Result<Vec<Cookie>, CdpError> {
        self.page.cookies(urls)
    }

    /// 截屏并保存（与 DrissionPage `get_screenshot(path=...)` 一致）
    pub fn screenshot(&self, path: &str) -> Result<(), CdpError> {
        self.page.screenshot(path)
    }

    /// 新建标签页并返回 Page；可选 url 则在新标签页中打开（与 DrissionPage `new_tab(url)` 一致）
    pub fn new_tab(&self, url: Option<&str>) -> Result<Page, CdpError> {
        let tab = self.browser.new_tab()?;
        if let Some(u) = url {
            tab.goto(u)?;
        }
        Ok(tab)
    }

    pub fn tabs(&self) -> Result<Vec<Page>, CdpError> {
        self.browser.tabs()
    }

    /// 关闭当前标签页（与 DrissionPage `close()` 一致）
    pub fn close(&self) -> Result<(), CdpError> {
        self.page.close()
    }

    /// 关闭当前标签页（更明确的命名）
    pub fn close_tab(&self) -> Result<(), CdpError> {
        self.page.close()
    }

    /// 关闭浏览器（仅对启动的新浏览器实例有效，会结束 Chrome 进程）
    pub fn close_browser(&mut self) {
        self.browser.close()
    }

    /// 获取底层浏览器引用（与 DrissionPage `browser` 属性一致）
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// 获取当前标签页引用（与 DrissionPage 的 tab 一致，用于多 Tab 时拿到 Page 操作）
    pub fn tab(&self) -> &Page {
        &self.page
    }

    /// 取出浏览器与当前标签页的所有权（会话缓存、跨模块持有等场景）
    pub fn into_parts(self) -> (Browser, Page) {
        (self.browser, self.page)
    }

    pub fn get_iframe(&self, locator: &str) -> Result<Option<Frame>, CdpError> {
        self.page.get_frame(locator)
    }

    pub fn get_iframes(&self, locator: Option<&str>) -> Result<Vec<Frame>, CdpError> {
        self.page.get_frames(locator)
    }

    // ==================== 网络数据包监听 ====================

    /// 启动网络数据包监听器，返回 [Listener] 用于接收请求/响应数据。
    ///
    /// 监听器通过独立的 CDP 连接接收 Network 事件，不影响页面的正常操作。
    ///
    /// # 基本用法
    ///
    /// ```no_run
    /// use rust_drission::ChromiumPage;
    /// use rust_drission::BrowserConfig;
    /// use std::time::Duration;
    ///
    /// let page = ChromiumPage::new(BrowserConfig::new()).unwrap();
    /// let mut listener = page.listen().unwrap();
    ///
    /// page.get("https://www.example.com").unwrap();
    ///
    /// // 阻塞等待下一个数据包（带超时）
    /// while let Some(packet) = listener.wait(Duration::from_secs(5)).unwrap() {
    ///     println!("{} {} -> {} {}",
    ///         packet.request.method,
    ///         packet.request.url,
    ///         packet.response.status.unwrap_or(0),
    ///         packet.resource_type.as_deref().unwrap_or("-")
    ///     );
    /// }
    /// ```
    pub fn listen(&self) -> Result<Listener, CdpError> {
        self.page.listen()
    }

    /// 启动监听并按条件过滤：只保留 URL 包含 `url_contains` 的数据包。
    ///
    /// ```no_run
    /// use rust_drission::ChromiumPage;
    /// use rust_drission::BrowserConfig;
    /// use std::time::Duration;
    ///
    /// let page = ChromiumPage::new(BrowserConfig::new()).unwrap();
    /// let mut listener = page.listen_url("api/data").unwrap();
    ///
    /// page.get("https://example.com").unwrap();
    ///
    /// if let Some(packet) = listener.wait(Duration::from_secs(10)).unwrap() {
    ///     println!("捕获到 API 请求: {}", packet.request.url);
    ///     if let Some(body) = &packet.body {
    ///         println!("响应体: {} bytes", body.len());
    ///     }
    /// }
    /// ```
    pub fn listen_url(&self, url_contains: &str) -> Result<Listener, CdpError> {
        let listener = self.page.listen()?;
        Ok(Listener::filter_url(listener, url_contains.to_string()))
    }

    /// 启动监听并按资源类型过滤（如 "XHR", "Fetch", "Document", "Script" 等）。
    ///
    /// ```no_run
    /// use rust_drission::ChromiumPage;
    /// use rust_drission::BrowserConfig;
    /// use std::time::Duration;
    ///
    /// let page = ChromiumPage::new(BrowserConfig::new()).unwrap();
    /// let mut listener = page.listen_resource_type("XHR").unwrap();
    ///
    /// page.get("https://example.com").unwrap();
    ///
    /// while let Some(packet) = listener.wait(Duration::from_secs(5)).unwrap() {
    ///     println!("XHR: {} {}", packet.request.method, packet.request.url);
    /// }
    /// ```
    pub fn listen_resource_type(&self, resource_type: &str) -> Result<Listener, CdpError> {
        let listener = self.page.listen()?;
        Ok(Listener::filter_resource_type(listener, resource_type.to_string()))
    }

    /// 从已有监听器持续收集数据包到 `Vec`，直到超时或闭包返回 `false`。
    ///
    /// 使用前需先调用 `page.listen()` 启动监听器，然后导航页面，最后收集。
    ///
    /// ```no_run
    /// use rust_drission::ChromiumPage;
    /// use rust_drission::BrowserConfig;
    /// use std::time::Duration;
    ///
    /// let page = ChromiumPage::new(BrowserConfig::new()).unwrap();
    ///
    /// // 先启动监听（阻塞直到就绪）
    /// let listener = page.listen().unwrap();
    /// // 再导航
    /// page.get("https://example.com").unwrap();
    /// // 最后收集数据包
    /// let packets = page.listen_collect(
    ///     &listener,
    ///     Duration::from_secs(5),
    ///     |packet| {
    ///         println!("{} {}", packet.request.method, packet.request.url);
    ///         true  // 返回 true 继续，false 停止
    ///     },
    /// ).unwrap();
    ///
    /// println!("共收集 {} 个数据包", packets.len());
    /// ```
    pub fn listen_collect<F>(
        &self,
        listener: &Listener,
        timeout: Duration,
        on_packet: F,
    ) -> Result<Vec<DataPacket>, CdpError>
    where
        F: FnMut(&DataPacket) -> bool,
    {
        listener.collect(timeout, on_packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time assertion: new_without_stealth exists with the expected
    /// signature `fn(BrowserConfig) -> Result<ChromiumPage, CdpError>`.
    #[test]
    fn chromium_page_new_without_stealth_has_expected_signature() {
        fn assert_signature(f: fn(BrowserConfig) -> Result<ChromiumPage, CdpError>) {
            let _ = f;
        }
        assert_signature(ChromiumPage::new_without_stealth);
    }

    /// Compile-time assertion: ChromiumPage::new must keep this exact signature.
    /// If this test compiles, the signature is verified.
    #[test]
    fn chromium_page_new_keeps_existing_signature() {
        const NEW_SIG: Option<fn(BrowserConfig) -> Result<ChromiumPage, CdpError>> =
            Some(ChromiumPage::new);
        let _ = NEW_SIG;
    }
}
