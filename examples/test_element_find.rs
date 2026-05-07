//! 测试 element/elements/ele/eles 等查找元素的功能
//! 重点验证：当页面没有匹配元素时，应返回 Ok(None)/Ok(vec![]) 而不是 Error

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_userdata");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    // 构建带 iframe 的测试页面
    // iframe 内容（同源 data: URL，CDP 可访问）
    let iframe_html = r#"<!DOCTYPE html><p id="iframe-para" class="iframe-content">Inside iframe</p>"#;
    let main_html = format!(
        r#"<!DOCTYPE html><html><body>
        <div id="app"><p class="hello" id="hello">Hello World</p></div>
        <iframe id="test-frame" src="{iframe_src}"></iframe>
        </body></html>"#,
        iframe_src = format!("data:text/html,{}", urlencoding(iframe_html))
    );
    page.get(&format!("data:text/html,{}", urlencoding(&main_html)))?;

    // 调试：打印页面 HTML，确认 iframe 元素存在
    println!("\n--- DEBUG: Page diagnostics ---");
    match page.html() {
        Ok(html) => println!("  Page HTML (first 500 chars): {}", &html[..std::cmp::min(500, html.len())]),
        Err(e) => println!("  Failed to get HTML: {}", e),
    }

    // 测试能否找到 iframe 标签
    let debug_locators = vec![
        "tag:iframe",
        "#test-frame",
        "iframe",
        "xpath://iframe[@id='test-frame']",
    ];
    for loc in &debug_locators {
        match page.ele(loc) {
            Ok(Some(el)) => match el.tag() {
                Ok(tag) => println!("  Found '{}': <{} id={:?}>", loc, tag, el.attr("id").unwrap_or_default()),
                Err(e) => println!("  '{}' found but tag() error: {}", loc, e),
            },
            Ok(None) => println!("  '{}' -> Ok(None)", loc),
            Err(e) => println!("  '{}' -> Error: {}", loc, e),
        }
    }

    // Get iframe via XPath to verify it works
    println!("\n--- get_iframe via XPath ---");
    match page.get_iframe("xpath://iframe[@id='test-frame']") {
        Ok(Some(frame)) => {
            println!("  Got frame via XPath!");
            match frame.element("#iframe-para") {
                Ok(Some(_)) => println!("  Frame.element(#iframe-para): Ok(Some)"),
                Ok(None) => println!("  Frame.element(#iframe-para): Ok(None)"),
                Err(e) => println!("  Frame.element(#iframe-para): Error - {}", e),
            }
        }
        Ok(None) => println!("  XPath get_iframe returned None"),
        Err(e) => println!("  XPath get_iframe error: {}", e),
    }

    println!("\n=== Iframe 内元素查找测试 ===");

    // iframe 内存在的元素
    println!("\n--- iframe 内存在的元素 (CSS) ---");
    test_ele(&page, "#iframe-para", "CSS id in iframe (#iframe-para)");

    // iframe 内不存在的元素
    println!("\n--- iframe 内不存在的元素 (CSS) ---");
    test_ele(&page, "#nonexistent-in-iframe", "CSS id in iframe (#nonexistent-in-iframe)");
    test_ele(&page, ".nonexistent-iframe-class", "CSS class in iframe");

    // 主页面和 iframe 同时查找
    println!("\n--- 主页面+iframe 多元素查找 ---");
    test_eles(&page, "p", "tag p (main + iframe)");
    test_eles(&page, "#hello", "id #hello (only main)");
    test_eles(&page, "#iframe-para", "id #iframe-para (only iframe)");

    // XPath 在 iframe 内查找
    println!("\n--- XPath 在 iframe 内查找 ---");
    test_ele(&page, "xpath://p[@id='iframe-para']", "XPath in iframe (exist)");
    test_ele(&page, "xpath://p[@id='nonexistent']", "XPath in iframe (non-exist)");

    // Text 在 iframe 内查找
    println!("\n--- Text 在 iframe 内查找 ---");
    test_ele(&page, "text:Inside iframe", "Text in iframe (exist)");
    test_ele(&page, "text:NoSuchTextInIframe", "Text in iframe (non-exist)");

    // 通过 get_iframe 获取 Frame 后查找
    println!("\n--- get_iframe + Frame.element() 测试 ---");
    let frame_opt = page.get_iframe("#test-frame")?;
    if let Some(frame) = frame_opt {
        let result = frame.element("#iframe-para");
        match &result {
            Ok(Some(_)) => println!("  OK frame.element(#iframe-para): Ok(Some(...))"),
            Ok(None) => println!("  BAD frame.element(#iframe-para): Ok(None) - should exist!"),
            Err(e) => println!("  BAD frame.element(#iframe-para): Error - {}", e),
        }
        let result = frame.element("#nonexistent-in-frame");
        match &result {
            Ok(Some(_)) => println!("  BAD frame.element(#nonexistent): Ok(Some) - should be None!"),
            Ok(None) => println!("  OK frame.element(#nonexistent): Ok(None)"),
            Err(e) => println!("  BAD frame.element(#nonexistent): Error - {}", e),
        }
    } else {
        println!("  SKIP: Could not get iframe #test-frame");
    }

    println!("=== ChromiumPage 层面 ===");

    // --- ele (单个元素) ---
    println!("\n--- page.ele() CSS测试 ---");

    // 元素不存在 → 应返回 Ok(None)
    test_ele(&page, "#nonexistent", "CSS id (#nonexistent)");
    test_ele(&page, ".nonexistent-class", "CSS class (.nonexistent-class)");
    test_ele(&page, "nonexistent-tag", "CSS tag (nonexistent-tag)");
    test_ele(&page, "[data-foo=bar]", "CSS attr ([data-foo=bar])");

    // 元素存在 → 应返回 Ok(Some)
    test_ele(&page, "#hello", "CSS id (#hello) - 存在");

    // --- eles (多个元素) ---
    println!("\n--- page.eles() CSS测试 ---");

    // 元素不存在 → 应返回 Ok(vec![])
    test_eles(&page, "#nonexistent", "CSS id (#nonexistent)");
    test_eles(&page, ".nonexistent", "CSS class (.nonexistent)");

    // 元素存在 → 应返回包含元素的 Vec
    test_eles(&page, "#hello", "CSS id (#hello) - 存在");
    test_eles(&page, "p", "CSS tag (p) - 存在");

    // --- XPath 定位器 ---
    println!("\n--- page.ele() XPath测试 ---");
    test_ele(&page, "xpath://div[@id='nonexistent']", "XPath non-existent");
    test_ele(&page, "xpath://p[@id='hello']", "XPath existent");

    println!("\n--- page.eles() XPath测试 ---");
    test_eles(&page, "xpath://div[@id='nonexistent']", "XPath eles non-existent");
    test_eles(&page, "xpath://p[@id='hello']", "XPath eles existent");

    // --- Text 定位器 ---
    println!("\n--- page.ele() Text测试 ---");
    test_ele(&page, "text:ThisTextDoesNotExistAnywhere", "Text non-existent");
    test_ele(&page, "text:Hello", "Text existent");

    // --- ID 定位器 ---
    println!("\n--- page.ele() ID测试 ---");
    test_ele(&page, "id:nonexistent_id", "Id non-existent");
    test_ele(&page, "id:hello", "Id existent");

    // --- Class 定位器 ---
    println!("\n--- page.ele() Class测试 ---");
    test_ele(&page, "class:nonexistent_class", "Class non-existent");

    // --- Attr 定位器 ---
    println!("\n--- page.ele() Attr测试 ---");
    test_ele(&page, "attr:data-foo=bar", "Attr non-existent");
    test_ele(&page, "attr:id=hello", "Attr existent");

    // --- Tag 定位器 ---
    println!("\n--- page.ele() Tag测试 ---");
    test_ele(&page, "tag:nonexistenttag", "Tag non-existent");
    test_eles(&page, "tag:nonexistenttag", "Tag eles non-existent");

    // --- Page 层面 (page.tab().element / page.tab().elements) ---
    println!("\n--- page.tab().element() 测试 ---");
    let tab = page.tab();

    // 元素不存在
    test_element_page(tab, "#nonexistent", "CSS id (#nonexistent)");
    test_element_page(tab, ".nonexistent", "CSS class (.nonexistent)");
    test_element_page(tab, "xpath://div[@id='nonexistent']", "XPath non-existent");
    test_element_page(tab, "text:NoSuchText", "Text non-existent");

    // 元素存在
    test_element_page(tab, "#hello", "CSS id (#hello) - 存在");

    println!("\n--- page.tab().elements() 测试 ---");
    test_elements_page(tab, "#nonexistent", "CSS id (#nonexistent)");
    test_elements_page(tab, "p", "CSS tag (p) - 存在");
    test_elements_page(tab, "xpath://div[@id='nonexistent']", "XPath non-existent");

    // --- 子元素查找 ---
    println!("\n--- 子元素 element().element() 测试 ---");
    if let Ok(Some(app)) = tab.element("#hello") {
        let result = app.element("#nonexistent");
        match &result {
            Ok(Some(_)) => println!("  OK elem.element(#nonexistent): Ok(Some)"),
            Ok(None) => println!("  OK elem.element(#nonexistent): Ok(None)"),
            Err(e) => println!("  BAD elem.element(#nonexistent): Error - {}", e),
        }
    }

    println!("\n=== All tests done ===");
    page.close_browser();
    Ok(())
}

