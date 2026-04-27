# rust_drission

`rust_drission` is a Rust browser automation library built on Chrome DevTools Protocol (CDP). Its public API is intentionally close to DrissionPage, and the recommended entry point for most usage is `ChromiumPage`.

`rust_drission` 是一个基于 Chrome DevTools Protocol (CDP) 的 Rust 浏览器自动化库。它的公开 API 尽量贴近 DrissionPage，大多数场景建议直接从 `ChromiumPage` 开始。

## Why `ChromiumPage`

- One entry point for launch/connect, navigation, element lookup, JS execution, screenshots, cookies, and tab access
- DrissionPage-style locator strings such as `css:`, `xpath:`, `text:`, `id:`, `class:`, `tag:`
- Supports direct JS/CDP calls when the high-level API is not enough
- Keeps advanced capabilities available through `page.tab()` and `page.browser()`

## Installation

```toml
[dependencies]
rust_drission = "0.1.5"
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
- `listen()` is available on `Page`, so with `ChromiumPage` you usually call `page.tab().listen()?`.
- Element lookup methods often return `Result<Option<Element>, CdpError>`. Handle the `None` case explicitly.
