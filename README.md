# rust_drission

`rust_drission` is a Rust browser automation library built on Chrome DevTools Protocol (CDP). Its public API is intentionally close to DrissionPage, and the recommended entry point for most usage is `ChromiumPage`.

`rust_drission` 是一个基于 Chrome DevTools Protocol (CDP) 的 Rust 浏览器自动化库。它的公开 API 尽量贴近 DrissionPage，大多数场景建议直接从 `ChromiumPage` 开始。

## Why `ChromiumPage`

- One entry point for launch/connect, navigation, element lookup, JS execution, screenshots, cookies, and tab access
- DrissionPage-style locator strings such as `css:`, `xpath:`, `text:`, `id:`, `class:`, `tag:`
- Supports direct JS/CDP calls when the high-level API is not enough
- Keeps advanced capabilities available through `page.tab()` and `page.browser()`
- Built-in network traffic listening with URL/resource-type filtering and batch collection

## Installation

```toml
[dependencies]
rust_drission = "0.2"
```

## Quick Start

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
        println!("h1 = {}", h1.text()?);
    }

    page.screenshot("example.png")?;
    Ok(())
}
```

## Common Patterns

### Launch a new Chrome

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

### Connect to an existing Chrome

Chrome must already be started with remote debugging enabled, for example:

```text
chrome --remote-debugging-port=9222
```

Then connect:

```rust
use rust_drission::ChromiumPage;

let page = ChromiumPage::connect("127.0.0.1:9222")?;
```

### Locate and operate on elements

```rust
page.input("css:input[name='q']", "rust cdp")?;
page.click("text:Search")?;

if let Some(button) = page.ele("id:submit")? {
    println!("enabled = {}", button.is_enabled()?);
}
```

### Listen to network traffic

```rust
use std::time::Duration;

// Start listener first (blocks until ready), then navigate
let listener = page.listen()?;
page.get("https://example.com")?;

// Receive all packets
while let Some(pkt) = listener.wait(Duration::from_secs(5))? {
    println!("{} {} → {}", pkt.request.method, pkt.request.url, pkt.response.status.unwrap_or(0));
}

// Or filter by URL keyword
let url_listener = page.listen_url("api")?;
page.get("https://example.com")?;
if let Some(pkt) = url_listener.wait(Duration::from_secs(10))? {
    println!("API response: {:?}", pkt.body);
}

// Or filter by resource type (XHR, Fetch, Document, Script, etc.)
let fetch_listener = page.listen_resource_type("Fetch")?;

// Or batch collect after navigation
let listener = page.listen()?;
page.get("https://example.com")?;
let packets = page.listen_collect(&listener, Duration::from_secs(5), |pkt| true)?;
println!("collected {} packets", packets.len());
```

### Run JavaScript

```rust
let result = page.run_js("document.title")?;
let async_result = page.run_js_await("fetch('https://httpbin.org/get').then(r => r.json())")?;
```

### Use advanced page APIs

```rust
use std::time::Duration;

let tab = page.tab();
tab.wait_visible("css:.result", Duration::from_secs(10))?;
tab.set_local_storage("token", "demo-value")?;
```

## Documentation

- Chinese guide: [docs/usage.zh-CN.md](docs/usage.zh-CN.md)
- English guide: [docs/usage.en.md](docs/usage.en.md)
- Skill guide for usage-oriented prompting: [skills/rust_drission/SKILL.md](skills/rust_drission/SKILL.md)

## Notes

- `ChromiumPage` is the recommended starting point. Use `page.tab()` or `page.browser()` only when you need lower-level control.
- `Frame` support is aimed at same-origin iframes.
- `listen()` is available directly on `ChromiumPage`, as well as `listen_url()`, `listen_resource_type()`, and `listen_collect()`.
- Always start the listener **before** navigating, so no network events are missed: `let l = page.listen()?; page.get("...")?;`.
- Element lookup methods often return `Result<Option<Element>, CdpError>`. Handle the `None` case explicitly.

## Changelog

### v0.2.2

- Fix: when a custom `user_data_dir` is specified via `BrowserConfig::user_data_dir()`, the directory is now auto-created before Chrome starts. Previously it was only created for auto-generated temp paths, which could cause Chrome to show a "Cannot perform read/write operations" system dialog on Windows when the directory didn't exist.

### v0.2.1

- Fix: clean up Chrome Singleton lock files (`SingletonLock`, `SingletonCookie`, `SingletonSocket`) in `user_data_dir` before browser launch, preventing startup failures when a previous browser instance crashed or was killed unexpectedly

### v0.2.0

- All error messages changed from Chinese to English for better internationalization
- macOS: auto-detect Chrome, Chrome Canary, Chromium, Edge, and Brave browser paths
- Fixed element lookup fallback logic to avoid false errors when main scan returns no results
- Added 5 runnable API test examples (navigation, element actions, element queries, browser tabs)
- Cross-platform example paths (no longer Windows-only)