fn test_ele(page: &ChromiumPage, locator: &str, desc: &str) {
    let result = page.ele(locator);
    match &result {
        Ok(Some(_)) => println!("  OK {}: Ok(Some(...))", desc),
        Ok(None) => println!("  OK {}: Ok(None)", desc),
        Err(e) => println!("  BAD {}: Error - {}", desc, e),
    }
}

fn test_eles(page: &ChromiumPage, locator: &str, desc: &str) {
    let result = page.eles(locator);
    match &result {
        Ok(v) if v.is_empty() => println!("  OK {}: Ok(vec![])", desc),
        Ok(v) => println!("  OK {}: Ok(vec len={})", desc, v.len()),
        Err(e) => println!("  BAD {}: Error - {}", desc, e),
    }
}

fn test_element_page(tab: &rust_drission::Page, locator: &str, desc: &str) {
    let result = tab.element(locator);
    match &result {
        Ok(Some(_)) => println!("  OK {}: Ok(Some(...))", desc),
        Ok(None) => println!("  OK {}: Ok(None)", desc),
        Err(e) => println!("  BAD {}: Error - {}", desc, e),
    }
}

fn test_elements_page(tab: &rust_drission::Page, locator: &str, desc: &str) {
    let result = tab.elements(locator);
    match &result {
        Ok(v) if v.is_empty() => println!("  OK {}: Ok(vec![])", desc),
        Ok(v) => println!("  OK {}: Ok(vec len={})", desc, v.len()),
        Err(e) => println!("  BAD {}: Error - {}", desc, e),
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
