//! iframe 作为普通元素：在同源 iframe 内查找元素，无需切入切出（与 DrissionPage ChromiumFrame 一致）

use crate::cdp::{CdpClient, CdpError};
use crate::dom::{
    get_backend_node_id, get_iframe_content_document_node_id, query_selector,
    query_selector_all_under_root, resolve_node_to_object_id,
};
use crate::element::Element;
use crate::locator::Locator;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;

fn is_no_node_error(e: &CdpError) -> bool {
    match e {
        CdpError::Protocol { message, .. } => {
            message.contains("No node with given id")
                || message.contains("Could not find node with given id")
        }
        _ => false,
    }
}

/// 同源 iframe 的封装，可在该 frame 内直接查找元素（把 iframe 看作普通元素，逻辑更清晰）。
pub struct Frame {
    pub(crate) client: Arc<CdpClient>,
    pub(crate) session_id: String,
    /// iframe/frame 元素自身的 nodeId（对应 Python 的 frame_ele）
    pub(crate) frame_element_node_id: i64,
    /// contentDocument 的 nodeId（RefCell 以便 DOM 更新后从 iframe 元素重新取用）
    pub(crate) content_document_node_id: RefCell<i64>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("frame_element_node_id", &self.frame_element_node_id)
            .finish_non_exhaustive()
    }
}

impl Frame {
    pub(crate) fn new(
        client: Arc<CdpClient>,
        session_id: String,
        frame_element_node_id: i64,
        content_document_node_id: i64,
    ) -> Self {
        Self {
            client,
            session_id,
            frame_element_node_id,
            content_document_node_id: RefCell::new(content_document_node_id),
        }
    }

    /// contentDocument 失效时（如 Vue 重渲染）从 iframe 元素重新取用并更新，便于后续重试。
    fn try_refresh_content_document(&self) -> Result<(), CdpError> {
        let fresh = get_iframe_content_document_node_id(
            &self.client,
            &self.session_id,
            self.frame_element_node_id,
        )?;
        if let Some(nid) = fresh {
            *self.content_document_node_id.borrow_mut() = nid;
        }
        Ok(())
    }

    /// 返回 iframe 元素本身（与 DrissionPage `frame_ele` 一致）
    pub fn frame_ele(&self) -> Element {
        Element::new(
            Arc::clone(&self.client),
            self.session_id.clone(),
            self.frame_element_node_id,
        )
    }

    /// 在本 frame 内按定位器查单个元素（与 DrissionPage 在 ChromiumFrame 上 ele() 一致）
    pub fn ele(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        self._ele_inner(locator).map_err(|e| e.with_context(locator))
    }

