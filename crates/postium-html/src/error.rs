//!
//! # 错误类型
//!
//! 管线各阶段的可预期失败（spec §8）。所有变体均不应触发 panic：
//! 超限 / 空内容由调用方（email 视图层）捕获并降级为纯文本显示。

use thiserror::Error;

/// HTML 渲染管线错误。
#[derive(Debug, Clone, PartialEq, Error)]
pub enum HtmlError {
    /// HTML 超过字节限制（携带实际字节数，spec §8 → 纯文本降级）。
    #[error("HTML 过大：{0} 字节，超出限制")]
    TooLarge(usize),

    /// 解析树节点数超限（携带实际节点数，spec §8 → 纯文本降级）。
    #[error("HTML 节点过多：{0} 个，超出限制")]
    TooManyNodes(usize),

    /// 净化后无有效内容（spec §8 → 原文本 trim 后纯文本降级）。
    #[error("HTML 为空或净化后无有效内容")]
    Empty,
}
