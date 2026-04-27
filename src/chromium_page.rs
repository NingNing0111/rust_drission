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
    pub fn new(config: BrowserConfig) -> Result<Self, CdpError> {
        let browser = Browser::connect_or_launch(config)?;
        let page = browser.tabs()?.into_iter().next().unwrap_or_else(|| {
            // tabs 为空时创建新标签页；如果创建也失败，panic 是合理的（无法继续）
            browser
                .new_tab()
                .expect("无法创建新标签页，浏览器可能已关闭")
        });
        stealth::inject(&page)?;
        Ok(Self { browser, page })
    }

    /// 仅连接已有浏览器（不启动），绑定当前标签页。地址如 `"127.0.0.1:9222"` 或 `"http://127.0.0.1:9222"`。
    pub fn connect(endpoint: &str) -> Result<Self, CdpError> {
        let browser = Browser::connect(endpoint)?;
        let page = browser.tabs()?.into_iter().next().unwrap_or_else(|| {
            browser
                .new_tab()
                .expect("无法创建新标签页，浏览器可能已关闭")
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

    /// 关闭当前标签页（与 DrissionPage `close()` 一致）
    pub fn close(&self) -> Result<(), CdpError> {
        self.page.close()
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
}
