# rust_drission Usage Guide

This guide is for users of the library, not contributors. In most cases, you should start with `ChromiumPage`, because it combines browser control and the current tab into one practical entry point.

## 1. What it is good for

`rust_drission` is useful when you need to:

- launch or connect to Chrome / Chromium
- open pages and navigate
- find elements with DrissionPage-style locators
- click, type, scroll, wait, and take screenshots
- run JavaScript or raw CDP commands
- manage cookies, `localStorage`, and `sessionStorage`
- listen to requests and responses
- work with same-origin iframes

If your goal is “use DrissionPage-like browser automation from Rust”, `ChromiumPage` should be your default starting point.

## 2. Installation

```toml
[dependencies]
rust_drission = "0.2"
```

## 3. Minimal example

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

## 4. Two ways to start

### 4.1 Launch a new Chrome instance

This is the most common workflow.

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

Common configuration methods:

- `chrome_path(...)`
- `user_data_dir(...)`
- `headless(true)`
- `set_local_port(9222)`
- `incognito(true)`
- `mute(true)`
- `no_imgs(true)`
- `set_user_agent(...)`
- `set_proxy(...)`

### 4.2 Connect to an existing Chrome

Start Chrome manually with remote debugging enabled:

```text
chrome --remote-debugging-port=9222
```

Then connect:

```rust
use rust_drission::ChromiumPage;

let page = ChromiumPage::connect("127.0.0.1:9222")?;
```

If you want “connect if available, otherwise launch”, keep using `ChromiumPage::new(BrowserConfig::new().set_local_port(9222))`.

## 5. Locator syntax

`rust_drission` keeps locator strings close to DrissionPage.

### 5.1 Supported prefixes

- `css:.btn`
- `xpath://div[@id='main']`
- `text:Sign in`
- `attr:data-id=123`
- `id:submit`
- `class:card`
- `tag:input`

### 5.2 Bare CSS also works

These are automatically parsed as CSS selectors:

- `#kw`
- `.search-btn`
- `input[name='q']`
- `div.result a`

### 5.3 Examples

```rust
page.ele("css:.login-btn")?;
page.ele("text:Login")?;
page.ele("id:username")?;
page.eles("tag:a")?;
page.ele("xpath://button[contains(., 'Submit')]")?;
```

## 6. Common `ChromiumPage` operations

### 6.1 Navigation and page info

```rust
page.get("https://example.com")?;
page.refresh()?;
page.back()?;
page.forward()?;

let title = page.title()?;
let url = page.url()?;
let html = page.html()?;
```

### 6.2 Find elements

```rust
let one = page.ele("css:.item")?;
let many = page.eles("css:.item")?;
```

Important:

- `ele()` returns `Result<Option<Element>, CdpError>`
- `eles()` returns `Result<Vec<Element>, CdpError>`
- when an element is not found, `ele()` returns `Ok(None)`

### 6.3 Direct page actions

```rust
page.click("text:Login")?;
page.input("css:input[name='keyword']", "rust")?;
page.screenshot("page.png")?;
```

### 6.4 Wait for an element

```rust
use std::time::Duration;

let search_box = page.wait("css:input[name='q']", Duration::from_secs(10))?;
search_box.input("rust drission")?;
```

## 7. Element operations

Once you have an `Element`, you can inspect it or interact with it.

```rust
if let Some(ele) = page.ele("css:a.docs")? {
    println!("text = {}", ele.text()?);
    println!("href = {}", ele.attr("href")?);
    println!("html = {}", ele.html()?);
    println!("visible = {}", ele.is_displayed()?);
}
```

### 7.1 Frequently used element methods

- `text()`
- `html()` / `inner_html()`
- `attr("href")`
- `property("value")`
- `click()`
- `input("...")`
- `clear()`
- `focus()`
- `hover()` / `hover_at(...)`
- `screenshot("a.png")`
- `scroll_into_view()`
- `check(false)` to check a checkbox/radio
- `check(true)` to uncheck it
- `select("Visible Text", true)` for `<select>` by text
- `select("option_value", false)` for `<select>` by value

### 7.2 Child and neighbor traversal

```rust
if let Some(card) = page.ele("css:.card")? {
    let title = card.element("css:h2")?;
    let parent = card.parent(1)?;
    let next = card.next()?;
    let children = card.children()?;
}
```

## 8. Run JavaScript

### 8.1 Page-level JS

```rust
let result = page.run_js("document.title")?;
println!("{result:#?}");
```

### 8.2 Await async JS

```rust
let data = page.run_js_await(
    "fetch('https://httpbin.org/get').then(r => r.json())"
)?;
println!("{data:#?}");
```

### 8.3 Run JS on an element

```rust
if let Some(button) = page.ele("css:button")? {
    let result = button.run_js("return this.innerText;")?;
    println!("{result:#?}");
}
```

## 9. Advanced page APIs

`ChromiumPage` covers the common workflow. When you need more control, grab the underlying `Page`:

```rust
let tab = page.tab();
```

### 9.1 Visibility waits

```rust
use std::time::Duration;

tab.wait_visible("css:.loaded", Duration::from_secs(10))?;
tab.wait_hidden("css:.loading", Duration::from_secs(10))?;
```

### 9.2 Scrolling and viewport info

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

### 9.4 Cookies

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

### 9.6 Clear browser-side state

```rust
tab.clear_cache(true, true, true, true)?;
```

### 9.7 Send raw CDP commands

```rust
let version = tab.run_cdp("Browser.getVersion", None)?;
println!("{version:#?}");
```

## 10. Multi-tab usage

You can create a new tab directly from `ChromiumPage`:

