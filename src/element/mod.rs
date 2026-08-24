//! 元素 API：单个 DOM 节点的操作（click、text、html、attr、input 等）

use crate::cdp::CdpError;
use crate::dom::{
    get_backend_node_id, get_iframe_content_document_node_id, get_node_id_from_backend,
    get_outer_html, query_selector, resolve_backend_to_object_id, resolve_node_to_object_id,
};
use crate::frame::Frame;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::sync::Arc;

use crate::cdp::CdpClient;
use std::fmt;

/// 单个 DOM 元素（对应 nodeId；可存取元素时的 objectId 与 backendNodeId，与 DrissionPage 一致）
pub struct Element {
    pub(crate) client: Arc<CdpClient>,
    pub(crate) session_id: String,
    pub(crate) node_id: RefCell<i64>,
    /// 取元素时持有的 objectId，iframe 内节点优先用它做 callFunctionOn，避免 nodeId 失效
    pub(crate) initial_object_id: RefCell<Option<String>>,
    /// 稳定引用，DOM 更新后可用其重新取 nodeId/objectId
    pub(crate) backend_node_id: Option<i64>,
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Element")
            .field("node_id", &*self.node_id.borrow())
            .finish_non_exhaustive()
    }
}

/// 是否为“节点已失效”类错误（含 No node / Could not find node with given id），用于触发 backendNodeId 刷新重试
fn is_no_node_error(e: &CdpError) -> bool {
    match e {
        CdpError::Protocol { message, .. } => {
            message.contains("No node with given id")
                || message.contains("Could not find node with given id")
        }
        _ => false,
    }
}

/// 通过 objectId 获取 nodeId 和 backendNodeId，使用 DOM.describeNode（比 DOM.requestNode 更可靠）
fn describe_node_by_object_id(
    client: &Arc<CdpClient>,
    session_id: &str,
    object_id: &str,
) -> Result<(Option<i64>, Option<i64>), CdpError> {
    let params = json!({ "objectId": object_id });
    let res = client.send_with_session("DOM.describeNode", Some(params), Some(session_id))?;
    let node = res.get("node");
    let node_id = node.and_then(|n| n.get("nodeId")).and_then(Value::as_i64);
    let backend_node_id = node
        .and_then(|n| n.get("backendNodeId"))
        .and_then(Value::as_i64);
    Ok((node_id, backend_node_id))
}

fn is_object_invalid_error(e: &CdpError) -> bool {
    match e {
        CdpError::Protocol { message, .. } => {
            message.contains("No node with given id")
                || message.contains("Could not find object")
                || message.contains("Object has been collected")
                || message.contains("given id")
        }
        _ => false,
    }
}

impl Element {
    pub(crate) fn new(client: Arc<CdpClient>, session_id: String, node_id: i64) -> Self {
        let backend_node_id = get_backend_node_id(&client, &session_id, node_id).ok();
        Self {
            client,
            session_id,
            node_id: RefCell::new(node_id),
            initial_object_id: RefCell::new(None),
            backend_node_id,
        }
    }

    /// 用已取好的 backend 创建元素（批量取 backend 后调用）
    pub(crate) fn new_with_backend(
        client: Arc<CdpClient>,
        session_id: String,
        node_id: i64,
        backend_node_id: Option<i64>,
    ) -> Self {
        Self {
            client,
            session_id,
            node_id: RefCell::new(node_id),
            initial_object_id: RefCell::new(None),
            backend_node_id,
        }
    }

    /// 创建时带入取元素时的 objectId（iframe 内节点优先用其做 callFunctionOn）
    pub(crate) fn new_with_object_id(
        client: Arc<CdpClient>,
        session_id: String,
        node_id: i64,
        object_id: Option<String>,
        backend_node_id: Option<i64>,
    ) -> Self {
        Self {
            client,
            session_id,
            node_id: RefCell::new(node_id),
            initial_object_id: RefCell::new(object_id),
            backend_node_id,
        }
    }

    /// 先尝试 first()，若失败且为 "No node with given id" 则用 backendNodeId 刷新后执行 retry(fresh_nid)
    fn with_valid_node_id<T, F1, F2>(&self, first: F1, retry: F2) -> Result<T, CdpError>
    where
        F1: FnOnce() -> Result<T, CdpError>,
        F2: FnOnce(i64) -> Result<T, CdpError>,
    {
        match first() {
            Ok(t) => Ok(t),
            Err(e) => {
                if is_no_node_error(&e) {
                    if let Some(backend_id) = self.backend_node_id {
                        let fresh =
                            get_node_id_from_backend(&self.client, &self.session_id, backend_id)?;
                        self.node_id.replace(fresh);
                        return retry(fresh);
                    }
                }
                Err(e)
            }
        }
    }

