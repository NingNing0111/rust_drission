# ChromiumPage Stealth Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ChromiumPage::new(config)` inject the stealth script by default while adding a fully compatible opt-out constructor that skips injection.

**Architecture:** Keep the public API compatibility boundary at `ChromiumPage` by leaving `BrowserConfig` unchanged and adding one explicit opt-out constructor. Refactor the existing constructor logic into one private shared builder so the default and opt-out paths differ only in whether they call `stealth::inject(&page)`.

**Tech Stack:** Rust, cargo test, existing `Browser`, `Page`, `ChromiumPage`, and `stealth` modules.

---

## File structure

- Modify `src/chromium_page.rs`
  - Add a private shared constructor helper.
  - Keep `ChromiumPage::new(config)` as the default stealth-enabled entrypoint.
  - Add `ChromiumPage::new_without_stealth(config)` as the explicit opt-out entrypoint.
  - Leave `ChromiumPage::connect(endpoint)` unchanged.

- Modify `src/lib.rs`
  - Only if rustdoc examples or exported API docs need a short note about the new constructor.
  - Do not change re-exports unless required by the compiler.

- Test in `src/chromium_page.rs`
  - Add focused unit tests that verify the helper split at the API level without requiring real browser startup.
  - Prefer a compile-time signature assertion approach if runtime browser setup would make tests flaky.

### Task 1: Refactor ChromiumPage construction into one shared path

**Files:**
- Modify: `src/chromium_page.rs:27-50`
- Test: `src/chromium_page.rs` existing or new `#[cfg(test)]` module near the end of the file

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)]` module at the bottom of `src/chromium_page.rs` with a compile-time API assertion for the new constructor:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chromium_page_new_without_stealth_has_expected_signature() {
        let constructor: fn(BrowserConfig) -> Result<ChromiumPage, CdpError> =
            ChromiumPage::new_without_stealth;
        let _ = constructor;
    }
}
```

This should fail initially because `new_without_stealth` does not exist yet.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test chromium_page_new_without_stealth_has_expected_signature --lib`
Expected: FAIL with an error that `new_without_stealth` is not found on `ChromiumPage`.

- [ ] **Step 3: Write minimal implementation**

Replace the constructor section in `src/chromium_page.rs` with this shared structure:

```rust
impl ChromiumPage {
    /// 连接已有浏览器或启动新浏览器，并绑定当前标签页（与 DrissionPage `ChromiumPage(addr_or_opts)` 一致）。
    /// 若有已存在标签页则使用第一个，否则新建 about:blank 标签页。
    pub fn new(config: BrowserConfig) -> Result<Self, CdpError> {
        Self::build(config, true)
    }

