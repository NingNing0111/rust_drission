//! 测试指定本地 user_data_dir
//!
//! 运行方式: cargo run --example test_user_data_dir

use rust_drission::{BrowserConfig, ChromiumPage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 模拟用户指定本地目录 — Windows 上就是类似 C:\Users\xxx\my_chrome_profile 这种
    let user_data_dir = std::env::temp_dir().join("drission_custom_profile");

    println!("user_data_dir: {}", user_data_dir.display());
    println!("目录是否存在: {}", user_data_dir.exists());

    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir.to_string_lossy().to_string())
        .headless(true);

    let mut page = ChromiumPage::new(config)?;

    println!("目录是否存在（启动后）: {}", user_data_dir.exists());
    println!("启动成功！");

    // 跑一个简单操作确认浏览器正常工作
    page.get("data:text/html,<h1>Hello World</h1>")?;
    let title = page.title()?;
    println!("页面标题: {}", title);

    // 看一眼目录里有什么
    if user_data_dir.exists() {
        let entries: Vec<_> = std::fs::read_dir(&user_data_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        println!("目录内容 ({}) :", entries.len());
        for name in &entries {
            println!("  - {}", name);
        }
    }

    page.close_browser();
    println!("浏览器已关闭。数据目录保留: {}", user_data_dir.display());

    Ok(())
}