    fn object_id(&self) -> Result<String, CdpError> {
        if let Some(oid) = self.initial_object_id.borrow().as_ref() {
            return Ok(oid.clone());
        }
        match resolve_node_to_object_id(&self.client, &self.session_id, *self.node_id.borrow()) {
            Ok(oid) => Ok(oid),
            Err(e) => {
                if is_no_node_error(&e) {
                    if let Some(bid) = self.backend_node_id {
                        if let Ok(oid) =
                            resolve_backend_to_object_id(&self.client, &self.session_id, bid)
                        {
                            return Ok(oid);
                        }
                        if let Ok(fresh_nid) =
                            get_node_id_from_backend(&self.client, &self.session_id, bid)
                        {
                            self.node_id.replace(fresh_nid);
                            return resolve_node_to_object_id(
                                &self.client,
                                &self.session_id,
                                fresh_nid,
                            );
                        }
                    }
                }
                Err(e)
            }
        }
    }

    pub(crate) fn call_on(&self, fn_decl: &str, args: Option<Value>) -> Result<Value, CdpError> {
        self.call_on_inner(fn_decl, args, false)
    }

    /// 同 call_on，但设置 returnByValue=true，将 JS 对象序列化为 JSON 值返回。
    /// 用于需要读取属性/数值的场景（如 getBoundingClientRect、is_displayed 等）。
    /// 需要 objectId 的场景（如 parent、children）应使用 call_on。
    pub(crate) fn call_on_value(
        &self,
        fn_decl: &str,
        args: Option<Value>,
    ) -> Result<Value, CdpError> {
        self.call_on_inner(fn_decl, args, true)
    }

    fn call_on_inner(
        &self,
        fn_decl: &str,
        args: Option<Value>,
        return_by_value: bool,
    ) -> Result<Value, CdpError> {
        let object_id = self.object_id()?;
        let mut params = json!({
            "functionDeclaration": fn_decl,
            "objectId": object_id,
            "returnByValue": return_by_value
        });
        if let Some(ref a) = args {
            params["arguments"] = a.clone();
        }
        let result = self.client.send_with_session(
            "Runtime.callFunctionOn",
            Some(params),
            Some(self.session_id.as_str()),
        );
        match result {
            Ok(r) => Ok(r.get("result").cloned().unwrap_or(Value::Null)),
            Err(e) => {
                if is_object_invalid_error(&e) {
                    if self.initial_object_id.borrow().is_some() {
                        self.initial_object_id.replace(None);
                        return self.call_on_inner(fn_decl, args.clone(), return_by_value);
                    }
                    // 子元素等无 initial_object_id：用 backend_node_id 刷新 node_id 后重试一次
                    if let Some(bid) = self.backend_node_id {
                        if let Ok(fresh_nid) =
                            get_node_id_from_backend(&self.client, &self.session_id, bid)
                        {
                            self.node_id.replace(fresh_nid);
                            return self.call_on_inner(fn_decl, args.clone(), return_by_value);
                        }
                    }
                }
                Err(e)
            }
        }
    }

    /// 点击元素
    pub fn click(&self) -> Result<(), CdpError> {
        self.call_on("function(){ this.click(); }", None)?;
        Ok(())
    }

