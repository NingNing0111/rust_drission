//! page.wait 系列 API 使用示例
//!
//! 演示 wait / wait_element / wait_visible / wait_hidden / wait_network_idle
//! 在实际场景中的用法。
//!
//! 运行方式: cargo run --example page_wait_demo

use std::time::Duration;

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_data_dir = std::env::temp_dir().join("drission_demo_page_wait");
    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);
    let mut page = ChromiumPage::new(config)?;

    demo_wait_element_appears(&page)?;
    demo_wait_visible_for_modal(&page)?;
    demo_wait_hidden_for_spinner(&page)?;
    demo_wait_with_different_locators(&page)?;
    demo_timeout_handling(&page)?;

    println!("\nAll demos completed.");
    page.close_browser();
    Ok(())
}

// ── 场景 1: 等待动态内容加载 ────────────────────────────────────────
//
// 页面加载后，内容可能由 JS 异步渲染。
// 使用 wait() 等待目标元素出现在 DOM 中。

fn demo_wait_element_appears(page: &ChromiumPage) -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 1. 等待动态内容加载 ===");

    let html = build_html("<div id='app'></div>");
    page.get(&to_data_url(&html))?;

    // 模拟异步数据加载：1 秒后插入列表
    page.run_js(
        "setTimeout(() => { \
         const app = document.getElementById('app'); \
         app.innerHTML = '<ul class=\"item-list\"><li>Item A</li><li>Item B</li></ul>'; \
         }, 1000)",
    )?;

    // 等待 .item-list 出现在 DOM 中
    match page.wait(".item-list", Duration::from_secs(5)) {
        Ok(Some(el)) => {
            let items = el.elements("li")?;
            println!("  列表已加载，包含 {} 项", items.len());
            for (i, item) in items.iter().enumerate() {
                println!("    [{}] {}", i, item.text().unwrap_or_default());
            }
        }
        Ok(None) => println!("  加载超时"),
        Err(e) => println!("  等待失败: {}", e),
    }

    Ok(())
}

// ── 场景 2: 等待弹窗/模态框出现 ──────────────────────────────────
//
// 弹窗（如确认框、登录框）可能在 DOM 中已存在但 display:none，
// 只有触发时才变为可见。使用 wait_visible() 等待它显示。

fn demo_wait_visible_for_modal(page: &ChromiumPage) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 2. 等待弹窗出现（wait_visible） ===");

    let html = build_html(
        "<div id='modal' style='display:none'>\
           <p class='modal-text'>确认删除？</p>\
           <button class='btn-confirm'>确定</button>\
         </div>\
         <button id='trigger-btn' onclick=\"document.getElementById('modal').style.display='block'\">打开弹窗</button>",
    );
    page.get(&to_data_url(&html))?;

    // 点击按钮触发弹窗（延迟 300ms 模拟用户操作）
    page.run_js("setTimeout(() => document.getElementById('trigger-btn').click(), 300)")?;

    // 等待弹窗可见
    match page.tab().wait_visible("#modal", Duration::from_secs(5)) {
        Ok(modal) => {
            let text = modal
                .element(".modal-text")?
                .map(|e| e.text().unwrap_or_default());
            println!("  弹窗已出现: {:?}", text);
        }
        Err(e) => println!("  弹窗未出现: {}", e),
    }

    Ok(())
}

// ── 场景 3: 等待加载动画消失 ────────────────────────────────────
//
// 页面加载时通常有 loading spinner，请求完成后消失。
// 使用 wait_hidden() 等待其隐藏。

fn demo_wait_hidden_for_spinner(page: &ChromiumPage) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 3. 等待加载动画消失（wait_hidden） ===");

    let html = build_html(
        "<div id='spinner' class='loading'>Loading...</div>\
         <div id='content' style='display:none'>实际内容</div>",
    );
    page.get(&to_data_url(&html))?;

    // 模拟请求完成：800ms 后隐藏 spinner，显示内容
    page.run_js(
        "setTimeout(() => { \
         document.getElementById('spinner').style.display = 'none'; \
         document.getElementById('content').style.display = 'block'; \
         }, 800)",
    )?;

    // 等待加载动画消失
    match page.tab().wait_hidden("#spinner", Duration::from_secs(5)) {
        Ok(()) => {
            println!("  spinner 已消失");
            // 确认内容已显示
            let content_el = page.ele("#content")?;
            println!(
                "  内容已显示: {}",
                content_el
                    .map(|e| e.text().unwrap_or_default())
                    .unwrap_or_default()
            );
        }
        Err(e) => println!("  spinner 超时未消失: {}", e),
    }

    Ok(())
}

