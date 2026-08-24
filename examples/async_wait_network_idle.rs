use rust_drission::{AsyncBrowser, CdpError};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), CdpError> {
    // 先用类似命令启动 Chrome：
    // chrome --remote-debugging-port=9222
    let browser = AsyncBrowser::connect("127.0.0.1:9222").await?;
    let page = browser.new_tab().await?;

    page.get("https://example.com").await?;
    page.wait_network_idle_for(Duration::from_millis(500), Duration::from_secs(10))
        .await?;

    println!("网络已空闲，title = {}", page.title().await?);

    page.close().await?;
    Ok(())
}
