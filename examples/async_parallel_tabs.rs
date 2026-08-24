use rust_drission::{AsyncBrowser, BrowserConfig, CdpError};
use std::time::Duration;

#[derive(Debug)]
struct TabResult {
    index: usize,
    name: &'static str,
    target_id: String,
    title: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), CdpError> {
    // 通过 SDK 启动 Chrome，不需要用户提前执行 chrome --remote-debugging-port。
    let mut browser =
        AsyncBrowser::launch(BrowserConfig::new().set_local_port(9222).headless(false)).await?;

    let jobs = [
        ("百度", "https://www.baidu.com"),
        ("bilibili", "https://www.bilibili.com"),
    ];

    // 创建多个 tab。每个 AsyncPage 都共享同一个 AsyncCdpClient，CDP 响应会按 id 路由回对应任务。
    let mut pages = Vec::new();
    for _ in jobs {
        pages.push(browser.new_tab().await?);
    }

    // 多个 tab 并行执行：导航到不同网站、等待页面 readyState、读取 title/url。
    // 百度、bilibili 这类真实站点可能长期保持埋点、推送、视频等请求，不能用严格 network idle 判断完成。
    let mut handles = Vec::new();
    for (index, (page, (name, url))) in pages.into_iter().zip(jobs).enumerate() {
        let handle = tokio::spawn(async move {
            page.get(url).await?;
            wait_document_ready(&page, Duration::from_secs(20)).await?;

            let result = TabResult {
                index,
                name,
                target_id: page.tab_id().to_string(),
                title: page.title().await?,
                url: page.url().await?,
            };

            page.close().await?;
            Ok::<_, CdpError>(result)
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle
            .await
            .map_err(|e| CdpError::ChannelClosed(format!("tab task failed to join: {e}")))??;
        println!(
            "tab #{index} ({name}): target={target_id}, title={title}, url={url}",
            index = result.index + 1,
            name = result.name,
            target_id = result.target_id,
            title = result.title,
            url = result.url,
        );
    }

    browser.close();
    Ok(())
}

async fn wait_document_ready(
    page: &rust_drission::AsyncPage,
    timeout: Duration,
) -> Result<(), CdpError> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let ready_state = page
            .run_js("document.readyState")
            .await?
            .get("value")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();

        if matches!(ready_state.as_str(), "interactive" | "complete") {
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(CdpError::Timeout(format!(
                "Timed out while waiting for document.readyState after {:?}",
                timeout
            )));
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