    /// 标签名，小写（与 DrissionPage `tag` 属性一致）
    pub fn tag(&self) -> Result<String, CdpError> {
        let result = self.call_on(
            "function(){ return (this.localName || this.tagName || '').toLowerCase(); }",
            None,
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 是否为 iframe 或 frame 元素（与 DrissionPage 一致，用于将 iframe 看作普通元素）
    pub fn is_frame(&self) -> Result<bool, CdpError> {
        let t = self.tag()?;
        Ok(t == "iframe" || t == "frame")
    }

    /// 若当前元素是同源 iframe/frame，返回其 [Frame] 以便在 frame 内查找元素；否则返回 None。跨域 iframe 也返回 None。
    pub fn into_frame(self) -> Result<Option<Frame>, CdpError> {
        if !self.is_frame()? {
            return Ok(None);
        }
        let content_document_node_id = self.with_valid_node_id(
            || {
                get_iframe_content_document_node_id(
                    &self.client,
                    &self.session_id,
                    *self.node_id.borrow(),
                )
            },
            |nid| get_iframe_content_document_node_id(&self.client, &self.session_id, nid),
        )?;
        let Some(content_document_node_id) = content_document_node_id else {
            return Ok(None); // 跨域或无法访问
        };
        Ok(Some(Frame::new(
            self.client,
            self.session_id,
            *self.node_id.borrow(),
            content_document_node_id,
        )))
    }

    /// 元素内 HTML（innerHTML）（与 DrissionPage `inner_html` 一致）
    pub fn inner_html(&self) -> Result<String, CdpError> {
        let result = self.call_on("function(){ return this.innerHTML || ''; }", None)?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 所有属性，键值对列表（与 DrissionPage `attrs` 一致）
    pub fn attrs(&self) -> Result<std::collections::HashMap<String, String>, CdpError> {
        let result = self.call_on(
            "function(){ var m=this.attributes; var o={}; for(var i=0;i<m.length;i++){ o[m[i].name]=m[i].value; } return o; }",
            None,
        )?;
        let obj = result
            .get("value")
            .and_then(Value::as_object)
            .ok_or_else(|| CdpError::Protocol {
                id: None,
                code: -1,
                message: "The attrs result was not an object".into(),
            })?;
        let mut map = std::collections::HashMap::new();
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                map.insert(k.clone(), s.to_string());
            }
        }
        Ok(map)
    }

    /// 元素内文本（innerText）（与 DrissionPage `text` 一致）
    pub fn text(&self) -> Result<String, CdpError> {
        let result = self.call_on("function(){ return this.innerText || ''; }", None)?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 元素内文本（textContent，不依赖渲染状态）
    pub fn text_content(&self) -> Result<String, CdpError> {
        let result = self.call_on("function(){ return this.textContent || ''; }", None)?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 元素 HTML
    pub fn html(&self) -> Result<String, CdpError> {
        self.with_valid_node_id(
            || get_outer_html(&self.client, &self.session_id, *self.node_id.borrow()),
            |nid| get_outer_html(&self.client, &self.session_id, nid),
        )
    }

    /// 获取 DOM 属性值，如 value、checked（与 DrissionPage `property(name)` 一致）
    pub fn property(&self, name: &str) -> Result<Value, CdpError> {
        let name_json = serde_json::to_string(name).map_err(CdpError::Json)?;
        let result = self.call_on(
            &format!("function(){{ return this[{}]; }}", name_json),
            None,
        )?;
        Ok(result.get("value").cloned().unwrap_or(Value::Null))
    }

    /// 在当前元素上执行 JS，返回 result（与 DrissionPage 元素 `run_js(script)` 一致，脚本内 this 指向当前元素）
    pub fn run_js(&self, script: &str) -> Result<Value, CdpError> {
        self.call_on(
            "function(scriptBody){ try { return (new Function(scriptBody)).call(this); } catch(e) { throw e; } }",
            Some(json!([{ "value": script }])),
        )
    }

    /// 获取属性值（与 DrissionPage `attr(name)` 一致）
    pub fn attr(&self, name: &str) -> Result<String, CdpError> {
        let name_json = serde_json::to_string(name).map_err(CdpError::Json)?;
        let result = self.call_on(
            &format!(
                "function(){{ var v = this.getAttribute({}); return v !== null ? v : ''; }}",
                name_json
            ),
            None,
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 输入文本（设置 value 并触发 input 事件，适用于 input/textarea）
    pub fn input(&self, text: &str) -> Result<(), CdpError> {
        let text_escaped = serde_json::to_string(text).map_err(CdpError::Json)?;
        self.call_on(
            &format!(
                "function(){{ this.focus(); this.value = {}; this.dispatchEvent(new Event('input', {{ bubbles: true }})); this.dispatchEvent(new Event('change', {{ bubbles: true }})); }}",
                text_escaped
            ),
            None,
        )?;
        Ok(())
    }

    /// 清空内容（value = ''）
    pub fn clear(&self) -> Result<(), CdpError> {
        self.call_on("function(){ this.focus(); this.value = ''; this.dispatchEvent(new Event('input', { bubbles: true })); }", None)?;
        Ok(())
    }

    /// 聚焦
    pub fn focus(&self) -> Result<(), CdpError> {
        self.call_on("function(){ this.focus(); }", None)?;
        Ok(())
    }

    /// 悬停（用 Input.dispatchMouseEvent 到元素中心）
    pub fn hover(&self) -> Result<(), CdpError> {
        let result = self.call_on(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left + r.width/2, y: r.top + r.height/2 }; }",
            None,
        )?;
        let x = result
            .get("value")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let y = result
            .get("value")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        self.client
            .send_with_session("Input.enable", None, Some(self.session_id.as_str()))
            .ok();
        let params = json!({
            "type": "mouseMoved",
            "x": x,
            "y": y
        });
        self.client.send_with_session(
            "Input.dispatchMouseEvent",
            Some(params),
            Some(self.session_id.as_str()),
        )?;
        Ok(())
    }

    /// 是否可见（offsetParent !== null 且 getBoundingClientRect 在视口内粗略判断）
    pub fn is_displayed(&self) -> Result<bool, CdpError> {
        let result = self.call_on(
            "function(){ var r = this.getBoundingClientRect(); return r.width > 0 && r.height > 0 && window.getComputedStyle(this).visibility !== 'hidden' && window.getComputedStyle(this).display !== 'none'; }",
            None,
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// 是否可操作（非 disabled）
    pub fn is_enabled(&self) -> Result<bool, CdpError> {
        let result = self.call_on("function(){ return !this.disabled; }", None)?;
        Ok(result.get("value").and_then(Value::as_bool).unwrap_or(true))
    }

    /// 截取元素区域截图并保存
    pub fn screenshot(&self, path: &str) -> Result<(), CdpError> {
        let result = self.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left, y: r.top, width: r.width, height: r.height }; }",
            None,
        )?;
        let v = result.get("value").ok_or_else(|| CdpError::Protocol {
            id: None,
            code: -1,
            message: "Could not read the element bounding rectangle".into(),
        })?;
        let x = v.get("x").and_then(Value::as_f64).unwrap_or(0.0);
        let y = v.get("y").and_then(Value::as_f64).unwrap_or(0.0);
        let width = v.get("width").and_then(Value::as_f64).unwrap_or(0.0);
        let height = v.get("height").and_then(Value::as_f64).unwrap_or(0.0);
        let params = json!({
            "format": "png",
            "clip": { "x": x, "y": y, "width": width, "height": height, "scale": 1.0 }
        });
        let result = self.client.send_with_session(
            "Page.captureScreenshot",
            Some(params),
            Some(self.session_id.as_str()),
        )?;
        let data_b64 =
            result
                .get("data")
                .and_then(Value::as_str)
                .ok_or_else(|| CdpError::Protocol {
                    id: None,
                    code: -1,
                    message: "Page.captureScreenshot did not return image data".into(),
                })?;
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data_b64)
            .map_err(|e| CdpError::Protocol {
                id: None,
                code: -1,
                message: format!("Failed to decode base64 data: {}", e),
            })?;
        std::fs::write(path, data).map_err(|e| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!("Failed to write the file: {}", e),
        })?;
        Ok(())
    }

