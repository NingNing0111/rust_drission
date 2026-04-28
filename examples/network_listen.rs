//! 网络数据包监听示例
//!
//! 演示 ChromiumPage 的四种网络监听 API：
//!   1. listen()            — 基础监听，接收所有网络数据包
//!   2. listen_url()        — 按 URL 关键词过滤
//!   3. listen_resource_type() — 按资源类型过滤（XHR / Fetch / Document 等）
//!   4. listen_collect()    — 从已有监听器批量收集
//!
//! 核心用法模式：
//!   先 listen() → 再 page.get() → 最后 listener.wait() / collect()
//!
//! 运行方式: cargo run --example network_listen

use std::time::Duration;

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() {
    let config = BrowserConfig::new().headless(false);
    let mut page = ChromiumPage::new(config).expect("无法启动浏览器");

    // ============================================================
    // 示例 1: listen() — 基础监听，打印所有数据包
    // ============================================================
    println!("===== 示例 1: listen() 基础监听 =====\n");

    // 1) 先启动监听（阻塞直到后台线程就绪）
    let listener = page.listen().expect("启动监听失败");
    // 2) 再导航页面（监听器在后台实时接收网络事件）
    page.get("https://www.httpbin.org/get")
        .expect("访问失败");
    // 3) 收集数据包
    for i in 1..=20 {
        match listener.wait(Duration::from_secs(5)) {
            Ok(Some(pkt)) => print_packet(i, &pkt),
            Ok(None) => {
                println!("[{:02}] （超时，无更多数据包）", i);
                break;
            }
            Err(e) => {
                println!("[{:02}] 错误: {}", i, e);
                break;
            }
        }
    }

    drop(listener);

    // ============================================================
    // 示例 2: listen_url() — 只捕获 URL 包含指定关键词的请求
    // ============================================================
    println!("\n===== 示例 2: listen_url() 按 URL 过滤 =====\n");

    let url_listener = page
        .listen_url("httpbin")
        .expect("启动 URL 过滤监听失败");

    page.get("https://www.httpbin.org/ip")
        .expect("访问失败");

    // 只有 URL 包含 "httpbin" 的数据包才会被返回，其余被自动跳过
    if let Some(pkt) = url_listener.wait(Duration::from_secs(10)).expect("等待失败") {
        let status = pkt.response.status.unwrap_or(0);
        println!(
            "捕获到 httpbin 请求: {} {} → {}",
            pkt.request.method, pkt.request.url, status
        );
        if let Some(body) = &pkt.body {
            let text = String::from_utf8_lossy(body);
            println!("响应内容: {}", text);
        }
    }

    drop(url_listener);

    // ============================================================
    // 示例 3: listen_resource_type() — 只捕获 Fetch 类型请求
    // ============================================================
    println!("\n===== 示例 3: listen_resource_type() 按资源类型过滤 =====\n");

    let fetch_listener = page
        .listen_resource_type("Fetch")
        .expect("启动资源类型过滤监听失败");

    // 先启动监听，再用 JS 触发 fetch 请求
    page.run_js_await(
        r#"
        fetch("https://www.httpbin.org/headers")
            .then(r => r.json())
            .then(d => JSON.stringify(d))
    "#,
    )
    .expect("执行 fetch 失败");

    match fetch_listener.wait(Duration::from_secs(10)) {
        Ok(Some(pkt)) => {
            println!(
                "捕获到 Fetch 请求: {} {} → {}",
                pkt.request.method,
                pkt.request.url,
                pkt.response.status.unwrap_or(0)
            );
            if let Some(body) = &pkt.body {
                let text = String::from_utf8_lossy(body);
                println!("Fetch 响应: {}", text);
            }
        }
        Ok(None) => {
            // CDP 可能将 JS fetch() 标记为其他类型（如 XHR）
            println!("（未捕获到 Fetch 类型请求，CDP 可能将其归为其他类型）");
        }
        Err(e) => println!("错误: {}", e),
    }

    drop(fetch_listener);

    // ============================================================
    // 示例 4: listen_collect() — 批量收集到 Vec
    // ============================================================
    println!("\n===== 示例 4: listen_collect() 批量收集 =====\n");

    // 1) 启动监听
    let listener = page.listen().expect("启动监听失败");
    // 2) 导航
    page.get("https://www.httpbin.org/html")
        .expect("访问失败");
    // 3) 批量收集（最多等 8 秒收集所有剩余数据包）
    let packets = page
        .listen_collect(&listener, Duration::from_secs(8), |pkt| {
            let status = pkt.response.status.unwrap_or(0);
            let rtype = pkt.resource_type.as_deref().unwrap_or("-");
            println!(
                "  收集: {} {} → {} ({})",
                pkt.request.method,
                truncate_url(&pkt.request.url, 60),
                status,
                rtype
            );
            true // 继续收集
        })
        .expect("收集失败");

    // 统计各资源类型数量
    let mut type_counts = std::collections::HashMap::new();
    for pkt in &packets {
        let key = pkt.resource_type.clone().unwrap_or_else(|| "Unknown".to_string());
        *type_counts.entry(key).or_insert(0) += 1;
    }
    println!("\n共收集 {} 个数据包", packets.len());
    println!("资源类型分布:");
    for (rtype, count) in &type_counts {
        println!("  {}: {}", rtype, count);
    }

    // ============================================================
    // 关闭浏览器
    // ============================================================
    println!("\n示例完成，关闭浏览器");
    page.close_browser();
}

fn print_packet(i: usize, pkt: &rust_drission::DataPacket) {
    let status = pkt.response.status.unwrap_or(0);
    let rtype = pkt.resource_type.as_deref().unwrap_or("-");
    let url = truncate_url(&pkt.request.url, 80);

    println!("[{:02}] {} {} → {} ({})", i, pkt.request.method, url, status, rtype);

    if let Some(body) = &pkt.body {
        let preview = String::from_utf8_lossy(body);
        let preview: String = preview.chars().take(200).collect();
        if !preview.is_empty() {
            println!("     body: {}", preview);
        }
    }
}

/// 截断 URL 以便终端展示
fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len])
    }
}
