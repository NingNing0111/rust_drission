# rust_drission 使用文档

本文档面向使用者，而不是开发者。默认推荐从 `ChromiumPage` 开始，因为它已经把浏览器实例和当前标签页整合在一起，适合大多数自动化场景。

## 1. 适用场景

`rust_drission` 适合这些任务：

- 启动或连接 Chrome / Chromium
- 打开网页、读取标题、执行跳转
- 用类似 DrissionPage 的定位语法查找元素
- 点击、输入、截图、滚动、等待元素出现
- 执行 JavaScript 或直接调用 CDP
- 操作 cookie、localStorage、sessionStorage
- 监听请求和响应
- 访问同源 iframe

如果你的目标只是“像 DrissionPage 一样快速控制浏览器”，优先使用 `ChromiumPage`。

## 2. 安装

```toml
[dependencies]
rust_drission = "0.1.5"
```

## 3. 最小示例

```rust
use rust_drission::{BrowserConfig, ChromiumPage, CdpError};

fn main() -> Result<(), CdpError> {
    let page = ChromiumPage::new(
        BrowserConfig::new()
            .headless(false)
            .set_local_port(9222),
    )?;

    page.get("https://example.com")?;

    println!("title = {}", page.title()?);
    println!("url = {}", page.url()?);

    if let Some(h1) = page.ele("css:h1")? {
        println!("h1 text = {}", h1.text()?);
    }

    page.screenshot("example.png")?;
    Ok(())
}
```

## 4. 两种启动方式

### 4.1 自动启动一个新的 Chrome

这是最常见的用法。

```rust
use rust_drission::{BrowserConfig, ChromiumPage};

let page = ChromiumPage::new(
    BrowserConfig::new()
        .chrome_path("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe")
        .user_data_dir("./data/profile")
        .set_local_port(9222)
        .headless(false),
)?;
```

常见配置项：

- `chrome_path(...)`：指定浏览器可执行文件
- `user_data_dir(...)`：指定用户数据目录，适合保留登录态
- `headless(true)`：无头模式
- `set_local_port(9222)`：指定调试端口
- `incognito(true)`：无痕模式
- `mute(true)`：静音
- `no_imgs(true)`：禁用图片加载
- `set_user_agent(...)`：设置 UA
- `set_proxy(...)`：设置代理

### 4.2 连接一个已经启动的 Chrome

先手动启动 Chrome，并开启远程调试端口：

```text
chrome --remote-debugging-port=9222
```

然后连接：

```rust
use rust_drission::ChromiumPage;

let page = ChromiumPage::connect("127.0.0.1:9222")?;
```

如果你想“有就连接，没有就启动”，可以继续用 `ChromiumPage::new(BrowserConfig::new().set_local_port(9222))`。

## 5. 推荐的定位语法

`rust_drission` 的定位字符串尽量贴近 DrissionPage。

### 5.1 支持的前缀

- `css:.btn`
- `xpath://div[@id='main']`
- `text:立即登录`
- `attr:data-id=123`
- `id:submit`
- `class:card`
- `tag:input`

### 5.2 可直接写成裸 CSS 的情况

下面这些会自动按 CSS 解析：

- `#kw`
- `.search-btn`
- `input[name='q']`
- `div.result a`

### 5.3 常用示例

```rust
page.ele("css:.login-btn")?;
page.ele("text:登录")?;
page.ele("id:username")?;
page.eles("tag:a")?;
page.ele("xpath://button[contains(., '提交')]")?;
```

## 6. `ChromiumPage` 常用操作

### 6.1 导航和页面信息

```rust
page.get("https://example.com")?;
page.refresh()?;
page.back()?;
page.forward()?;

let title = page.title()?;
let url = page.url()?;
let html = page.html()?;
```

### 6.2 查找元素

```rust
let one = page.ele("css:.item")?;
let many = page.eles("css:.item")?;
```

注意：

