//!
//! # 渲染管线（spec §4，阶段顺序经裁定修正：内联先于净化）
//!
//! [`render_pipeline`] 串联全部阶段：
//!
//! 1. 字节数检查：超过 [`Limits::max_bytes`] 直接 [`HtmlError::TooLarge`]（调用方降级纯文本）；
//! 2. [`strip_mso_comments`]：MSO 条件注释整体删除；
//! 3. [`inline_css`]：css-inline 把 `<style>` 规则内联为元素 `style` 属性；
//! 4. [`sanitize_lenient`]：ammonia 宽松净化；
//! 5. [`build_document`](crate::builder::build_document)：解析为中间表示（Task 4 接管）。
//!
//! ## 为什么内联在净化之前
//!
//! ammonia 净化会把 `<style>` 块连同内容整体删除（默认 `clean_content_tags`），
//! 内联放在净化之后将永远无事可做。css-inline 在未净化 HTML 上只读 `<style>`
//! 块、只写元素 `style` 属性，不引入新的攻击面；script/iframe 等危险元素仍由
//! 后置 [`sanitize_lenient`] 全量拦截（远程样式表已禁用，见 [`inliner`]）。

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

/// 管线预处理：剥 MSO → 内联样式 → 宽松净化，返回预处理后 HTML。
///
/// 独立成函数供测试断言中间产物（管线级内联 + 净化兜底一次锁定），
/// [`render_pipeline`] 与之共用同一实现。
pub(crate) fn preprocess(html: &str) -> String {
    let no_mso = strip_mso_comments(html);
    let inlined = inline_css(&no_mso);
    sanitize_lenient(&inlined)
}

/// 完整渲染管线：原始 HTML → [`HtmlDocument`] 中间表示。
///
/// 顺序见模块文档；超限错误（[`HtmlError::TooLarge`]）由调用方捕获并降级为纯文本。
pub fn render_pipeline(html: &str) -> Result<HtmlDocument, HtmlError> {
    let limits = Limits::default();
    if html.len() > limits.max_bytes {
        return Err(HtmlError::TooLarge(html.len()));
    }
    let preprocessed = preprocess(html);
    builder::build_document(&preprocessed, &limits)
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
    #[test]
    fn preprocess_inlines_styles_and_strips_scripts() {
        let html = "<style>h1{color:#ff0000}</style><h1>标题</h1><script>alert(1)</script>";
        let out = preprocess(html);
        // 管线级内联：规则值出现在元素 style 属性中（内联先于净化才可能发生）。
        assert!(out.contains("style=") && out.contains("ff0000"));
        // 净化兜底：script 标签与内容剔除，`<style>` 块不再残留。
        assert!(!out.contains("<style>") && !out.contains("script"));
    }

    #[test]
    fn pipeline_builds_document_from_fragment() {
        // Task 4 接管：stub（恒空文档）移除后，管线产出真实块级树
        let doc = render_pipeline(r#"<div style="padding:4px"><p>hi</p></div>"#).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(
            &doc.blocks[0],
            crate::types::Block::Container { .. }
        ));
    }

    #[test]
    fn pipeline_empty_html_is_empty_error() {
        assert!(matches!(render_pipeline(""), Err(HtmlError::Empty)));
    }

    #[test]
    fn pipeline_full_document_head_not_leaked() {
        // 评审 I-1：ammonia 以 fragment 解析、剥 html/head/body 壳但把 head 内容
        // 重挂到顶层，<title> 不在白名单被解包 → 标题文本曾泄漏为正文首段。
        let html = "<html><head><title>邮 件</title></head><body><p>正文</p></body></html>";
        let doc = render_pipeline(html).unwrap();
        assert_eq!(doc.blocks.len(), 1, "只应剩下正文一个段落");
        let dump = format!("{doc:?}");
        assert!(dump.contains("正文"), "正文应保留");
        assert!(!dump.contains("邮 件"), "head 中的标题文本不应泄漏进正文");
    }
}
