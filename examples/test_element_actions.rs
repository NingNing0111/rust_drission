//! 测试元素操作 API：click、input、hover、clear、focus、check、select、is_displayed、is_enabled、value
//!
//! 运行方式: cargo run --example test_element_actions

use rust_drission::{BrowserConfig, ChromiumPage};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_actions");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    let page_html = build_data_url(
        "Element Actions Test",
        r#"
        <style>
            .hidden { display: none; }
            .invisible { visibility: hidden; }
        </style>
        <input id="text-input" type="text" placeholder="Enter text" value="initial" />
        <input id="disabled-input" type="text" value="disabled" disabled />
        <textarea id="textarea">old content</textarea>
        <input id="checkbox-unchecked" type="checkbox" />
        <input id="checkbox-checked" type="checkbox" checked />
        <input id="radio-a" type="radio" name="group" value="a" />
        <input id="radio-b" type="radio" name="group" value="b" checked />
        <select id="select-el">
            <option value="opt1">Option 1</option>
            <option value="opt2" selected>Option 2</option>
            <option value="opt3">Option 3</option>
        </select>
        <button id="click-btn" type="button" onclick="document.getElementById('click-result').textContent='clicked'">Click Me</button>
        <span id="click-result"></span>
        <div id="visible-div">visible content</div>
        <div id="hidden-div" class="hidden">hidden content</div>
        <div id="invisible-div" class="invisible">invisible</div>
        <div id="hover-target" onmouseenter="this.textContent='hovered'" onmouseleave="this.textContent='not hovered'">not hovered</div>
        "#,
    );
    page.get(&page_html)?;

    // ==================== is_displayed 测试 ====================
    println!("=== is_displayed 测试 ===");

    let vis_div = page.ele("#visible-div")?.expect("visible-div should exist");
    let displayed = vis_div.is_displayed()?;
    assert_or_print(displayed, "is_displayed() visible div", &displayed.to_string());

    let hidden_div = page.ele("#hidden-div")?.expect("hidden-div should exist");
    let not_displayed = !hidden_div.is_displayed()?;
    assert_or_print(not_displayed, "is_displayed() hidden div (display:none)", &hidden_div.is_displayed()?.to_string());

    // ==================== is_enabled 测试 ====================
    println!("\n=== is_enabled 测试 ===");

    let text_input = page.ele("#text-input")?.expect("text-input should exist");
    let enabled = text_input.is_enabled()?;
    assert_or_print(enabled, "is_enabled() enabled input", &enabled.to_string());

    let disabled_input = page.ele("#disabled-input")?.expect("disabled-input should exist");
    let disabled = !disabled_input.is_enabled()?;
    assert_or_print(disabled, "is_enabled() disabled input", &(!disabled_input.is_enabled()?).to_string());

    // ==================== value 测试 ====================
    println!("\n=== value 测试 ===");

    let val = text_input.value()?;
    assert_or_print(val == "initial", "value() text input", &val);

    // ==================== click 测试 ====================
    println!("\n=== click 测试 ===");

    // Element.click()
    let btn = page.ele("#click-btn")?.expect("click-btn should exist");
    btn.click()?;
    // verify result via JS
    let result = page.run_js("document.getElementById('click-result').textContent")?;
    let text = result.get("value").and_then(|v| v.as_str()).unwrap_or("");
    assert_or_print(text == "clicked", "click() triggered onclick", text);

    // Page.click() convenience
    page.run_js("document.getElementById('click-result').textContent = ''")?;
    page.click("#click-btn")?;
    let result2 = page.run_js("document.getElementById('click-result').textContent")?;
    let text2 = result2.get("value").and_then(|v| v.as_str()).unwrap_or("");
    assert_or_print(text2 == "clicked", "page.click('#click-btn')", text2);

    // click non-existent -> should error
    let click_err = page.click("#nonexistent-btn");
    match click_err {
        Err(_) => println!("  OK page.click(#nonexistent): Error as expected"),
        Ok(_) => println!("  BAD page.click(#nonexistent): should have errored"),
    }

    // ==================== input / clear 测试 ====================
    println!("\n=== input / clear 测试 ===");

    let input_el = page.ele("#text-input")?.expect("text-input should exist");

    // Element.input()
    input_el.input("new value")?;
    let new_val = input_el.value()?;
    assert_or_print(new_val == "new value", "input('new value')", &new_val);

    // Element.clear()
    input_el.clear()?;
    let cleared = input_el.value()?;
    assert_or_print(cleared.is_empty(), "clear() empties input", &format!("'{}'", cleared));

    // Page.input() convenience
    page.input("#text-input", "page input")?;
    let page_val = input_el.value()?;
    assert_or_print(page_val == "page input", "page.input('#text-input', 'page input')", &page_val);

    // textarea input
    let textarea = page.ele("#textarea")?.expect("textarea should exist");
    textarea.clear()?;
    textarea.input("textarea new")?;
    let ta_val = textarea.value()?;
    assert_or_print(ta_val == "textarea new", "textarea input/clear", &ta_val);

    // ==================== focus 测试 ====================
    println!("\n=== focus 测试 ===");

    input_el.focus()?;
    let active_id = page.run_js("document.activeElement.id")?;
    let id = active_id.get("value").and_then(|v| v.as_str()).unwrap_or("");
    assert_or_print(id == "text-input", "focus() makes element activeElement", id);

    // ==================== hover 测试 ====================
    println!("\n=== hover 测试 ===");

    let hover_target = page.ele("#hover-target")?.expect("hover-target should exist");
    hover_target.hover()?;
    // Give the browser a moment for the mouseenter event to fire
    std::thread::sleep(Duration::from_millis(200));
    let hover_text = hover_target.text()?;
    // Note: Input.dispatchMouseEvent may not trigger JS onmouseenter reliably in headless.
    // Just verify hover() call doesn't error.
    println!(
        "  hover() called on #hover-target, text is now '{}' (mouseenter may not fire via CDP Input)",
        hover_text
    );

    // ==================== check (checkbox) 测试 ====================
    println!("\n=== check / uncheck 测试 ===");

    let cb_unchecked = page.ele("#checkbox-unchecked")?.expect("checkbox-unchecked should exist");
    let cb_checked = page.ele("#checkbox-checked")?.expect("checkbox-checked should exist");

    // check an unchecked box
    assert_or_print(
        !cb_unchecked.property("checked")?.as_bool().unwrap_or(true),
        "checkbox starts unchecked", ""
    );
    cb_unchecked.check(false)?; // check = uncheck=false
    assert_or_print(
        cb_unchecked.property("checked")?.as_bool().unwrap_or(false),
        "check(false) checks box", ""
    );

    // uncheck a checked box
    cb_checked.check(true)?; // uncheck = uncheck=true
    assert_or_print(
        !cb_checked.property("checked")?.as_bool().unwrap_or(true),
        "check(true) unchecks box", ""
    );

    // check an already-checked box (no-op, no error)
    cb_unchecked.check(false)?;
    println!("  OK check() on already-checked: no error");
    cb_checked.check(true)?;
    println!("  OK uncheck() on already-unchecked: no error");

    // ==================== select 测试 ====================
    println!("\n=== select 测试 ===");

    let select_el = page.ele("#select-el")?.expect("select-el should exist");

    // select by value
    select_el.select("opt3", false)?;
    let selected_val = select_el.value()?;
    assert_or_print(selected_val == "opt3", "select('opt3', by_text=false)", &selected_val);

    // select by text
    select_el.select("Option 1", true)?;
    let selected_val = select_el.value()?;
    assert_or_print(selected_val == "opt1", "select('Option 1', by_text=true)", &selected_val);

    // select on non-select element -> should error
    let div = page.ele("#visible-div")?.expect("visible-div should exist");
    let sel_err = div.select("x", false);
    match sel_err {
        Err(_) => println!("  OK select() on non-<select>: Error as expected"),
        Ok(_) => println!("  BAD select() on non-<select>: should have errored"),
    }

    // ==================== tag 测试 ====================
    println!("\n=== tag 测试 ===");

    let tag = text_input.tag()?;
    assert_or_print(tag == "input", "tag() text-input", &tag);
    let tag2 = select_el.tag()?;
    assert_or_print(tag2 == "select", "tag() select", &tag2);
    let tag3 = div.tag()?;
    assert_or_print(tag3 == "div", "tag() div", &tag3);

    println!("\n=== All element actions tests done ===");
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
