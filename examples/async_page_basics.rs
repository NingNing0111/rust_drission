use rust_drission::{AsyncBrowser, CdpError};

#[tokio::main]
async fn main() -> Result<(), CdpError> {
    // 先用类似命令启动 Chrome：
    // chrome --remote-debugging-port=9222
    let browser = AsyncBrowser::connect("127.0.0.1:9222").await?;
    let page = browser.new_tab().await?;

    page.get("https://example.com").await?;
    println!("target = {}", page.tab_id());
    println!("title = {}", page.title().await?);
    println!("url = {}", page.url().await?);
    println!(
        "h1 = {:?}",
        page.run_js("document.querySelector('h1')?.textContent")
            .await?
    );

    page.close().await?;
    Ok(())
}
