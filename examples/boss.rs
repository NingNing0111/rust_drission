//! Boss自动打招呼 简易版

use rust_drission::{utils::sleep_random_ms, BrowserConfig, CdpError, ChromiumPage};

fn main() -> Result<(), CdpError> {
    // 跨平台用户数据目录（Windows: C:\Users\xxx\..., macOS: /Users/xxx/..., Linux: /home/xxx/...）
    let user_data_dir = std::env::temp_dir().join("drission_boss_userdata");
    let user_data_dir = user_data_dir.to_string_lossy().to_string();

    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir)
        .headless(false);
    let mut page = ChromiumPage::new(config)?;

    // 进入到牛人页
    page.get("https://www.zhipin.com/web/geek/jobs")?;

    sleep_random_ms(1000,3000);
    page.close_browser();
    sleep_random_ms(30000, 60000);

    Ok(())


}