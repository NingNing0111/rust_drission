//! 测试浏览器标签页管理 API：new_tab、tabs、tab_ids、tabs_count、
//! latest_tab、get_tab、get_tabs、activate_tab、close_tabs、browser、tab、into_parts
//!
//! 运行方式: cargo run --example test_browser_tabs

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_test_tabs");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    // ==================== tabs / tab_ids / tabs_count 测试 ====================
    println!("=== tabs / tab_ids / tabs_count 测试 ===");

    let init_tabs = page.tabs()?;
    let init_count = init_tabs.len();
    println!("  Initial tabs count: {}", init_count);
    assert_or_print(init_count >= 1, "tabs() returns at least 1 tab", &init_count.to_string());

    let ids = page.browser().tab_ids()?;
    assert_or_print(
        ids.len() == init_count,
        "tab_ids() matches tabs() count",
        &ids.len().to_string(),
    );

    let count = page.browser().tabs_count()?;
    assert_or_print(
        count == init_count,
        "tabs_count() matches tabs().len()",
        &count.to_string(),
    );

    // ==================== new_tab 测试 ====================
    println!("\n=== new_tab 测试 ===");

    let page1_url = build_data_url("Tab 1", "<h1>Tab One</h1>");
    let tab1 = page.new_tab(Some(&page1_url))?;
    println!("  OK new_tab(Some(url)): created tab 1");

    let tab2 = page.new_tab(None)?; // about:blank
    println!("  OK new_tab(None): created about:blank tab");

    // verify count increased
    let after_new = page.tabs()?;
    assert_or_print(
        after_new.len() == init_count + 2,
        "tabs() count increased by 2",
        &format!("{} (was {})", after_new.len(), init_count),
    );

    // ==================== tabs_count 测试 ====================
    println!("\n=== tabs_count 验证 ===");
    let count2 = page.browser().tabs_count()?;
    assert_or_print(
        count2 == init_count + 2,
        "tabs_count() after new_tab",
        &count2.to_string(),
    );

    // ==================== latest_tab 测试 ====================
    println!("\n=== latest_tab 测试 ===");

    let latest = page.browser().latest_tab()?;
    let latest_url = latest.url()?;
    // about:blank in headless may return "about:blank" or empty
    println!("  OK latest_tab() url: {}", latest_url);

    // ==================== get_tab 测试 ====================
    println!("\n=== get_tab 测试 ===");

    // get_tab by 1-based index
    let tab1_by_idx = page.browser().get_tab("1", None, None, None)?;
    assert_or_print(tab1_by_idx.is_some(), "get_tab('1') by index", "exists");

    // get_tab by index 0 -> None
    let tab0 = page.browser().get_tab("0", None, None, None)?;
    assert_or_print(tab0.is_none(), "get_tab('0') -> None", "None");

    // get_tab with title filter
    let tab_by_title = page.browser().get_tab("0", Some("Tab 1"), None, None)?;
    assert_or_print(tab_by_title.is_some(), "get_tab(title='Tab 1')", "exists");

    let tab_by_title_none = page.browser().get_tab("0", Some("NoSuchTitleXYZ"), None, None)?;
    assert_or_print(
        tab_by_title_none.is_none(),
        "get_tab(title='NoSuchTitleXYZ') -> None",
        "None",
    );

    // ==================== get_tabs 测试 ====================
    println!("\n=== get_tabs 测试 ===");

    let all_tabs = page.browser().get_tabs(None, None, None)?;
    assert_or_print(
        all_tabs.len() == init_count + 2,
        "get_tabs(None) returns all tabs",
        &all_tabs.len().to_string(),
    );

    let filtered = page.browser().get_tabs(Some("Tab 1"), None, None)?;
    assert_or_print(
        filtered.len() == 1,
        "get_tabs(title='Tab 1') returns 1",
        &filtered.len().to_string(),
    );

    // ==================== activate_tab 测试 ====================
    println!("\n=== activate_tab 测试 ===");

    // Navigate our first tab to a known page
    page.get(&build_data_url("Main Tab", "<h1>Main</h1>"))?;
    let main_tab_id = page.tab().tab_id().to_string();

    // Activate tab1 by target_id
    let tab1_id = tab1.tab_id().to_string();
    page.browser().activate_tab(&tab1_id)?;
    println!("  OK activate_tab('tab1'): no error");

    // Activate back to main
    page.browser().activate_tab(&main_tab_id)?;
    println!("  OK activate_tab('main'): no error");

    // ==================== close_tab / close_tabs 测试 ====================
    println!("\n=== close_tab / close_tabs 测试 ===");

    // close tab2 by target_id (using Browser::close_tabs)
    let tab2_id = tab2.tab_id().to_string();
    page.browser().close_tabs(&[tab2_id.clone()], false)?;
    println!("  OK close_tabs(['tab2'], others=false): closed tab2");

    let after_close_one = page.tabs()?;
    assert_or_print(
        after_close_one.len() == init_count + 1,
        "tabs() count after closing one",
        &after_close_one.len().to_string(),
    );

    // close the remaining extra tab with close_tabs (others=false)
    let remaining_ids = page.browser().tab_ids()?;
    let extra_ids: Vec<String> = remaining_ids
        .into_iter()
        .filter(|id| id != &main_tab_id)
        .collect();
    if !extra_ids.is_empty() {
        page.browser().close_tabs(&extra_ids, false)?;
        println!("  OK close_tabs(extra, false): closed remaining extra tabs");
    }

    let final_tabs = page.tabs()?;
    println!("  Final tabs count: {}", final_tabs.len());

    // ==================== browser() / tab() / into_parts() 测试 ====================
    println!("\n=== browser() / tab() / into_parts() 测试 ===");

    let browser_ref = page.browser();
    assert_or_print(
        browser_ref.tabs_count().unwrap_or(0) > 0,
        "page.browser().tabs_count() works",
        &format!("{}", browser_ref.tabs_count().unwrap_or(0)),
    );

    let tab_ref = page.tab();
    let tab_url = tab_ref.url()?;
    println!("  OK page.tab().url(): {}", tab_url);

    // into_parts - this consumes page, so we do it last (with a fresh page)
    let page2_config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let page2 = ChromiumPage::new(page2_config)?;
    let (browser, tab) = page2.into_parts();
    let url = tab.url()?;
    println!("  OK into_parts() success, tab.url(): {}", url);
    // clean up browser (no close() needed since we don't own the child process)
    drop(browser);
    drop(tab);

    println!("\n=== All browser tabs tests done ===");
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