// ── 场景 4: 各种定位器类型 ──────────────────────────────────────
//
// wait 系列方法支持所有 Locator 类型：CSS、XPath、Text、ID 等。

fn demo_wait_with_different_locators(
    page: &ChromiumPage,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 4. 各种定位器类型 ===");

    let html = build_html("<div id='container'></div>");
    page.get(&to_data_url(&html))?;

    // 延迟插入多种元素
    page.run_js(
        "setTimeout(() => { \
         const c = document.getElementById('container'); \
         c.innerHTML = [ \
           '<button id=\"my-btn\" class=\"action-btn\">点击我</button>', \
           '<span data-role=\"badge\">99+</span>', \
           '<p>这是一段提示文本</p>' \
         ].join(''); \
         }, 500)",
    )?;

    // CSS class 定位
    if let Ok(Some(el)) = page.wait(".action-btn", Duration::from_secs(5)) {
        println!("  CSS(.action-btn): {}", el.text().unwrap_or_default());
    }

    // ID 定位
    if let Ok(Some(el)) = page.wait("id:my-btn", Duration::from_secs(5)) {
        println!("  ID(my-btn): tag={}", el.tag().unwrap_or_default());
    }

    // Text 定位
    if let Ok(Some(el)) = page.wait("text:提示", Duration::from_secs(5)) {
        println!("  Text(提示): {}", el.text().unwrap_or_default());
    }

    // XPath 定位
    if let Ok(Some(el)) = page.wait("xpath://span[@data-role='badge']", Duration::from_secs(5)) {
        println!("  XPath(badge): {}", el.text().unwrap_or_default());
    }

    // Tag 定位
    if let Ok(Some(el)) = page.wait("tag:button", Duration::from_secs(5)) {
        println!("  Tag(button): {}", el.text().unwrap_or_default());
    }

    Ok(())
}

// ── 场景 5: 超时处理 ──────────────────────────────────────────────
//
// 根据场景选择合适的超时时间，并对超时做降级处理。

fn demo_timeout_handling(page: &ChromiumPage) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== 5. 超时处理策略 ===");

    let html = build_html("<div id='app'></div>");
    page.get(&to_data_url(&html))?;

    // ── 策略 A: 关键元素用较长超时，失败则中止 ──
    println!("  [策略 A] 等待关键元素...");
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('div'); \
         el.id = 'critical'; \
         el.textContent = 'ready'; \
         document.getElementById('app').appendChild(el); \
         }, 300)",
    )?;
    match page.wait("#critical", Duration::from_secs(10)) {
        Ok(Some(_)) => println!("    关键元素就绪，继续流程"),
        Ok(None) => {
            println!("    关键元素缺失，中止");
            return Err("关键元素等待超时".into());
        }
        Err(e) => {
            println!("    等待失败，中止: {}", e);
            return Err(e.into());
        }
    }

    // ── 策略 B: 非关键元素用短超时，超时则降级 ──
    println!("  [策略 B] 等待可选元素...");
    match page.wait("#optional-banner", Duration::from_secs(1)) {
        Ok(Some(el)) => println!("    banner 已显示: {}", el.text().unwrap_or_default()),
        Ok(None) => println!("    banner 未出现，跳过（降级处理）"),
        Err(e) => println!("    等待失败: {}", e),
    }

    // ── 策略 C: 使用 wait_element 快捷方法（默认 30s） ──
    println!("  [策略 C] wait_element 快捷方法...");
    page.run_js(
        "setTimeout(() => { \
         const el = document.createElement('span'); \
         el.id = 'quick-el'; \
         el.textContent = 'quick'; \
         document.getElementById('app').appendChild(el); \
         }, 200)",
    )?;
    match page.tab().wait_element("#quick-el") {
        Ok(Some(el)) => println!("    元素出现: {}", el.text().unwrap_or_default()),
        Ok(None) => println!("    超时，元素未出现"),
        Err(e) => println!("    等待失败: {}", e),
    }

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
