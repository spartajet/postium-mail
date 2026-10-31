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
/// 在 **32MB 大栈线程**上执行（Windows 主线程默认仅 1MB）：管线内有多处
/// 与输入嵌套深度成正比的递归——html5ever 解析产物 rcdom 的**递归 Drop**
///（发生在我们的 [`crate::builder::MAX_TREE_DEPTH`] 剪枝之前）、css-inline
/// 内部处理、树遍历自身——深嵌套邮件（如层层转发的引用链）在主线程直接
/// 栈溢出。大栈线程承担全部解析侧递归；主线程只剩渲染层 ≤ `MAX_TREE_DEPTH`
/// 的有界递归。
pub fn render_pipeline(html: &str) -> Result<HtmlDocument, HtmlError> {
    let limits = Limits::default();
    if html.len() > limits.max_bytes {
        return Err(HtmlError::TooLarge(html.len()));
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn_scoped(scope, || {
                let preprocessed = preprocess(html);
                builder::build_document(&preprocessed, &limits)
            })
            .expect("管线线程启动失败")
            .join()
            .expect("管线线程 panic")
    })
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
        let big = format!("<div>{}</div>", "x".repeat(4 * 1024 * 1024 + 10));
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
    fn deep_nesting_on_default_thread_no_stack_overflow() {
        // 线上回归（Windows 主线程 1MB 栈溢出）：3000 层嵌套的转发链在
        // 默认线程直接跑管线——解析侧递归（rcdom 递归 Drop 等）由 32MB
        // 大栈线程承担，超 MAX_TREE_DEPTH 子树静默剪枝，不再溢出。
        let depth = 3000usize;
        let mut html = String::with_capacity(depth * 24);
        for i in 0..depth {
            html.push_str(&format!(
                r##"<div class="q{i}" style="margin:2px"><div>第 {i} 层引用</div>"##
            ));
        }
        for _ in 0..depth {
            html.push_str("</div>");
        }
        let doc = render_pipeline(&html).expect("深嵌套应剪枝成功而非溢出");
        assert!(!doc.blocks.is_empty());
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

    #[test]
    fn pipeline_cid_image_reaches_builder() {
        // T5 评审裁定修复：cid: img src 经 sanitize 精细放行 → ImageRef::Cid
        // 端到端可达（此前被 ammonia 默认 scheme 白名单剥成 Invalid 占位）；
        // 行内 run 形态（线上反馈③第二波：图片留在段落流内）
        use crate::types::{Block, ImageRef, InlineRun};
        let doc = render_pipeline(r#"<img src="cid:Image001@ABC" alt="签章">"#).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("img 应物化为段落（行内 run 形态）")
        };
        let InlineRun::Image {
            src: ImageRef::Cid { content_id },
            alt,
            ..
        } = &runs[0]
        else {
            panic!("cid: 图片应以内嵌附件形态到达树构建")
        };
        assert_eq!(content_id, "image001@abc"); // content_id 小写化
        assert_eq!(alt, "签章");
    }

    #[test]
    fn pipeline_data_image_reaches_builder() {
        use crate::types::{Block, ImageRef, InlineRun};
        let doc = render_pipeline(r#"<img src="data:image/png;base64,iVBORw0KGgo=">"#).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("img 应物化为段落（行内 run 形态）")
        };
        let InlineRun::Image {
            src: ImageRef::DataUri { mime, bytes },
            ..
        } = &runs[0]
        else {
            panic!("data:image 图片应以 DataUri 形态到达树构建")
        };
        assert_eq!(mime, "image/png");
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn pipeline_data_href_still_downgrades_to_text() {
        // 负例：scheme 精细放行仅限 img src——a href="data:" 仍被剥除，
        // 树构建收到无 href 的链接 → 降级纯文字（spec §4.4）
        use crate::types::{Block, InlineRun};
        let doc = render_pipeline(r#"<a href="data:text/html,<b>evil</b>">点我</a>"#).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert!(runs.iter().all(|run| matches!(run, InlineRun::Text { .. })));
        assert!(
            runs.iter()
                .any(|run| matches!(run, InlineRun::Text { text, .. } if text == "点我"))
        );
        let dump = serde_json::to_string(&doc).unwrap();
        assert!(!dump.contains("data:"), "data: 协议不得出现在任何字段");
    }

    #[test]
    fn pipeline_builds_table_with_cell_attributes() {
        // T6：表格在完整管线可达——ammonia 白名单保留 table 一族，td 的
        // colspan/width/style 穿透净化到达树构建（宽度属性为 T6 新放行）
        use crate::types::{Block, Width};
        let doc = render_pipeline(
            r#"<table><tr><td colspan="2" width="300" style="font-size:13px">单元</td></tr></table>"#,
        )
        .unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("表格应构建为 Table")
        };
        assert_eq!(rows.len(), 1);
        let cell = &rows[0].cells[0];
        assert_eq!(cell.colspan, 2);
        assert_eq!(cell.width, Some(Width::Px(300.0)));
        let crate::types::Block::Paragraph { runs, .. } = &cell.blocks[0] else {
            panic!("cell 内容应为 Paragraph")
        };
        let crate::types::InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.font_size, Some(13.0));
    }
}
