//! Boss自动打招呼 简易版

use std::time::Duration;

use rust_drission::{utils::sleep_random_ms, BrowserConfig, CdpError, ChromiumPage};

fn main() -> Result<(), CdpError> {
    let user_data_dir = r#"C:\Users\admin\AppData\Roaming\com.huice.ai\UserData\13"#;

    let config = BrowserConfig::new()
        .user_data_dir(user_data_dir)
        .headless(false);
    let page = ChromiumPage::new(config)?;

    // 进入到牛人页
    page.get("https://www.zhipin.com/web/chat/recommend")?;

    sleep_random_ms(800, 1000);

    let card_selector = "#recommend-list .card-item";

    page.wait(card_selector, Duration::from_secs(3))?;

    // 滚动
    const RECOMMEND_FRAME_LOCATOR: &str = "css:iframe[name=recommendFrame]";
    // 记录已经打过招呼的
    let mut sayed: Vec<String> = vec![];
    // 打招呼人数
    let say_limit = 2;
    // 已打招呼的
    let mut sayed_count = 0;
    // 获取iframe 用于滚动
    let iframe_ele = page.get_iframe(RECOMMEND_FRAME_LOCATOR)?.unwrap();

    while sayed_count < say_limit {
        // 牛人卡片 并排除 含  .similar-geek-wrap 的卡片
        let eles = page.eles(card_selector)?.into_iter().filter(|ele| {
            ele.element(".similar-geek-wrap")
                .map(|e| e.is_none())
                .unwrap_or(false)
        });
        let mut bk = false;
        for ele in eles {
            if say_limit - sayed_count <= 0 {
                break;
            }
            if check_vip2(&page)? {
                println!("检测到充值vip弹窗，停止打招呼");
                bk = true;
                break;
            }
            // 获取牛人筛选配置 用于筛选打招呼的牛人

            // 获取牛人id
            let card_inner_ele = ele.element(".card-inner")?;
            if card_inner_ele.is_none() {
                continue;
            }
            // 已经打过招呼的 就跳过
            let geek_id = card_inner_ele.unwrap().attr("data-geekid")?;
            if sayed.contains(&geek_id) {
                continue;
            }
            // 获取牛人 期望的岗位
            let expect_job_ele = ele.element(".expect-wrap")?;
            if expect_job_ele.is_none() {
                continue;
            }
            let expect_text = expect_job_ele.unwrap().text()?;

            // 打招呼按钮
            let say_hello_btn_ele = ele.element(".btn.btn-greet")?;
            if say_hello_btn_ele.is_none() {
                continue;
            }

            // 点击打招呼
            say_hello_btn_ele.unwrap().click()?;
            sayed_count += 1;
            sayed.push(geek_id.clone());
            println!("牛人id: {}, 期望岗位: {}", geek_id.clone(), expect_text);

            println!("打招呼进度: {}/{}", sayed_count, say_limit);

            sleep_random_ms(800, 1200);
        }

        if bk {
            break;
        }

        iframe_ele.run_js("window.scrollTo(0, document.body.scrollHeight);")?;
        sleep_random_ms(1200, 1500);
    }

    Ok(())
}

// 检测充值vip弹窗
pub fn check_vip2(page: &ChromiumPage) -> Result<bool, CdpError> {
    let vip_layout = page.ele(".vip2-layout")?;
    if vip_layout.is_some() {
        // 关闭窗口
        let close_ele = page.ele(".icon-close")?;
        if let Some(close_ele) = close_ele {
            close_ele.click()?;
        }
    }
    return Ok(vip_layout.is_some());
}
