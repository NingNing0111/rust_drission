//! Stealth 反检测：在页面注入脚本，降低被识别为自动化的概率（README §13）

use crate::cdp::CdpError;
use crate::page::Page;
use serde_json::json;

/// 在页面加载前注入反检测脚本（navigator.webdriver = false 等）
/// 需在 goto 之前调用，或对已打开的页面仅对后续导航生效
pub fn inject(page: &Page) -> Result<(), CdpError> {
    let script = r#"
(function () {
    'use strict';

    // 1. 缓存原始方法，用于内部调用（如果不小心报错可以自救）
    const rawToString = Function.prototype.toString;
    const rawSetInterval = window.setInterval;
    const rawClear = console.clear;

    // 2. 核心：万能伪装函数，确保任何被修改的方法看起来都像原生的
    const makeNative = (fn, name) => {
        const wrapper = {
            [name]: function () { return fn.apply(this, arguments); }
        }[name];
        // 关键：修改 toString 指向
        wrapper.toString = () => `function ${name}() { [native code] }`;
        return wrapper;
    };

    // 3. 彻底屏蔽 Console：不只是 table，是全家桶
    // 网站现在打印数组、div、时间，全部都是通过这些方法
    const silent = () => {};
    const consoleMethods = ['log', 'debug', 'info', 'warn', 'error', 'table', 'clear', 'dir', 'group', 'groupCollapsed'];
    
    consoleMethods.forEach(method => {
        // 直接用一个不执行任何操作的空函数覆盖
        // 使用 Object.defineProperty 确保它不容易被网站改回去
        Object.defineProperty(console, method, {
            value: makeNative(silent, method),
            writable: false,
            configurable: false
        });
    });

    // 4. 时序检测防御：修复 performance.now
    const start = Date.now();
    Object.defineProperty(performance, 'now', {
        value: makeNative(() => Date.now() - start, 'now'),
        configurable: false
    });

    // 屏蔽 location.replace 到空白页
    const rawReplace = location.replace;
    location.replace = makeNative(function(url) {
        if (url.includes('about:blank')) return;
        return rawReplace.call(location, url);
    }, 'replace');

    // 6. 拦截定时器炸弹
    // 图中反复出现的 Sat May 09 ... 说明它在一个 setInterval 里
    window.setInterval = makeNative((fn, delay, ...args) => {
        if (typeof fn === 'string' && fn.includes('debugger')) return -1;
        if (typeof fn === 'function') {
            const str = rawToString.call(fn);
            // 如果函数体内包含大量的 console 或 debugger，直接不运行
            if (str.includes('debugger') || (str.match(/console/g) || []).length > 5) {
                return -1; 
            }
        }
        return rawSetInterval(fn, delay, ...args);
    }, 'setInterval');

    // 7. 特征抹除：针对 navigator.webdriver
    Object.defineProperty(navigator, 'webdriver', { get: () => false });

})();
"#;
    let params = json!({ "source": script });
    page.client.send_with_session(
        "Page.addScriptToEvaluateOnNewDocument",
        Some(params),
        Some(page.session_id.as_str()),
    )?;
    Ok(())
}
