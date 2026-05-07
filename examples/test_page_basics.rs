//! 测试页面基础 API：导航、状态、JS执行、Cookie、Storage、截图、滚动
//!
//! 运行方式: cargo run --example test_page_basics

use rust_drission::{page::Cookie, BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_page_basics");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    // 构建带标题的测试页面
    let page1 = build_data_url("Page 1", "<h1>Page One</h1><p>First page content.</p>");
    let page2 = build_data_url("Page 2", "<h1>Page Two</h1><p>Second page content.</p>");
    let page3 = build_data_url(
        "Form Page",
        r#"<h1>Form Page</h1>
        <input id="text-input" type="text" placeholder="Enter text" />
        <textarea id="textarea"></textarea>
        <input id="checkbox" type="checkbox" />
        <select id="select-el"><option value="a">Option A</option><option value="b">Option B</option></select>
        <div id="scroll-container" style="height:200px;width:200px;overflow:auto;">
            <div style="height:2000px;width:2000px;">big content</div>
        </div>"#,
    );

    // ==================== 导航测试 ====================
    println!("=== 导航测试 ===");

    // get / goto
    page.get(&page1)?;
    println!("  OK get(): navigated to page1");

    // url
    let current_url = page.url()?;
    println!("  OK url(): {}", &current_url[..std::cmp::min(80, current_url.len())]);

    // title
    let title = page.title()?;
    assert_or_print(title == "Page 1", "title()", &title);

    // html
    let html = page.html()?;
    assert_or_print(
        html.contains("Page One"),
        "html() contains 'Page One'",
        &format!("{}...", &html[..std::cmp::min(60, html.len())]),
    );

    // 导航到 page2 构建历史
    page.get(&page2)?;
    let title2 = page.title()?;
    assert_or_print(title2 == "Page 2", "title() after get page2", &title2);

    // back
    page.back()?;
    let title_after_back = page.title()?;
    assert_or_print(
        title_after_back == "Page 1",
        "back() navigated to page1",
        &title_after_back,
    );

    // forward
    page.forward()?;
    let title_after_forward = page.title()?;
    assert_or_print(
        title_after_forward == "Page 2",
        "forward() navigated to page2",
        &title_after_forward,
    );

    // refresh / reload
    page.refresh()?;
    let title_after_refresh = page.title()?;
    assert_or_print(
        title_after_refresh == "Page 2",
        "refresh() keeps page2",
        &title_after_refresh,
    );

    // stop_loading (test on a page that never finishes loading 不太现实，简单调用验证不报错)
    page.tab().stop_loading()?;
    println!("  OK stop_loading(): no error");

    // 测试从无历史的页面 back/forward 不报错
    page.get(&page1)?;
    page.back()?; // no-op no history
    println!("  OK back() on fresh page: no error");
    page.forward()?; // no-op
    println!("  OK forward() on fresh page: no error");

    // ==================== JS 执行测试 ====================
    println!("\n=== JS 执行测试 ===");

    // run_js
    let result = page.run_js("1 + 2")?;
    let val = result.get("value").and_then(|v| v.as_i64()).unwrap_or(-1);
    assert_or_print(val == 3, "run_js('1 + 2') == 3", &val.to_string());

    // run_js_await
    let result = page.run_js_await("Promise.resolve(42)")?;
    let val = result.get("value").and_then(|v| v.as_i64()).unwrap_or(-1);
    assert_or_print(val == 42, "run_js_await('Promise.resolve(42)') == 42", &val.to_string());

    // evaluate
    let result = page.tab().evaluate("document.title")?;
    let val = result.get("value").and_then(|v| v.as_str()).unwrap_or("");
    assert_or_print(val == "Page 1", "evaluate('document.title')", val);

    // run_js with DOM manipulation
    let _ = page.run_js("document.body.setAttribute('data-test', 'js-ok')");
    let attr_val = page.tab().evaluate("document.body.getAttribute('data-test')")?;
    let attr_str = attr_val.get("value").and_then(|v| v.as_str()).unwrap_or("");
    assert_or_print(attr_str == "js-ok", "run_js setAttribute works", attr_str);

    // ==================== Scroll 测试 ====================
    println!("\n=== Scroll 测试 ===");

    // 导航到需要滚动的页面
    page.get(&page3)?;

    // scroll_to_top
    page.tab().scroll_to_top()?;
    println!("  OK scroll_to_top(): no error");

    // scroll_to_bottom
    page.tab().scroll_to_bottom()?;
    println!("  OK scroll_to_bottom(): no error");

    // scroll
    page.tab().scroll(100, 200)?;
    println!("  OK scroll(100, 200): no error");

    // scroll_by
    page.tab().scroll_by(0, 50)?;
    println!("  OK scroll_by(0, 50): no error");

    // rect
    let rect = page.tab().rect()?;
    let has_fields = rect.get("scrollWidth").is_some() && rect.get("viewportHeight").is_some();
    assert_or_print(has_fields, "rect() returns scroll/viewport info", &format!("{:?}", rect));

    // ==================== Cookie 测试 ====================
    println!("\n=== Cookie 测试 ===");

    // 设置 cookie
    let cookie = Cookie {
        name: "test_key".into(),
        value: "test_value".into(),
        domain: None,
        path: Some("/".into()),
    };
    page.tab().set_cookie(&cookie, None)?;
    println!("  OK set_cookie(): test_key=test_value");

    // 获取 cookies
    let cookies = page.cookies(None)?;
    let has_test = cookies.iter().any(|c| c.name == "test_key");
    assert_or_print(has_test, "cookies() contains test_key", &format!("{} cookies", cookies.len()));

    // 删除 cookie
    page.tab().delete_cookie("test_key", None)?;
    let cookies_after = page.cookies(None)?;
    let test_gone = !cookies_after.iter().any(|c| c.name == "test_key");
    assert_or_print(test_gone, "delete_cookie() removed test_key", &format!("{} cookies left", cookies_after.len()));

    // ==================== Storage 测试 ====================
    println!("\n=== SessionStorage 测试 ===");

    let tab = page.tab();

    // session_storage
    let before = tab.session_storage(Some("ss_key"))?;
    assert_or_print(before.is_none(), "session_storage nonexistent -> None", &format!("{:?}", before));

    tab.set_session_storage("ss_key", "ss_value")?;
    println!("  OK set_session_storage('ss_key', 'ss_value')");

    let after = tab.session_storage(Some("ss_key"))?;
    assert_or_print(after.as_deref() == Some("ss_value"), "session_storage('ss_key')", after.as_deref().unwrap_or("None"));

    // 读取全部
    let all = tab.session_storage(None)?;
    assert_or_print(all.is_some(), "session_storage(None) returns all", all.as_deref().unwrap_or("None"));

    // 删除单个
    tab.delete_session_storage(Some("ss_key"))?;
    let del = tab.session_storage(Some("ss_key"))?;
    assert_or_print(del.is_none(), "delete_session_storage removed key", &format!("{:?}", del));

    // 删除全部
    tab.set_session_storage("a", "1")?;
    tab.set_session_storage("b", "2")?;
    tab.delete_session_storage(None)?;
    let empty = tab.session_storage(Some("a"))?;
    assert_or_print(empty.is_none(), "delete_session_storage(None) clears all", &format!("{:?}", empty));

    println!("\n=== LocalStorage 测试 ===");

    // local_storage
    let before = tab.local_storage(Some("ls_key"))?;
    assert_or_print(before.is_none(), "local_storage nonexistent -> None", &format!("{:?}", before));

    tab.set_local_storage("ls_key", "ls_value")?;
    println!("  OK set_local_storage('ls_key', 'ls_value')");

    let after = tab.local_storage(Some("ls_key"))?;
    assert_or_print(after.as_deref() == Some("ls_value"), "local_storage('ls_key')", after.as_deref().unwrap_or("None"));

    // 读取全部
    let all_ls = tab.local_storage(None)?;
    assert_or_print(all_ls.is_some(), "local_storage(None) returns all", all_ls.as_deref().unwrap_or("None"));

    // 删除单个
    tab.delete_local_storage(Some("ls_key"))?;
    let del = tab.local_storage(Some("ls_key"))?;
    assert_or_print(del.is_none(), "delete_local_storage removed key", &format!("{:?}", del));

    // 删除全部
    tab.set_local_storage("x", "y")?;
    tab.delete_local_storage(None)?;
    let empty_ls = tab.local_storage(Some("x"))?;
    assert_or_print(empty_ls.is_none(), "delete_local_storage(None) clears all", &format!("{:?}", empty_ls));

    // ==================== clear_cache 测试 ====================
    println!("\n=== clear_cache 测试 ===");
    tab.clear_cache(true, true, true, true)?;
    println!("  OK clear_cache(all=true): no error");

    // ==================== Screenshot 测试 ====================
    println!("\n=== Screenshot 测试 ===");
    let ss_path = std::env::temp_dir().join("drission_test_page_screenshot.png");
    page.screenshot(&ss_path.to_string_lossy())?;
    let exists = ss_path.exists();
    let size = if exists {
        std::fs::metadata(&ss_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };
    assert_or_print(exists && size > 0, "screenshot() saved file", &format!("{} bytes", size));
    let _ = std::fs::remove_file(&ss_path);

    // ==================== active_ele / tab_id 测试 ====================
    println!("\n=== active_ele / tab_id 测试 ===");

    let tab_id = tab.tab_id();
    assert_or_print(!tab_id.is_empty(), "tab_id() non-empty", tab_id);

    let active = tab.active_ele()?;
    // 页面加载后通常 body 或 html 是 active element
    println!(
        "  OK active_ele(): {}",
        match &active {
            Some(el) => format!("tag={}", el.tag().unwrap_or_default()),
            None => "None (可能为 data: URL 限制)".into(),
        }
    );

    println!("\n=== All page basics tests done ===");
    page.close_browser();
    Ok(())
}

fn build_data_url(title: &str, body: &str) -> String {
    let html = format!(
        "<!DOCTYPE html><html><head><title>{}</title></head><body>{}</body></html>",
        title, body
    );
    format!("data:text/html,{}", urlencoding(&html))
}

fn assert_or_print(ok: bool, desc: &str, actual: &str) {
    if ok {
        println!("  OK {}: {}", desc, actual);
    } else {
        println!("  BAD {}: {}", desc, actual);
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
