//! 测试元素查询 API：parent、children、prev、next、text、inner_html、html、
//! attr、attrs、property、rect、style、element_text/exists/attr/texts、tag、text_content
//!
//! 运行方式: cargo run --example test_element_queries

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_queries");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    let page_html = build_data_url(
        "Element Queries Test",
        r#"
        <div id="container" data-info="main-container" style="color:red;">
            <p id="para1" class="para" title="first paragraph">First paragraph <b>bold text</b> more text.</p>
            <p id="para2" class="para" title="second paragraph">Second paragraph.</p>
            <p id="para3" class="para" hidden>Hidden paragraph.</p>
            <ul id="list">
                <li class="item" data-id="1">Item 1</li>
                <li class="item" data-id="2">Item 2</li>
                <li class="item" data-id="3">Item 3</li>
            </ul>
            <input id="query-input" type="text" value="query-test" placeholder="query" />
        </div>
        "#,
    );
    page.get(&page_html)?;

    let container = page.ele("#container")?.expect("#container should exist");

    // ==================== text / text_content 测试 ====================
    println!("=== text / text_content 测试 ===");

    let text = container.text()?;
    assert_contains(&text, "First paragraph", "text() contains 'First paragraph'");
    // innerText does NOT include hidden elements
    assert_not_contains(&text, "Hidden", "text() does not include hidden paragraph");

    let text_content = container.text_content()?;
    assert_contains(&text_content, "Hidden paragraph", "text_content() includes hidden element");

    let para1 = page.ele("#para1")?.expect("#para1 should exist");
    let p1_text = para1.text()?;
    assert_contains(&p1_text, "bold text", "para1 text() contains 'bold text'");

    // ==================== inner_html / html 测试 ====================
    println!("\n=== inner_html / html 测试 ===");

    let inner = para1.inner_html()?;
    assert_contains(&inner, "<b>bold text</b>", "inner_html() contains <b>bold text</b>");

    let outer = para1.html()?;
    assert_contains(&outer, r#"id="para1""#, "html() (outerHTML) contains id attr");
    assert_contains(&outer, "<b>bold text</b>", "html() contains <b>bold text</b>");

    // ==================== attr / attrs 测试 ====================
    println!("\n=== attr / attrs 测试 ===");

    let id_attr = container.attr("id")?;
    assert_or_print(id_attr == "container", "attr('id')", &id_attr);

    let data_attr = container.attr("data-info")?;
    assert_or_print(data_attr == "main-container", "attr('data-info')", &data_attr);

    let non_exist = container.attr("nonexistent")?;
    assert_or_print(non_exist.is_empty(), "attr('nonexistent') -> ''", &format!("'{}'", non_exist));

    let all_attrs = container.attrs()?;
    assert_or_print(
        all_attrs.get("id") == Some(&"container".to_string()),
        "attrs() includes id",
        &format!("{:?}", all_attrs.keys().collect::<Vec<_>>()),
    );

    // ==================== property 测试 ====================
    println!("\n=== property 测试 ===");

    let input_el = page.ele("#query-input")?.expect("#query-input should exist");
    let prop_val = input_el.property("value")?;
    assert_or_print(
        prop_val.as_str() == Some("query-test"),
        "property('value')",
        &format!("{:?}", prop_val),
    );

    let prop_placeholder = input_el.property("placeholder")?;
    assert_or_print(
        prop_placeholder.as_str() == Some("query"),
        "property('placeholder')",
        &format!("{:?}", prop_placeholder),
    );

    // ==================== rect 测试 ====================
    println!("\n=== rect 测试 ===");

    let rect = container.rect()?;
    let has_fields = rect.get("x").is_some()
        && rect.get("y").is_some()
        && rect.get("width").is_some()
        && rect.get("height").is_some();
    assert_or_print(has_fields, "rect() returns x/y/width/height", &format!("{:?}", rect));
    let w = rect.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
    assert_or_print(w > 0.0, "rect().width > 0", &w.to_string());

    // ==================== style 测试 ====================
    println!("\n=== style 测试 ===");

    let style = container.style()?;
    assert_contains(&style, "color", "style() contains 'color'");
    assert_contains(&style, "red", "style() contains 'red'");

    let no_style = para1.style()?;
    println!("  OK style() on element without inline style: '{}'", no_style);

    // ==================== parent 测试 ====================
    println!("\n=== parent 测试 ===");

    let parent_el = para1.parent(1)?;
    match &parent_el {
        Some(p) => {
            let pid = p.attr("id")?;
            assert_or_print(pid == "container", "parent(1) -> #container", &pid);
        }
        None => println!("  BAD parent(1): returned None"),
    }

    let grandparent = para1.parent(2)?;
    match &grandparent {
        Some(p) => {
            let tag = p.tag()?;
            assert_or_print(tag == "body", "parent(2) -> body", &tag);
        }
        None => println!("  BAD parent(2): returned None"),
    }

    let too_far = para1.parent(100)?;
    assert_or_print(too_far.is_none(), "parent(100) -> None (too far)", "None");

    // ==================== children 测试 ====================
    println!("\n=== children 测试 ===");

    let list = page.ele("#list")?.expect("#list should exist");
    let children = list.children()?;
    assert_or_print(
        children.len() == 3,
        "children() returns 3 li elements",
        &children.len().to_string(),
    );
    if children.len() >= 1 {
        let first_tag = children[0].tag()?;
        assert_or_print(first_tag == "li", "first child tag is li", &first_tag);
    }

    // ==================== prev / next 测试 ====================
    println!("\n=== prev / next 测试 ===");

    let para2 = page.ele("#para2")?.expect("#para2 should exist");

    let prev_el = para2.prev()?;
    match &prev_el {
        Some(p) => {
            let pid = p.attr("id")?;
            assert_or_print(pid == "para1", "prev() sibling before #para2 -> #para1", &pid);
        }
        None => println!("  BAD prev(): returned None"),
    }

    let next_el = para2.next()?;
    match &next_el {
        Some(n) => {
            let nid = n.attr("id")?;
            // para3 is hidden but still in DOM
            assert_or_print(nid == "para3", "next() sibling after #para2 -> #para3", &nid);
        }
        None => println!("  BAD next(): returned None"),
    }

    // first child prev -> None
    let first_p = page.ele("#para1")?.expect("#para1 should exist");
    let no_prev = first_p.prev()?;
    assert_or_print(no_prev.is_none(), "prev() on first child -> None", "None");

    // ==================== element_text 测试 ====================
    println!("\n=== element_text 测试 ===");

    // element on container
    let et = container.element_text("#para1")?;
    assert_or_print(
        et.as_deref().map(|s| s.contains("First paragraph")).unwrap_or(false),
        "element_text('#para1')",
        et.as_deref().unwrap_or("None"),
    );

    let et_none = container.element_text("#nonexistent")?;
    assert_or_print(et_none.is_none(), "element_text('#nonexistent') -> None", "None");

    // ==================== element_exists 测试 ====================
    println!("\n=== element_exists 测试 ===");

    let exists = container.element_exists("#para1")?;
    assert_or_print(exists, "element_exists('#para1')", &exists.to_string());

    let not_exists = container.element_exists("#nonexistent")?;
    assert_or_print(!not_exists, "element_exists('#nonexistent')", &not_exists.to_string());

    // ==================== element_attr 测试 ====================
    println!("\n=== element_attr 测试 ===");

    let ea = container.element_attr("#para2", "title")?;
    assert_or_print(
        ea.as_deref() == Some("second paragraph"),
        "element_attr('#para2', 'title')",
        ea.as_deref().unwrap_or("None"),
    );

    let ea_none = container.element_attr("#para2", "nonexistent")?;
    // getAttribute returns empty string for nonexistent attrs, element_attr returns Some("")
    println!("  element_attr('#para2', 'nonexistent') -> {:?}", ea_none);

    // ==================== element_texts 测试 ====================
    println!("\n=== element_texts 测试 ===");

    let texts = container.element_texts(".para")?;
    assert_or_print(
        texts.len() == 3, // para1, para2, para3 - but para3 hidden, textContent still works
        "element_texts('.para') returns 3 items",
        &format!("{:?}", texts),
    );

    let item_texts = list.element_texts(".item")?;
    assert_or_print(
        item_texts.len() == 3,
        "element_texts('.item') returns 3 items",
        &format!("{:?}", item_texts),
    );
    if item_texts.len() == 3 {
        assert_or_print(item_texts[0] == "Item 1", "item_texts[0]", &item_texts[0]);
        assert_or_print(item_texts[2] == "Item 3", "item_texts[2]", &item_texts[2]);
    }

    // ==================== tag 测试 ====================
    println!("\n=== tag 测试 ===");

    let tag = container.tag()?;
    assert_or_print(tag == "div", "tag() returns 'div'", &tag);
    let tag_p = para1.tag()?;
    assert_or_print(tag_p == "p", "tag() returns 'p' for <p>", &tag_p);

    println!("\n=== All element queries tests done ===");
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

fn assert_contains(text: &str, sub: &str, desc: &str) {
    if text.contains(sub) {
        println!("  OK {}: contains '{}'", desc, sub);
    } else {
        println!("  BAD {}: does NOT contain '{}' (got: '{}')", desc, sub, text);
    }
}

fn assert_not_contains(text: &str, sub: &str, desc: &str) {
    if !text.contains(sub) {
        println!("  OK {}: does not contain '{}'", desc, sub);
    } else {
        println!("  BAD {}: SHOULD NOT contain '{}' (got: '{}')", desc, sub, text);
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
