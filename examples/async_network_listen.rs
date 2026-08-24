use rust_drission::{AsyncBrowser, CdpError};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), CdpError> {
    // 先用类似命令启动 Chrome：
    // chrome --remote-debugging-port=9222
    let browser = AsyncBrowser::connect("127.0.0.1:9222").await?;
    let page = browser.new_tab().await?;

    let mut listener = page.listen_url("httpbin").await?;
    page.get("https://httpbin.org/get").await?;

    if let Some(packet) = listener.wait(Duration::from_secs(10)).await? {
        println!("{} {}", packet.request.method, packet.request.url);
        println!("status = {:?}", packet.response.status);
        println!(
            "body bytes = {}",
            packet.body.as_ref().map(Vec::len).unwrap_or(0)
        );
    } else {
        println!("没有在超时时间内收到匹配的数据包");
    }

    page.close().await?;
    Ok(())
}
