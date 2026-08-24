//! DOM 查询：基于 CDP DOM / Runtime 的 getDocument、querySelector、getOuterHTML 等

mod query;

pub use query::{
    discard_search_results, get_backend_node_id, get_document_root,
    get_iframe_content_document_node_id, get_node_id_from_backend, get_outer_html,
    get_search_results, perform_search, query_selector, query_selector_all,
    query_selector_all_including_same_origin_frames, query_selector_all_under_root,
    resolve_backend_to_object_id, resolve_node_to_object_id,
};