- `ele()` 返回 `Result<Option<Element>, CdpError>`
- `eles()` 返回 `Result<Vec<Element>, CdpError>`
- 找不到元素时，`ele()` 不会报错，而是返回 `Ok(None)`

### 6.3 直接操作页面

```rust
page.click("text:登录")?;
page.input("css:input[name='keyword']", "rust")?;
page.screenshot("page.png")?;
```

### 6.4 等待元素

```rust
use std::time::Duration;

let search_box = page.wait("css:input[name='q']", Duration::from_secs(10))?;
search_box.input("rust drission")?;
```

## 7. 元素操作

一旦拿到 `Element`，你可以继续读取属性或执行交互。

```rust
if let Some(ele) = page.ele("css:a.docs")? {
    println!("text = {}", ele.text()?);
    println!("href = {}", ele.attr("href")?);
    println!("html = {}", ele.html()?);
    println!("visible = {}", ele.is_displayed()?);
}
```

### 7.1 常用元素能力

- `text()`：读取元素文本
- `html()` / `inner_html()`：读取 HTML
- `attr("href")`：读取属性
- `property("value")`：读取 DOM 属性
- `click()`：点击
- `input("xxx")`：输入
- `clear()`：清空输入框
- `focus()`：聚焦
- `hover()` / `hover_at(...)`：悬停
- `screenshot("a.png")`：元素截图
- `scroll_into_view()`：滚动到可见区域
- `check(false)`：勾选 checkbox/radio
- `check(true)`：取消勾选
- `select("选项文本", true)`：按文本选择 `<select>` 选项
- `select("option_value", false)`：按 value 选择

### 7.2 子元素和邻接元素

```rust
if let Some(card) = page.ele("css:.card")? {
    let title = card.element("css:h2")?;
    let parent = card.parent(1)?;
    let next = card.next()?;
    let children = card.children()?;
}
```

## 8. 运行 JavaScript

### 8.1 页面级 JS

```rust
let result = page.run_js("document.title")?;
println!("{result:#?}");
```

### 8.2 等待异步 JS

```rust
let data = page.run_js_await(
    "fetch('https://httpbin.org/get').then(r => r.json())"
)?;
println!("{data:#?}");
```

### 8.3 在元素上执行 JS

```rust
if let Some(button) = page.ele("css:button")? {
    let result = button.run_js("return this.innerText;")?;
    println!("{result:#?}");
}
```

## 9. 进阶页面能力

`ChromiumPage` 主要提供最常用的高层 API。需要更细一点的能力时，取出底层 `Page`：

```rust
let tab = page.tab();
```

### 9.1 可见性等待

```rust
use std::time::Duration;

tab.wait_visible("css:.loaded", Duration::from_secs(10))?;
tab.wait_hidden("css:.loading", Duration::from_secs(10))?;
```

### 9.2 滚动和窗口信息

```rust
tab.scroll(0, 500)?;
tab.scroll_by(0, 300)?;
tab.scroll_to_top()?;
tab.scroll_to_bottom()?;

let rect = tab.rect()?;
println!("{rect:#?}");
```

### 9.3 Alert / Confirm / Prompt

```rust
tab.handle_alert(true, None)?;
let ok = tab.wait_alert(true, Some("hello"), Duration::from_secs(5))?;
println!("alert handled = {ok}");
```

### 9.4 Cookie

```rust
let cookies = tab.cookies(None)?;
for cookie in cookies {
    println!("{}={}", cookie.name, cookie.value);
}
```

### 9.5 Storage

```rust
tab.set_local_storage("token", "abc")?;
println!("{:?}", tab.local_storage(Some("token"))?);

tab.set_session_storage("session_id", "123")?;
println!("{:?}", tab.session_storage(Some("session_id"))?);
```

### 9.6 清理缓存

```rust
tab.clear_cache(true, true, true, true)?;
```

### 9.7 直接发 CDP 命令

```rust
let version = tab.run_cdp("Browser.getVersion", None)?;
println!("{version:#?}");
```

## 10. 多标签页