```rust
let new_tab = page.new_tab(Some("https://www.rust-lang.org"))?;
println!("new tab title = {}", new_tab.title()?);
```

For fuller tab management, use `page.browser()`:

```rust
let browser = page.browser();

let ids = browser.tab_ids()?;
println!("tab ids = {ids:#?}");

let latest = browser.latest_tab()?;
println!("latest title = {}", latest.title()?);
```

Available browser-level tab APIs include:

- `tabs()`
- `tab_ids()`
- `tabs_count()`
- `latest_tab()`
- `get_tab(...)`
- `get_tabs(...)`
- `activate_tab(...)`
- `close_tabs(...)`

## 11. Listen to network traffic

`ChromiumPage` provides four network listening methods. Always start the listener **before** navigating to avoid missing events.

### 11.1 Basic listening

```rust
use std::time::Duration;

let listener = page.listen()?;
page.get("https://example.com")?;

while let Some(packet) = listener.wait(Duration::from_secs(5))? {
    println!("{} {} → {} ({})",
        packet.request.method,
        packet.request.url,
        packet.response.status.unwrap_or(0),
        packet.resource_type.as_deref().unwrap_or("-")
    );
    if let Some(body) = &packet.body {
        println!("  body: {} bytes", body.len());
    }
}
```

### 11.2 Filter by URL keyword

```rust
let url_listener = page.listen_url("api/data")?;
page.get("https://example.com")?;

if let Some(pkt) = url_listener.wait(Duration::from_secs(10))? {
    println!("API request: {} → {}", pkt.request.url, pkt.response.status.unwrap_or(0));
    if let Some(body) = &pkt.body {
        let text = String::from_utf8_lossy(body);
        println!("Response: {}", text);
    }
}
```

### 11.3 Filter by resource type

CDP resource types include: `Document`, `XHR`, `Fetch`, `Script`, `Stylesheet`, `Image`, `Other`, etc.

```rust
let fetch_listener = page.listen_resource_type("Fetch")?;
page.run_js_await("fetch('https://httpbin.org/get').then(r => r.json())")?;

if let Some(pkt) = fetch_listener.wait(Duration::from_secs(10))? {
    println!("Fetch: {} → {}", pkt.request.url, pkt.response.status.unwrap_or(0));
}
```

### 11.4 Batch collect

```rust
let listener = page.listen()?;
page.get("https://example.com")?;

let packets = page.listen_collect(&listener, Duration::from_secs(5), |pkt| {
    println!("  collecting: {} {}", pkt.request.method, pkt.request.url);
    true  // return false to stop early
})?;

println!("collected {} packets", packets.len());
```

### 11.5 Listener methods

- `listener.wait(timeout)` — block until a matching packet arrives or timeout
- `listener.wait_one()` — block indefinitely until a matching packet arrives
- `listener.try_recv()` — non-blocking, returns immediately
- `listener.collect(timeout, callback)` — batch collect with callback control
- `listener.filter_url(pattern)` — chain a URL filter onto an existing listener
- `listener.filter_resource_type(rt)` — chain a resource type filter

Notes:

- `listen()` blocks until the background thread is ready (connected + Network.enable)
- Each listener uses an independent CDP connection, so it does not interfere with page operations
- `page.tab().listen()` also works if you have a `Page` reference directly

## 12. Work with iframes

`rust_drission` treats same-origin iframes as a practical wrapper object.

```rust
if let Some(frame) = page.tab().get_frame("css:iframe")? {
    if let Some(input) = frame.ele("css:input")? {
        input.input("inside iframe")?;
    }
}
```

You can also get all same-origin iframes:

```rust
let frames = page.tab().get_frames(None)?;
println!("frame count = {}", frames.len());
```

Important:

- `Frame` is mainly intended for same-origin iframes
- cross-origin iframes are constrained by the browser security model

## 13. Copy-ready patterns

### 13.1 Fill a search box and submit

```rust
use std::time::Duration;

page.get("https://www.baidu.com")?;
let input = page.wait("css:#kw", Duration::from_secs(10))?;
input.input("rust drission")?;
page.click("css:#su")?;
```

### 13.2 Wait for content, then capture

```rust
use std::time::Duration;

page.get("https://example.com")?;
page.tab().wait_visible("css:body", Duration::from_secs(10))?;
page.screenshot("loaded.png")?;
```

### 13.3 Watch for an API response

```rust
use std::time::Duration;

let listener = page.listen_url("/api/")?;
page.get("https://example.com")?;

if let Some(packet) = listener.wait(Duration::from_secs(10))? {
    println!("api status = {:?}", packet.response.status);
    if let Some(body) = &packet.body {
        println!("api body = {}", String::from_utf8_lossy(body));
    }
}
```

## 14. Practical advice

- Start with `ChromiumPage`
- Use `page.tab()` only when you need lower-level page APIs
- On dynamic pages, prefer `wait(...)` and `wait_visible(...)`
- For optional elements, call `ele(...)` and handle `Option`
- Prefer stable selectors such as `css:`, `id:`, and `attr:`
- Use `user_data_dir(...)` when you need to keep login state

## 15. Frequently important details

### 15.1 Why does `ele()` not fail when the selector misses?

Because the API is designed to return `Ok(None)` for “not found”, which makes conditional logic easier.

### 15.2 When should I use `page.tab()`?

Use it when you need features such as:

- `wait_visible`
- `wait_hidden`
- `run_cdp`
- `local_storage`
- `session_storage`
- `clear_cache`
- `get_frame`

### 15.3 Why can I not access cross-origin iframe content the same way?

Because the browser's same-origin policy limits direct access to cross-origin frames. That is a platform constraint, not just a library choice.