    /// 连接已有浏览器或启动新浏览器，但不注入反检测脚本。
    /// 若有已存在标签页则使用第一个，否则新建 about:blank 标签页。
    pub fn new_without_stealth(config: BrowserConfig) -> Result<Self, CdpError> {
        Self::build(config, false)
    }

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
```

Do not change any other methods in this step.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test chromium_page_new_without_stealth_has_expected_signature --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/chromium_page.rs
git commit -m "feat: add ChromiumPage stealth opt-out constructor"
```

### Task 2: Prove the default constructor remains source-compatible

**Files:**
- Modify: `src/chromium_page.rs` test module added in Task 1
- Test: `src/chromium_page.rs` test module

- [ ] **Step 1: Write the failing test**

Extend the same test module with one more compile-time assertion for the unchanged default constructor:

```rust
#[test]
fn chromium_page_new_keeps_existing_signature() {
    let constructor: fn(BrowserConfig) -> Result<ChromiumPage, CdpError> = ChromiumPage::new;
    let _ = constructor;
}
```

If the refactor accidentally changes the signature, this test will fail.

- [ ] **Step 2: Run test to verify it fails when the contract is broken**

Run: `cargo test chromium_page_new_keeps_existing_signature --lib`
Expected: PASS immediately if the contract was preserved. If it fails, fix the constructor signature before continuing.

- [ ] **Step 3: Write minimal implementation**

No new production code is required if the test passes. If the test failed, correct `ChromiumPage::new` so it remains exactly:

```rust
pub fn new(config: BrowserConfig) -> Result<Self, CdpError>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test chromium_page_new_keeps_existing_signature --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/chromium_page.rs
git commit -m "test: lock ChromiumPage constructor compatibility"
```

### Task 3: Document the new opt-out constructor without changing existing guidance

**Files:**
- Modify: `src/chromium_page.rs:28-42`
- Modify: `src/lib.rs:5-17`
- Test: rustdoc snippets in `src/lib.rs` and `src/chromium_page.rs`

- [ ] **Step 1: Write the failing doc check**

Add or adjust rustdoc text so the default constructor remains the primary example and the opt-out path is mentioned explicitly. Use these exact doc lines:

In `src/chromium_page.rs`, update the constructor docs to include:

```rust
/// 连接已有浏览器或启动新浏览器，并绑定当前标签页（与 DrissionPage `ChromiumPage(addr_or_opts)` 一致）。
/// 默认会注入反检测脚本；若需关闭可使用 `ChromiumPage::new_without_stealth(config)`。
/// 若有已存在标签页则使用第一个，否则新建 about:blank 标签页。
```

In `src/lib.rs`, expand the quick-start narrative with:

```rust
//! 默认情况下 `ChromiumPage::new(BrowserConfig::new())` 会在页面初始化时注入反检测脚本。
//! 如需关闭注入，可使用 `ChromiumPage::new_without_stealth(BrowserConfig::new())`。
```

- [ ] **Step 2: Run doc tests or compile checks**

Run: `cargo test --doc`
Expected: PASS. If rustdoc formatting is broken by the new text, fix the comments before continuing.

- [ ] **Step 3: Write minimal implementation**

If needed, keep the code examples unchanged and only add the two documentation notes above. Do not replace the existing primary example using `ChromiumPage::new(BrowserConfig::new())`.

- [ ] **Step 4: Run verification**

Run: `cargo test --doc`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/chromium_page.rs src/lib.rs
git commit -m "docs: note ChromiumPage stealth defaults"
```

### Task 4: Run full verification for the compatibility-safe change

**Files:**
- Verify: `src/chromium_page.rs`
- Verify: `src/lib.rs`

- [ ] **Step 1: Run focused library tests**

Run: `cargo test --lib chromium_page`
Expected: PASS for the `chromium_page`-related unit tests.

- [ ] **Step 2: Run full library test suite**

Run: `cargo test --lib`
Expected: PASS

- [ ] **Step 3: Run full project test suite**

Run: `cargo test`
Expected: PASS, or if existing unrelated failures already exist, record exactly which tests are unrelated before stopping.

- [ ] **Step 4: Inspect the final diff**

Run: `git diff -- src/chromium_page.rs src/lib.rs`
Expected: only the shared constructor refactor, the new opt-out constructor, and the doc updates.

- [ ] **Step 5: Commit**

If Task 1-3 were committed separately as planned, do not create an extra code commit here. If any verification-triggered fixes were needed, commit only those fixes:

```bash
git add src/chromium_page.rs src/lib.rs
git commit -m "test: finalize ChromiumPage stealth integration"
```

## Self-review

- Spec coverage: covered default stealth injection in `ChromiumPage::new`, explicit opt-out via `new_without_stealth`, unchanged `BrowserConfig`, unchanged `connect(endpoint)`, and compatibility verification.
- Placeholder scan: no TBD/TODO placeholders remain.
- Type consistency: all tasks use `ChromiumPage::new(config: BrowserConfig) -> Result<Self, CdpError>` and `ChromiumPage::new_without_stealth(config: BrowserConfig) -> Result<Self, CdpError>` consistently.
