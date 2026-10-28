//!
//! # 渲染管线（spec §4，对应 crate 文档注释中的阶段图）
//!
//! [`render_pipeline`] 串联全部预处理阶段：
//!
//! 1. 字节数检查：超过 [`Limits::max_bytes`] 直接 [`HtmlError::TooLarge`]（调用方降级纯文本）；
//! 2. [`strip_mso_comments`]：MSO 条件注释整体删除；
//! 3. [`sanitize_lenient`]：ammonia 宽松净化；
//! 4. [`inline_css`]：css-inline 把 `<style>` 规则内联为元素 `style` 属性；
//! 5. [`build_document`](crate::builder::build_document)：解析为中间表示（Task 4 接管）。
//!
//! 单独暴露 [`inline_css`] 供测试与后续阶段复用。

use crate::builder;
use crate::error::HtmlError;
use crate::sanitize::{sanitize_lenient, strip_mso_comments};
use crate::types::{HtmlDocument, Limits};

use css_inline::CSSInliner;

/// [`inline_css`] 用的内联器配置：只内联文档内 `<style>` 块，不加载外部样式表。
///
/// `load_remote_stylesheets(false)` 同时是隐私边界（spec §4.4：邮件正文远程资源不放行）
/// 与行为保障：外部样式表获取是 css-inline 唯一的整体报错来源，关闭后 `<style>`
/// 解析错误只剩 cssparser 的逐规则错误恢复，不会让内联整体失败。
fn inliner() -> CSSInliner<'static> {
    CSSInliner::options().load_remote_stylesheets(false).build()
}

/// 将文档中全部 `<style>` 块的规则内联为元素 `style` 属性（css-inline 包装）。
///
/// - 只处理文档内 `<style>` 块，不加载外部样式表（远程资源不放行，邮件隐私）；
/// - 单个 `<style>` 块内的畸形规则由 cssparser 的错误恢复机制跳过，
///   不影响其余规则与其后的内容（不整体失败）；
/// - css-inline 整体报错时原样返回输入（降级为"无内联"，后续阶段照常处理）。
pub fn inline_css(html: &str) -> String {
    inliner().inline(html).unwrap_or_else(|_| html.to_string())
}

/// 完整渲染管线：原始 HTML → [`HtmlDocument`] 中间表示。
///
/// 顺序见模块文档；超限错误（[`HtmlError::TooLarge`]）由调用方捕获并降级为纯文本。
pub fn render_pipeline(html: &str) -> Result<HtmlDocument, HtmlError> {
    let limits = Limits::default();
    if html.len() > limits.max_bytes {
        return Err(HtmlError::TooLarge(html.len()));
    }
    let no_mso = strip_mso_comments(html);
    let clean = sanitize_lenient(&no_mso);
    let inlined = inline_css(&clean);
    builder::build_document(&inlined, &limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inlines_style_block() {
        let html = "<style>h1{color:#ff0000}</style><h1>标题</h1>";
        let out = inline_css(html);
        assert!(out.contains("h1") && out.contains("ff0000") && !out.contains("<style>"));
    }
    #[test]
    fn malformed_style_does_not_fail() {
        let out = inline_css("<style>h1{color:</style><p>t</p>");
        assert!(out.contains("<p>t</p>"));
    }
    #[test]
    fn oversized_input_rejected() {
        let big = format!("<div>{}</div>", "x".repeat(1024 * 1024 + 10));
        assert!(matches!(render_pipeline(&big), Err(HtmlError::TooLarge(_))));
    }
}