    /// 在当前元素下按 CSS 选择器取**第一个匹配子元素的文本**（不创建子元素引用，避免 DOM 更新后 nodeId 失效导致 "Could not find node with given id"）
    pub fn element_text(&self, locator: &str) -> Result<Option<String>, CdpError> {
        self._element_text_inner(locator)
            .map_err(|e| e.with_context(locator))
    }

    fn _element_text_inner(&self, locator: &str) -> Result<Option<String>, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        let Some(selector) = loc.to_css_selector() else {
            return Ok(None);
        };
        let result = self.call_on(
            "function(sel){ try { const el = this.querySelector(sel); return el ? el.textContent : null; } catch(e) { return null; } }",
            Some(json!([{ "value": selector }])),
        )?;
        let value = result.get("value");
        if value.map(Value::is_null) == Some(true) {
            return Ok(None);
        }
        Ok(value.and_then(Value::as_str).map(|s| s.to_string()))
    }

    /// 在当前元素下是否存在匹配选择器的子元素（不创建子元素引用）
    pub fn element_exists(&self, locator: &str) -> Result<bool, CdpError> {
        self._element_exists_inner(locator)
            .map_err(|e| e.with_context(locator))
    }

    fn _element_exists_inner(&self, locator: &str) -> Result<bool, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        let Some(selector) = loc.to_css_selector() else {
            return Ok(false);
        };
        let result = self.call_on(
            "function(sel){ try { return this.querySelector(sel) !== null; } catch(e) { return false; } }",
            Some(json!([{ "value": selector }])),
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// 在当前元素下取第一个匹配子元素的属性值（不创建子元素引用）
    pub fn element_attr(&self, locator: &str, attr: &str) -> Result<Option<String>, CdpError> {
        self._element_attr_inner(locator, attr)
            .map_err(|e| e.with_context(locator))
    }

    fn _element_attr_inner(&self, locator: &str, attr: &str) -> Result<Option<String>, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        let Some(selector) = loc.to_css_selector() else {
            return Ok(None);
        };
        let result = self.call_on(
            "function(sel, attr){ try { var el = this.querySelector(sel); return el ? el.getAttribute(attr) : null; } catch(e) { return null; } }",
            Some(json!([{ "value": selector }, { "value": attr }])),
        )?;
        let value = result.get("value");
        if value.map(Value::is_null) == Some(true) {
            return Ok(None);
        }
        Ok(value.and_then(Value::as_str).map(|s| s.to_string()))
    }

    /// 在当前元素下按选择器取所有匹配子元素的文本列表（不创建子元素引用，一次 callFunctionOn 返回 JSON 数组字符串）
    pub fn element_texts(&self, locator: &str) -> Result<Vec<String>, CdpError> {
        self._element_texts_inner(locator)
            .map_err(|e| e.with_context(locator))
    }

    fn _element_texts_inner(&self, locator: &str) -> Result<Vec<String>, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        let Some(selector) = loc.to_css_selector() else {
            return Ok(Vec::new());
        };
        let result = self.call_on(
            "function(sel){ try { var nodes = this.querySelectorAll(sel); var arr = []; for(var i=0;i<nodes.length;i++) { var t = (nodes[i].textContent||'').trim(); if(t) arr.push(t); } return JSON.stringify(arr); } catch(e) { return '[]'; } }",
            Some(json!([{ "value": selector }])),
        )?;
        let s = result.get("value").and_then(Value::as_str).unwrap_or("[]");
        let arr: Vec<String> = serde_json::from_str(s).unwrap_or_else(|_| Vec::new());
        Ok(arr)
    }

    /// 在当前元素下按定位器查单个子元素
    pub fn element(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        self._element_inner(locator)
            .map_err(|e| e.with_context(locator))
    }

    /// element 内部实现
    fn _element_inner(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        if let Some(selector) = loc.to_css_selector() {
            // 优先在当前元素 objectId 上执行 querySelector，避免动态 DOM 下 nodeId 易失效问题。
            let result = self.call_on(
                "function(sel){ try { return this.querySelector(sel); } catch(e) { return null; } }",
                Some(json!([{ "value": selector }])),
            )?;
            if let Some(oid) = result.get("objectId").and_then(Value::as_str) {
                let params = json!({ "objectId": oid });
                let node_res = self.client.send_with_session(
                    "DOM.requestNode",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                if let Some(nid) = node_res.get("nodeId").and_then(Value::as_i64) {
                    let backend_node_id =
                        get_backend_node_id(&self.client, &self.session_id, nid).ok();
                    return Ok(Some(Element::new_with_object_id(
                        Arc::clone(&self.client),
                        self.session_id.clone(),
                        nid,
                        Some(oid.to_string()),
                        backend_node_id,
                    )));
                }
            }
            // 兼容回退：少数页面可能无法通过 callFunctionOn 获取可请求节点，保留原 DOM.querySelector 路径。
            let node_id = self.with_valid_node_id(
                || {
                    query_selector(
                        &self.client,
                        &self.session_id,
                        *self.node_id.borrow(),
                        &selector,
                    )
                },
                |nid| query_selector(&self.client, &self.session_id, nid, &selector),
            )?;
            Ok(node_id.map(|id| {
                let backend_node_id = get_backend_node_id(&self.client, &self.session_id, id).ok();
                Element::new_with_backend(
                    Arc::clone(&self.client),
                    self.session_id.clone(),
                    id,
                    backend_node_id,
                )
            }))
        } else if let Some(xpath) = loc.to_xpath_expression() {
            let result = self.call_on(
                &format!(
                    "function(){{ var r = document.evaluate({}, this, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); return r.singleNodeValue; }}",
                    serde_json::to_string(&xpath).map_err(CdpError::Json)?
                ),
                None,
            )?;
            let obj_id = result.get("objectId").and_then(Value::as_str);
            if let Some(oid) = obj_id {
                let params = json!({ "objectId": oid });
                let res = self.client.send_with_session(
                    "DOM.requestNode",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                if let Some(nid) = res.get("nodeId").and_then(Value::as_i64) {
                    return Ok(Some(Element::new(
                        Arc::clone(&self.client),
                        self.session_id.clone(),
                        nid,
                    )));
                }
            }
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// 在当前元素下按定位器查多个子元素
    pub fn elements(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        self._elements_inner(locator)
            .map_err(|e| e.with_context(locator))
    }

    /// elements 内部实现
    fn _elements_inner(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        let loc = crate::locator::Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!(
                "Invalid locator: {}. Please check the locator syntax.",
                locator
            ),
        })?;
        if let Some(selector) = loc.to_css_selector() {
            let sel = serde_json::to_string(&selector).map_err(CdpError::Json)?;
            let result = self.call_on(
                &format!(
                    "function(){{ return Array.from(this.querySelectorAll({})); }}",
                    sel
                ),
                None,
            )?;
            let obj_id = result.get("objectId").and_then(Value::as_str);
            let mut out = Vec::new();
            if let Some(oid) = obj_id {
                let params = json!({ "functionDeclaration": "function(){ return this.length; }", "objectId": oid });
                let res = self.client.send_with_session(
                    "Runtime.callFunctionOn",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                let len = res
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                for i in 0..len {
                    let params = json!({ "functionDeclaration": "function(i){ return this[i]; }", "objectId": oid, "arguments": [{"value": i}] });
                    let res = self.client.send_with_session(
                        "Runtime.callFunctionOn",
                        Some(params),
                        Some(self.session_id.as_str()),
                    )?;
                    let eid = res
                        .get("result")
                        .and_then(|r| r.get("objectId"))
                        .and_then(Value::as_str);
                    if let Some(eid) = eid {
                        let params = json!({ "objectId": eid });
                        let node_res = self.client.send_with_session(
                            "DOM.requestNode",
                            Some(params),
                            Some(self.session_id.as_str()),
                        )?;
                        if let Some(nid) = node_res.get("nodeId").and_then(Value::as_i64) {
                            let backend_node_id =
                                get_backend_node_id(&self.client, &self.session_id, nid).ok();
                            out.push(Element::new_with_object_id(
                                Arc::clone(&self.client),
                                self.session_id.clone(),
                                nid,
                                Some(eid.to_string()),
                                backend_node_id,
                            ));
                        }
                    }
                }
            }
            Ok(out)
        } else if let Some(xpath) = loc.to_xpath_expression() {
            let result = self.call_on(
                &format!(
                    "function(){{ var r = document.evaluate({}, this, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); var a = []; for(var i=0;i<r.snapshotLength;i++) a.push(r.snapshotItem(i)); return a; }}",
                    serde_json::to_string(&xpath).map_err(CdpError::Json)?
                ),
                None,
            )?;
            let obj_id = result.get("objectId").and_then(Value::as_str);
            let mut elements = Vec::new();
            if let Some(oid) = obj_id {
                let params = json!({ "functionDeclaration": "function(){ return this.length; }", "objectId": oid });
                let res = self.client.send_with_session(
                    "Runtime.callFunctionOn",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                let len = res
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                for i in 0..len {
                    let params = json!({ "functionDeclaration": "function(i){ return this[i]; }", "objectId": oid, "arguments": [{"value": i}] });
                    let res = self.client.send_with_session(
                        "Runtime.callFunctionOn",
                        Some(params),
                        Some(self.session_id.as_str()),
                    )?;
                    let eid = res
                        .get("result")
                        .and_then(|r| r.get("objectId"))
                        .and_then(Value::as_str);
                    if let Some(eid) = eid {
                        let params = json!({ "objectId": eid });
                        let node_res = self.client.send_with_session(
                            "DOM.requestNode",
                            Some(params),
                            Some(self.session_id.as_str()),
                        )?;
                        if let Some(nid) = node_res.get("nodeId").and_then(Value::as_i64) {
                            elements.push(Element::new(
                                Arc::clone(&self.client),
                                self.session_id.clone(),
                                nid,
                            ));
                        }
                    }
                }
            }
            Ok(elements)
        } else {
            Ok(Vec::new())
        }
    }

    /// 获取父元素（`level=1` 表示直接父元素）
    pub fn parent(&self, level: u32) -> Result<Option<Element>, CdpError> {
        let script = format!(
            "function(){{ var el = this; for(var i=0;i<{};i++){{ if(el.parentElement){{el=el.parentElement;}}else{{return null;}} }} return el; }}",
            level
        );
        let result = self.call_on(&script, None)?;
        let obj_id = result.get("objectId").and_then(Value::as_str);
        if let Some(oid) = obj_id {
            let res = describe_node_by_object_id(&self.client, &self.session_id, oid)?;
            if let (Some(nid), bid) = res {
                return Ok(Some(Element::new_with_backend(
                    Arc::clone(&self.client),
                    self.session_id.clone(),
                    nid,
                    bid,
                )));
            }
        }
        Ok(None)
    }

    /// 获取第一个匹配子元素（直接子节点）
    pub fn child(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        self.element(locator)
    }

    /// 获取前一个兄弟元素
    pub fn prev(&self) -> Result<Option<Element>, CdpError> {
        let result = self.call_on("function(){ return this.previousElementSibling; }", None)?;
        let obj_id = result.get("objectId").and_then(Value::as_str);
        if let Some(oid) = obj_id {
            let res = describe_node_by_object_id(&self.client, &self.session_id, oid)?;
            if let (Some(nid), bid) = res {
                return Ok(Some(Element::new_with_backend(
                    Arc::clone(&self.client),
                    self.session_id.clone(),
                    nid,
                    bid,
                )));
            }
        }
        Ok(None)
    }

    /// 获取后一个兄弟元素
    pub fn next(&self) -> Result<Option<Element>, CdpError> {
        let result = self.call_on("function(){ return this.nextElementSibling; }", None)?;
        let obj_id = result.get("objectId").and_then(Value::as_str);
        if let Some(oid) = obj_id {
            let res = describe_node_by_object_id(&self.client, &self.session_id, oid)?;
            if let (Some(nid), bid) = res {
                return Ok(Some(Element::new_with_backend(
                    Arc::clone(&self.client),
                    self.session_id.clone(),
                    nid,
                    bid,
                )));
            }
        }
        Ok(None)
    }

    /// 获取所有子元素（直接子节点，不含文本节点）
    pub fn children(&self) -> Result<Vec<Element>, CdpError> {
        let result = self.call_on("function(){ return Array.from(this.children); }", None)?;
        let obj_id = result.get("objectId").and_then(Value::as_str);
        let mut elements = Vec::new();
        if let Some(oid) = obj_id {
            let params = json!({ "functionDeclaration": "function(){ return this.length; }", "objectId": oid });
            let res = self.client.send_with_session(
                "Runtime.callFunctionOn",
                Some(params),
                Some(self.session_id.as_str()),
            )?;
            let len = res
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            for i in 0..len {
                let params = json!({ "functionDeclaration": "function(i){ return this[i]; }", "objectId": oid, "arguments": [{"value": i}] });
                let res = self.client.send_with_session(
                    "Runtime.callFunctionOn",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                let eid = res
                    .get("result")
                    .and_then(|r| r.get("objectId"))
                    .and_then(Value::as_str);
                if let Some(eid) = eid {
                    let desc = describe_node_by_object_id(&self.client, &self.session_id, eid)?;
                    if let (Some(nid), bid) = desc {
                        elements.push(Element::new_with_backend(
                            Arc::clone(&self.client),
                            self.session_id.clone(),
                            nid,
                            bid,
                        ));
                    }
                }
            }
        }
        Ok(elements)
    }

    /// 获取元素边界矩形（与 DrissionPage `rect` 一致）
    pub fn rect(&self) -> Result<Value, CdpError> {
        let result = self.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left, y: r.top, width: r.width, height: r.height, top: r.top, bottom: r.bottom, left: r.left, right: r.right }; }",
            None,
        )?;
        Ok(result.get("value").cloned().unwrap_or(Value::Null))
    }

    /// 勾选或取消勾选 checkbox/radio（与 DrissionPage `check`/`uncheck` 一致）
    pub fn check(&self, uncheck: bool) -> Result<(), CdpError> {
        if uncheck {
            // 先获取当前状态，再决定是否点击
            let checked = self.property("checked")?;
            if checked.as_bool().unwrap_or(false) {
                self.click()?;
            }
        } else {
            let checked = self.property("checked")?;
            if !checked.as_bool().unwrap_or(false) {
                self.click()?;
            }
        }
        Ok(())
    }

    /// 从下拉框选择选项（与 DrissionPage `select` 一致）
    /// `text_or_value` - 要选择的 option 文本或 value
    /// `by_text` - true=按文本匹配，false=按 value 匹配
    pub fn select(&self, text_or_value: &str, by_text: bool) -> Result<(), CdpError> {
        let tag = self.tag()?;
        if tag != "select" {
            return Err(CdpError::Protocol {
                id: None,
                code: -1,
                message: format!("select() can only be used on <select> elements, but the current element is <{}>", tag),
            });
        }
        let escaped = serde_json::to_string(text_or_value).map_err(CdpError::Json)?;
        let script = if by_text {
            format!(
                "function(){{ var opts = this.options; for(var i=0;i<opts.length;i++){{ if(opts[i].text === {}){{ this.selectedIndex = i; this.dispatchEvent(new Event('change', {{bubbles:true}})); return true; }} }} return false; }}",
                escaped
            )
        } else {
            format!(
                "function(){{ var opts = this.options; for(var i=0;i<opts.length;i++){{ if(opts[i].value === {}){{ this.selectedIndex = i; this.dispatchEvent(new Event('change', {{bubbles:true}})); return true; }} }} return false; }}",
                escaped
            )
        };
        self.call_on(&script, None)?;
        Ok(())
    }

    /// 获取表单元素的值
    pub fn value(&self) -> Result<String, CdpError> {
        let result = self.call_on("function(){ return this.value || ''; }", None)?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 拖拽到目标位置（相对偏移）
    /// `offset_x` / `offset_y` - 相对当前元素的偏移（像素）
    /// `duration` - 拖拽持续时间（毫秒）
    pub fn drag(&self, offset_x: i64, offset_y: i64, duration: u64) -> Result<(), CdpError> {
        let result = self.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left + r.width/2, y: r.top + r.height/2 }; }",
            None,
        )?;
        let start_x = result
            .get("value")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let start_y = result
            .get("value")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let end_x = start_x + offset_x as f64;
        let end_y = start_y + offset_y as f64;
        self.drag_by_coords(start_x, start_y, end_x, end_y, duration)
    }

    /// 拖拽到目标元素或坐标
    /// `target` - 目标元素或坐标 JSON `{"x":100,"y":200}`
    /// `duration` - 拖拽持续时间（毫秒）
    pub fn drag_to(&self, target: &Element, duration: u64) -> Result<(), CdpError> {
        let start_result = self.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left + r.width/2, y: r.top + r.height/2 }; }",
            None,
        )?;
        let start_x = start_result
            .get("value")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let start_y = start_result
            .get("value")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let end_result = target.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left + r.width/2, y: r.top + r.height/2 }; }",
            None,
        )?;
        let end_x = end_result
            .get("value")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let end_y = end_result
            .get("value")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        self.drag_by_coords(start_x, start_y, end_x, end_y, duration)
    }

    fn drag_by_coords(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        duration: u64,
    ) -> Result<(), CdpError> {
        let _ = self
            .client
            .send_with_session("Input.enable", None, Some(self.session_id.as_str()));
        // mousePressed
        let params_press = json!({
            "type": "mousePressed",
            "x": start_x,
            "y": start_y,
            "button": "left",
            "clickCount": 1
        });
        self.client.send_with_session(
            "Input.dispatchMouseEvent",
            Some(params_press),
            Some(self.session_id.as_str()),
        )?;
        // 模拟拖拽路径（简化为直接移动）
        let steps = (duration as f64 / 16.0).ceil() as u32; // ~60fps
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let x = start_x + (end_x - start_x) * t;
            let y = start_y + (end_y - start_y) * t;
            let params_move = json!({
                "type": "mouseMoved",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 0
            });
            self.client.send_with_session(
                "Input.dispatchMouseEvent",
                Some(params_move),
                Some(self.session_id.as_str()),
            )?;
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
        // mouseReleased
        let params_release = json!({
            "type": "mouseReleased",
            "x": end_x,
            "y": end_y,
            "button": "left",
            "clickCount": 1
        });
        self.client.send_with_session(
            "Input.dispatchMouseEvent",
            Some(params_release),
            Some(self.session_id.as_str()),
        )?;
        Ok(())
    }

    /// 悬停到元素（可带偏移）
    pub fn hover_at(&self, offset_x: Option<f64>, offset_y: Option<f64>) -> Result<(), CdpError> {
        let result = self.call_on_value(
            "function(){ var r = this.getBoundingClientRect(); return { x: r.left + r.width/2, y: r.top + r.height/2 }; }",
            None,
        )?;
        let base_x = result
            .get("value")
            .and_then(|v| v.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let base_y = result
            .get("value")
            .and_then(|v| v.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let x = base_x + offset_x.unwrap_or(0.0);
        let y = base_y + offset_y.unwrap_or(0.0);
        self.client
            .send_with_session("Input.enable", None, Some(self.session_id.as_str()))
            .ok();
        let params = json!({
            "type": "mouseMoved",
            "x": x,
            "y": y
        });
        self.client.send_with_session(
            "Input.dispatchMouseEvent",
            Some(params),
            Some(self.session_id.as_str()),
        )?;
        Ok(())
    }

    /// 将元素滚动到可见区域（与 DrissionPage `over` 一致）
    pub fn scroll_into_view(&self) -> Result<(), CdpError> {
        self.call_on(
            "function(){ this.scrollIntoView({ block: 'center', inline: 'center' }); }",
            None,
        )?;
        Ok(())
    }

    /// 在元素内滚动
    pub fn scroll(&self, x: i64, y: i64) -> Result<(), CdpError> {
        let script = format!("function(){{ this.scrollTo({}, {}); }}", x, y);
        self.call_on(&script, None)?;
        Ok(())
    }

    /// 删除属性
    pub fn remove_attr(&self, name: &str) -> Result<(), CdpError> {
        let name_json = serde_json::to_string(name).map_err(CdpError::Json)?;
        self.call_on(
            &format!("function(){{ this.removeAttribute({}); }}", name_json),
            None,
        )?;
        Ok(())
    }

    /// 获取内联样式
    pub fn style(&self) -> Result<String, CdpError> {
        let result = self.call_on(
            "function(){ return this.getAttribute('style') || ''; }",
            None,
        )?;
        Ok(result
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    /// 移除元素（从 DOM 中删除）
    pub fn remove(&self) -> Result<(), CdpError> {
        self.call_on("function(){ this.remove(); }", None)?;
        Ok(())
    }
}
