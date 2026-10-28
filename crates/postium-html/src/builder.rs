//!
//! # 文档树构建（Task 4 前 stub）
//!
//! 本模块是渲染管线的最后一环：把内联后的 HTML 解析为
//! [`HtmlDocument`](crate::types::HtmlDocument) 中间表示。
//!
//! 当前为占位实现（恒返回空文档），由 Task 4 接管为 html5ever 真实解析：
//! 节点数上限（[`Limits::max_nodes`]）、`display:none` 剪枝、
//! `SafeUrl` / `ImageRef` 终审白名单（spec §3 / §4.4）。

use crate::error::HtmlError;
use crate::types::{HtmlDocument, Limits};

/// 解析 HTML 为中间表示（Task 4 前为 stub：恒返回空文档）。
pub fn build_document(html: &str, limits: &Limits) -> Result<HtmlDocument, HtmlError> {
    let _ = html;
    let _ = limits;
    Ok(HtmlDocument { blocks: Vec::new() })
}
