#[allow(unused_imports)]
use rust_drission::{Browser, BrowserConfig, CdpError};

fn main() -> Result<(), CdpError> {

    // 方式二：启动新 Chrome（带反检测参数，便于验证不被检测）
    let browser = Browser::launch(
        BrowserConfig::new()
        .chrome_path("C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe")
        .user_data_dir("D:\\Code\\StudyCode\\rust_drission\\data")
            .remote_debugging_port(9222)
            .headless(false),
    )?;

    let page = browser.new_tab()?;
    page.goto("https://www.zhipin.com")?;

    println!("已打开页面，可在浏览器中查看是否被检测为自动化。");
    println!("按 Enter 退出...");
    let _ = std::io::stdin().read_line(&mut String::new());

    Ok(())
}
