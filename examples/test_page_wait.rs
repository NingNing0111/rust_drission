//! 测试 page.wait / wait_element / wait_visible / wait_hidden / wait_network_idle
//!
//! 运行方式: cargo run --example test_page_wait

use std::time::Duration;

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_page_wait");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    // ==================== wait() 测试 ====================
    println!("=== wait() 测试 ===");

    // --- 元素已存在（立即返回） ---
    load_page(&mut page, "<div id='already-there'>Hello</div>")?;
    assert_some(
        "wait() 已存在元素立即返回",
        &page.wait("#already-there", Duration::from_secs(2)),
        |el| format!("tag={}", el.tag().unwrap_or_default()),
    );

    // --- 元素延迟出现（通过 run_js + setTimeout 创建） ---
    load_page(&mut page, "<div id='container'></div>")?;
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('p'); \
         el.id = 'late'; \
         el.textContent = 'arrived'; \
         document.getElementById('container').appendChild(el); \
         }, 500)",
    )?;
    assert_some(
        "wait() 元素延迟出现",
        &page.wait("#late", Duration::from_secs(5)),
        |el| format!("text={}", el.text().unwrap_or_default()),
    );

    // --- 超时（元素始终不存在） ---
    load_page(&mut page, "<div id='container'></div>")?;
    assert_none(
        "wait() 超时返回 None",
        &page.wait("#never-exists", Duration::from_millis(1000)),
    );

    // --- XPath 定位器 ---
    load_page(&mut page, "<div id='container'></div>")?;
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('span'); \
         el.className = 'marker-xp'; \
         el.textContent = 'found-by-xpath'; \
         document.getElementById('container').appendChild(el); \
         }, 500)",
    )?;
    assert_some(
        "wait() XPath 定位器",
        &page.wait("xpath://span[@class='marker-xp']", Duration::from_secs(5)),
        |el| format!("tag={}", el.tag().unwrap_or_default()),
    );

    // --- Text 定位器 ---
    load_page(&mut page, "<div id='container'></div>")?;
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('div'); \
         el.textContent = 'TEXT_MARKER_9X7K'; \
         document.getElementById('container').appendChild(el); \
         }, 500)",
    )?;
    assert_some(
        "wait() Text 定位器",
        &page.wait("text:TEXT_MARKER_9X7K", Duration::from_secs(5)),
        |el| format!("tag={}", el.tag().unwrap_or_default()),
    );

    // --- ID 定位器 ---
    load_page(&mut page, "<div id='container'></div>")?;
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('div'); \
         el.id = 'id-target'; \
         el.textContent = 'id-ok'; \
         document.getElementById('container').appendChild(el); \
         }, 500)",
    )?;
    assert_some(
        "wait() ID 定位器",
        &page.wait("id:id-target", Duration::from_secs(5)),
        |el| format!("text={}", el.text().unwrap_or_default()),
    );

    // ==================== wait_element() 测试 ====================
    println!("\n=== wait_element() 测试 ===");

    load_page(&mut page, "<div id='container'></div>")?;
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('h2'); \
         el.id = 'waited-default'; \
         el.textContent = 'default-timeout'; \
         document.getElementById('container').appendChild(el); \
         }, 400)",
    )?;
    assert_some(
        "wait_element() 默认 30s 超时（快速出现）",
        &page.tab().wait_element("#waited-default"),
        |el| format!("text={}", el.text().unwrap_or_default()),
    );

    // ==================== wait_visible() 测试 ====================
    println!("\n=== wait_visible() 测试 ===");

    // --- 元素初始隐藏，延迟变为可见 ---
    load_page(
        &mut page,
        "<div id='hide-show' style='display:none'>hidden then shown</div>",
    )?;
    page.run_js(
        "setTimeout(() => { \
         document.getElementById('hide-show').style.display = 'block'; \
         }, 500)",
    )?;
    assert_ok(
        "wait_visible() 隐藏→可见",
        &page
            .tab()
            .wait_visible("#hide-show", Duration::from_secs(5)),
        |el| format!("text={}", el.text().unwrap_or_default()),
    );

    // --- 元素一直隐藏 → 超时 ---
    load_page(
        &mut page,
        "<div id='stay-hidden' style='display:none'>never shown</div>",
    )?;
    assert_err(
        "wait_visible() 一直隐藏→超时",
        &page
            .tab()
            .wait_visible("#stay-hidden", Duration::from_millis(1500)),
    );

    // --- 元素不存在 → 超时 ---
    load_page(&mut page, "<div>nothing here</div>")?;
    assert_err(
        "wait_visible() 元素不存在→超时",
        &page
            .tab()
            .wait_visible("#no-such-el", Duration::from_millis(1000)),
    );

    // ==================== wait_hidden() 测试 ====================
    println!("\n=== wait_hidden() 测试 ===");

    // --- 元素初始可见，延迟隐藏 ---
    load_page(&mut page, "<div id='show-hide'>visible then hidden</div>")?;
    page.run_js(
        "setTimeout(() => { \
         document.getElementById('show-hide').style.display = 'none'; \
         }, 500)",
    )?;
    assert_ok_unit(
        "wait_hidden() 可见→隐藏",
        &page.tab().wait_hidden("#show-hide", Duration::from_secs(5)),
    );

    // --- 元素一直可见 → 超时 ---
    load_page(&mut page, "<div id='stay-visible'>always visible</div>")?;
    assert_err(
        "wait_hidden() 一直可见→超时",
        &page
            .tab()
            .wait_hidden("#stay-visible", Duration::from_millis(1500)),
    );

    // --- 元素本身就不存在 → 立即成功 ---
    load_page(&mut page, "<div>nothing here</div>")?;
    assert_ok_unit(
        "wait_hidden() 元素不存在→立即成功",
        &page
            .tab()
            .wait_hidden("#no-such-element", Duration::from_secs(2)),
    );

    // ==================== wait_network_idle() 测试 ====================
    println!("\n=== wait_network_idle() 测试 ===");

    load_page(&mut page, "<h1>idle page</h1>")?;
    assert_ok_unit(
        "wait_network_idle() 基本调用",
        &page.tab().wait_network_idle(),
    );

    println!("\n=== All page wait tests done ===");
    page.close_browser();
    Ok(())
}