`ChromiumPage` 也可以直接开新标签页：

```rust
let new_tab = page.new_tab(Some("https://www.rust-lang.org"))?;
println!("new tab title = {}", new_tab.title()?);
```

如果要做更完整的标签页管理，使用 `page.browser()`：

```rust
let browser = page.browser();

let ids = browser.tab_ids()?;
println!("tab ids = {ids:#?}");

let latest = browser.latest_tab()?;
println!("latest title = {}", latest.title()?);
```

可用能力包括：

- `tabs()`
- `tab_ids()`
- `tabs_count()`
- `latest_tab()`
- `get_tab(...)`
- `get_tabs(...)`
- `activate_tab(...)`
- `close_tabs(...)`

## 11. 监听网络请求

网络监听能力挂在 `Page` 上，所以通常这样用：

```rust
use std::time::Duration;

let listener = page.tab().listen()?;
page.get("https://example.com")?;

if let Some(packet) = listener.wait(Duration::from_secs(10))? {
    println!("request url = {}", packet.request.url);
    println!("method = {}", packet.request.method);
    println!("response url = {}", packet.response.url);
    println!("status = {:?}", packet.response.status);
}
```

注意：

- `listen()` 需要 `Page` 带有浏览器调试端点信息
- 用 `ChromiumPage::new()`、`ChromiumPage::connect()`、`Browser::new_tab()` 等正常拿到的页面，一般可以直接监听

## 12. iframe 使用

`rust_drission` 支持把同源 iframe 当作普通对象来操作。

```rust
if let Some(frame) = page.tab().get_frame("css:iframe")? {
    if let Some(input) = frame.ele("css:input")? {
        input.input("inside iframe")?;
    }
}
```

你也可以获取所有同源 iframe：

```rust
let frames = page.tab().get_frames(None)?;
println!("frame count = {}", frames.len());
```

注意：

- `Frame` 主要面向同源 iframe
- 跨域 iframe 通常无法直接像同源 iframe 一样深入访问

## 13. 适合直接抄用的组合写法

### 13.1 搜索框输入并提交

```rust
use std::time::Duration;

page.get("https://www.baidu.com")?;
let input = page.wait("css:#kw", Duration::from_secs(10))?;
input.input("rust drission")?;
page.click("css:#su")?;
```

### 13.2 等待内容加载后截图

```rust
use std::time::Duration;

page.get("https://example.com")?;
page.tab().wait_visible("css:body", Duration::from_secs(10))?;
page.screenshot("loaded.png")?;
```

### 13.3 读取一个接口返回

```rust
use std::time::Duration;

let listener = page.tab().listen()?;
page.get("https://example.com")?;

while let Some(packet) = listener.wait(Duration::from_secs(3))? {
    if packet.request.url.contains("/api/") {
        println!("api status = {:?}", packet.response.status);
        break;
    }
}
```

## 14. 使用建议

- 优先用 `ChromiumPage`
- 找不到更高层 API 时，再用 `page.tab()` 调到底层 `Page`
- 对不稳定页面，优先使用 `wait(...)` / `wait_visible(...)`
- 对可能不存在的元素，优先 `ele(...)` 再判断 `Option`
- 对复杂站点，定位优先考虑稳定的 `css:`、`id:`、`attr:`
- 需要保留登录态时，设置 `user_data_dir(...)`

## 15. 常见注意事项

### 15.1 `ele()` 为什么没报错？

因为设计上找不到元素会返回 `Ok(None)`，这样更适合做条件判断。

### 15.2 什么时候该用 `page.tab()`？

当你需要下面这些能力时：

- `wait_visible`
- `wait_hidden`
- `listen`
- `run_cdp`
- `local_storage`
- `session_storage`
- `clear_cache`
- `get_frame`

### 15.3 `Frame` 为什么拿不到跨域 iframe 内容？

因为浏览器同源策略会限制跨域 frame 的直接访问，这不是 `rust_drission` 单独能绕开的常规能力。