    /// ele 内部实现
    fn _ele_inner(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        let loc = Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!("Invalid locator: {}. Please check the locator syntax.", locator),
        })?;
        let do_ele = |root: i64| -> Result<Option<Element>, CdpError> {
            if let Some(selector) = loc.to_css_selector() {
                let node_id = query_selector(&self.client, &self.session_id, root, &selector)?;
                return Ok(node_id.map(|id| {
                    Element::new(Arc::clone(&self.client), self.session_id.clone(), id)
                }));
            }
            if let Some(xpath) = loc.to_xpath_expression() {
                let doc_oid = resolve_node_to_object_id(&self.client, &self.session_id, root)?;
                let params = json!({
                    "functionDeclaration": "function(xpath){ var r = this.evaluate(xpath, this, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); return r.singleNodeValue; }",
                    "objectId": doc_oid,
                    "arguments": [{"value": xpath}]
                });
                let result = self.client.send_with_session(
                    "Runtime.callFunctionOn",
                    Some(params),
                    Some(self.session_id.as_str()),
                )?;
                let obj_id = result
                    .get("result")
                    .and_then(|r| r.get("objectId"))
                    .and_then(Value::as_str);
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
                return Ok(None);
            }
            Ok(None)
        };
        let root = *self.content_document_node_id.borrow();
        match do_ele(root) {
            Ok(out) => Ok(out),
            Err(e) if is_no_node_error(&e) => {
                let _ = self.try_refresh_content_document();
                do_ele(*self.content_document_node_id.borrow())
            }
            Err(e) => Err(e),
        }
    }

    /// 在本 frame 内按定位器查多个元素（与 DrissionPage 在 ChromiumFrame 上 eles() 一致）
    pub fn eles(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        self._eles_inner(locator).map_err(|e| e.with_context(locator))
    }

    /// eles 内部实现
    fn _eles_inner(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        let loc = Locator::parse(locator).map_err(|_| CdpError::Protocol {
            id: None,
            code: -1,
            message: format!("Invalid locator: {}. Please check the locator syntax.", locator),
        })?;
        let do_eles = |root: i64| -> Result<Vec<Element>, CdpError> {
            if let Some(selector) = loc.to_css_selector() {
                let node_ids =
                    query_selector_all_under_root(&self.client, &self.session_id, root, &selector)?;
                let backends: Vec<Option<i64>> = node_ids
                    .iter()
                    .map(|&nid| get_backend_node_id(&self.client, &self.session_id, nid).ok())
                    .collect();
                return Ok(node_ids
                    .into_iter()
                    .zip(backends)
                    .map(|(id, b)| {
                        Element::new_with_backend(
                            Arc::clone(&self.client),
                            self.session_id.clone(),
                            id,
                            b,
                        )
                    })
                    .collect());
            }
            if let Some(xpath) = loc.to_xpath_expression() {
                let doc_oid = resolve_node_to_object_id(&self.client, &self.session_id, root)?;
            let expr = format!(
                "function(xpath){{ var r = this.evaluate(xpath, this, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); var a = []; for(var i = 0; i < r.snapshotLength; i++) a.push(r.snapshotItem(i)); return a; }}"
            );
            let params = json!({
                "functionDeclaration": expr,
                "objectId": doc_oid,
                "arguments": [{"value": xpath}]
            });
            let result = self.client.send_with_session(
                "Runtime.callFunctionOn",
                Some(params),
                Some(self.session_id.as_str()),
            )?;
            let obj_id = result
                .get("result")
                .and_then(|r| r.get("objectId"))
                .and_then(Value::as_str);
            let mut node_ids = Vec::new();
            if let Some(oid) = obj_id {
                let params = json!({
                    "functionDeclaration": "function(){ return this.length; }",
                    "objectId": oid
                });
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
                    let params = json!({
                        "functionDeclaration": "function(i){ return this[i]; }",
                        "objectId": oid,
                        "arguments": [{"value": i}]
                    });
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
                            node_ids.push(nid);
                        }
                    }
                }
            }
            let backends: Vec<Option<i64>> = node_ids
                .iter()
                .map(|&nid| get_backend_node_id(&self.client, &self.session_id, nid).ok())
                .collect();
            return Ok(node_ids
                .into_iter()
                .zip(backends)
                .map(|(id, b)| {
                    Element::new_with_backend(
                        Arc::clone(&self.client),
                        self.session_id.clone(),
                        id,
                        b,
                    )
                })
                .collect());
            }
            Ok(Vec::new())
        };
        let root = *self.content_document_node_id.borrow();
        match do_eles(root) {
            Ok(out) => Ok(out),
            Err(e) if is_no_node_error(&e) => {
                let _ = self.try_refresh_content_document();
                do_eles(*self.content_document_node_id.borrow())
            }
            Err(e) => Err(e),
        }
    }

    /// 与 [Frame::ele] 一致
    pub fn element(&self, locator: &str) -> Result<Option<Element>, CdpError> {
        self.ele(locator)
    }

    /// 与 [Frame::eles] 一致
    pub fn elements(&self, locator: &str) -> Result<Vec<Element>, CdpError> {
        self.eles(locator)
    }

    /// 在本 frame 内等待定位器匹配到元素，超时返回错误（与 Page::wait 一致，作用域为当前 frame）。
    pub fn wait(&self, locator: &str, timeout: Duration) -> Result<Element, CdpError> {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if let Some(el) = self.ele(locator)? {
                return Ok(el);
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Err(CdpError::Protocol {
            id: None,
            code: -1,
            message: format!("Timed out while waiting for an element inside the frame: {}", locator),
        })
    }

    /// 在本 frame 的 document 上下文中执行 JS（脚本内可用 document，与主页面 run_js 一致），返回 CDP 的 result。
    pub fn run_js(&self, script: &str) -> Result<Value, CdpError> {
        let do_run = |root: i64| -> Result<Value, CdpError> {
            let doc_oid = resolve_node_to_object_id(
                &self.client,
                &self.session_id,
                root,
            )?;
            let fun = format!("function() {{ var document = this; {} }}", script);
            let params = json!({
                "functionDeclaration": fun,
                "objectId": doc_oid,
                "userGesture": true
            });
            let result = self.client.send_with_session(
                "Runtime.callFunctionOn",
                Some(params),
                Some(self.session_id.as_str()),
            )?;
            Ok(result
                .get("result")
                .cloned()
                .unwrap_or(Value::Null))
        };
        let root = *self.content_document_node_id.borrow();
        match do_run(root) {
            Ok(v) => Ok(v),
            Err(e) if is_no_node_error(&e) => {
                let _ = self.try_refresh_content_document();
                do_run(*self.content_document_node_id.borrow())
            }
            Err(e) => Err(e),
        }
    }
}