// --- helpers ---

fn build_html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset='utf-8'></head><body>{}</body></html>",
        body
    )
}

fn to_data_url(html: &str) -> String {
    format!("data:text/html,{}", urlencoding(html))
}

/// 加载页面并等待 DOM 就绪
fn load_page(page: &mut ChromiumPage, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    page.get(&to_data_url(&build_html(body)))?;
    std::thread::sleep(Duration::from_millis(600));
    Ok(())
}

fn assert_ok<T, F>(desc: &str, result: &Result<T, rust_drission::CdpError>, fmt: F)
where
    F: FnOnce(&T) -> String,
{
    match result {
        Ok(v) => println!("  OK {}: {}", desc, fmt(v)),
        Err(e) => println!("  BAD {}: {}", desc, e),
    }
}

fn assert_ok_unit(desc: &str, result: &Result<(), rust_drission::CdpError>) {
    match result {
        Ok(()) => println!("  OK {}", desc),
        Err(e) => println!("  BAD {}: {}", desc, e),
    }
}

fn assert_some<T, F>(desc: &str, result: &Result<Option<T>, rust_drission::CdpError>, fmt: F)
where
    F: FnOnce(&T) -> String,
{
    match result {
        Ok(Some(v)) => println!("  OK {}: {}", desc, fmt(v)),
        Ok(None) => println!("  BAD {}: expected Some but got None", desc),
        Err(e) => println!("  BAD {}: {}", desc, e),
    }
}

fn assert_none<T>(desc: &str, result: &Result<Option<T>, rust_drission::CdpError>) {
    match result {
        Ok(None) => println!("  OK {}", desc),
        Ok(Some(_)) => println!("  BAD {}: expected None but got Some", desc),
        Err(e) => println!("  BAD {}: {}", desc, e),
    }
}

fn assert_err<T>(desc: &str, result: &Result<T, rust_drission::CdpError>) {
    match result {
        Ok(_) => println!("  BAD {}: expected Err but got Ok", desc),
        Err(e) => println!("  OK {}: {:?}", desc, e),
    }
}

fn urlencoding(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
        .replace(' ', "%20")
        .replace('"', "%22")
        .replace('#', "%23")
        .replace('<', "%3C")
        .replace('>', "%3E")
        .replace('[', "%5B")
        .replace(']', "%5D")
        .replace('{', "%7B")
        .replace('}', "%7D")
        .replace('|', "%7C")
        .replace('\\', "%5C")
        .replace('^', "%5E")
        .replace('`', "%60")
        .replace(';', "%3B")
        .replace('/', "%2F")
        .replace('?', "%3F")
        .replace(':', "%3A")
        .replace('@', "%40")
        .replace('=', "%3D")
        .replace('&', "%26")
}
