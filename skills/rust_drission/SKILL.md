---
name: rust_drission
description: Usage-oriented guidance for browser automation with rust_drission. Prefer ChromiumPage, use real public APIs, and focus on runnable user workflows instead of internal implementation details.
---

# rust_drission Skill

Use this skill when the task is about operating `rust_drission` as a user of the library.

本技能面向 `rust_drission` 的使用方式，不面向库内部开发。

## Goal

- Prefer `ChromiumPage` as the default entry point
- Explain or generate code with the current public API only
- Focus on runnable examples, recipes, and user workflows
- Avoid diving into internal modules unless they are needed to explain usage

## Default Guidance

1. Start from `ChromiumPage`
   - Use `ChromiumPage::new(BrowserConfig::new()...)` for the common launch flow
   - Use `ChromiumPage::connect("127.0.0.1:9222")` when the browser is already running

2. Treat `ChromiumPage` as the high-level surface
   - Navigation: `get`, `refresh`, `back`, `forward`
   - Page info: `title`, `url`, `html`
   - Element lookup: `ele`, `eles`
   - Direct actions: `click`, `input`, `screenshot`
   - JS: `run_js`, `run_js_await`
   - Network: `listen`, `listen_url`, `listen_resource_type`, `listen_collect`
   - Cookies: `cookies`

3. Drop to `page.tab()` only for lower-level page features
   - `wait_visible`, `wait_hidden`, `wait_element`
   - `run_cdp`
   - `get_frame`, `get_frames`
   - storage and cache APIs
   - scroll, scroll_to_top, scroll_to_bottom, scroll_by, rect
   - stop_loading, handle_alert, wait_alert
   - set_cookie, delete_cookie, evaluate, active_ele

4. Use `Element` for follow-up interactions
   - `text`, `attr`, `html`, `property`, `text_content`, `tag`, `style`
   - `click`, `input`, `clear`, `focus`
   - `is_displayed`, `is_enabled`
   - `select`, `check`, `scroll_into_view`
   - `parent`, `children`, `prev`, `next`, `child`
   - `element`, `elements`, `element_text`, `element_exists`, `element_attr`, `element_texts`
   - `screenshot`, `remove`, `remove_attr`, `drag`, `drag_to`, `hover_at`, `value`

## Locator Rules

Prefer these locator forms in examples:

- `css:.btn`
- `xpath://div[@id='main']`
- `text:Login`
- `attr:data-id=123`
- `id:submit`
- `class:card`
- `tag:input`

Bare CSS is also valid:

- `#kw`
- `.search-btn`
- `input[name='q']`

## Response Style

When asked to show how to do something with `rust_drission`:

- Give a runnable code snippet first
- Default to `use rust_drission::{BrowserConfig, ChromiumPage, CdpError};`
- Mention when `ele()` returns `Option<Element>`
- Mention when a feature actually lives on `Page` and requires `page.tab()`
- Keep the explanation usage-oriented, not architecture-oriented

## Common Recipes

### Open a page and read its title

```rust
use rust_drission::{BrowserConfig, ChromiumPage, CdpError};

fn main() -> Result<(), CdpError> {
    let page = ChromiumPage::new(BrowserConfig::new().headless(false))?;
    page.get("https://example.com")?;
    println!("{}", page.title()?);
    Ok(())
}
```

### Find an element and type text

```rust
if let Some(input) = page.ele("css:input[name='q']")? {
    input.input("rust drission")?;
}
```

### Use a lower-level page API

```rust
use std::time::Duration;

page.tab().wait_visible("css:.loaded", Duration::from_secs(10))?;
```

### Listen to network traffic

```rust
use std::time::Duration;

// Basic: listen to all network packets
let listener = page.listen()?;
page.get("https://example.com")?;

if let Some(packet) = listener.wait(Duration::from_secs(10))? {
    println!("{}", packet.request.url);
}

// Filter by URL keyword
let url_listener = page.listen_url("api")?;
page.get("https://example.com")?;
if let Some(pkt) = url_listener.wait(Duration::from_secs(5))? {
    println!("API: {} → {}", pkt.request.url, pkt.response.status.unwrap_or(0));
}

// Filter by resource type
let fetch_listener = page.listen_resource_type("Fetch")?;

// Batch collect after navigation
let listener = page.listen()?;
page.get("https://example.com")?;
let packets = page.listen_collect(&listener, Duration::from_secs(5), |pkt| true)?;
println!("collected {} packets", packets.len());
```

Important: always call `listen()` **before** `page.get()` to avoid missing events.

## Constraints To Mention When Relevant

- The library targets Chrome / Chromium through CDP
- On macOS, the library auto-detects Chrome, Chrome Canary, Chromium, Edge, and Brave
- When using `user_data_dir`, the directory is automatically created if it doesn't exist (since v0.2.2). Singleton lock files from crashed sessions are also cleaned up before launch (since v0.2.1)
- Error messages are in English (since v0.2.0)
- `Frame` usage is mainly for same-origin iframes
- `listen()`, `listen_url()`, `listen_resource_type()`, and `listen_collect()` are directly on `ChromiumPage`
- Some advanced behaviors require dropping to `Page` or `Browser`
- `run_js` / `run_js_await` return `serde_json::Value`

## Documentation Pointers

- Usage guide: `reference/usage.md`

## What Not To Do

- Do not default to internal module explanations
- Do not invent APIs that are not publicly re-exported
- Do not write usage examples around internal development helpers
- Do not force users into `Browser` / `Page` unless `ChromiumPage` is insufficient
