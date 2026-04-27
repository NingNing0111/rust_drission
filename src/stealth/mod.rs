//! Stealth 反检测：在页面注入脚本，降低被识别为自动化的概率（README §13）

use crate::cdp::CdpError;
use crate::page::Page;
use serde_json::json;

/// 在页面加载前注入反检测脚本（navigator.webdriver = false 等）
/// 需在 goto 之前调用，或对已打开的页面仅对后续导航生效
pub fn inject(page: &Page) -> Result<(), CdpError> {
    let script = r#"
// 反检测脚本 - 用于浏览器页面注入
(() => {
  "use strict";

  // 1. 保存原生 Function.prototype.toString
  const nativeFunctionToString = Function.prototype.toString;

  // 2. WeakMap：函数 → 伪原生源码
  const nativeSourceMap = new WeakMap();

  // 3. 注册伪原生源码
  const registerNativeSource = (fn, source) => {
    try {
      nativeSourceMap.set(fn, source);
    } catch (_) {}
  };

  // 4. 劫持 Function.prototype.toString
  Object.defineProperty(Function.prototype, "toString", {
    configurable: true,
    writable: true,
    value: function toString() {
      if (nativeSourceMap.has(this)) {
        return nativeSourceMap.get(this);
      }
      return nativeFunctionToString.call(this);
    },
  });

  // 5. 伪装 Function.prototype.toString 自身
  registerNativeSource(
    Function.prototype.toString,
    nativeFunctionToString.toString(),
  );

  // 6. stealthify：包装函数但保持"原生外观"
  const stealthify = (obj, prop, handler) => {
    const original = obj[prop];
    if (typeof original !== "function") return;

    const wrapped = function (...args) {
      return handler.call(this, original, args);
    };
    
    // 处理函数 name 属性
    const namePropertyDescriptor = Object.getOwnPropertyDescriptor(wrapped, "name");
    Object.defineProperty(wrapped, "name", {
      ...namePropertyDescriptor,
      value: prop,
    });
    
    // 保留 prototype
    try {
      Object.setPrototypeOf(wrapped, Object.getPrototypeOf(original));
    } catch (_) {}

    // 注册伪原生源码
    registerNativeSource(wrapped, nativeFunctionToString.call(original));

    // 用 defineProperty 保持 descriptor 接近原生
    const desc = Object.getOwnPropertyDescriptor(obj, prop);
    Object.defineProperty(obj, prop, {
      ...desc,
      value: wrapped,
    });
  };

  // 7. 过滤 console 参数，避免触发属性 getter
  const filterConsoleArgs = (args) =>
    args.map((arg) => {
      if (arg && typeof arg === "object") {
        return {};  // 返回空对象避免访问原有属性
      }
      return arg;
    });

  // 8. 劫持 console 方法
  ["log", "debug", "info", "warn", "error", "dir", "table"].forEach((name) => {
    stealthify(console, name, (original, args) => {
      return original.apply(console, filterConsoleArgs(args));
    });
  });

  // 9. 防御性补丁 - 隐藏 registerNativeSource 真实源码
  registerNativeSource(
    registerNativeSource,
    "function registerNativeSource() { [native code] }",
  );
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
