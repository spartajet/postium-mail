//! MailView：邮件正文渲染编排层（S1）。
//!
//! 架构（docs/plans/2026-11-16-mailtextview-feasibility.md 实施版）：
//! TextView 作**块流宿主**（保留态 + 块编排 + 无障碍文本管线），全部视觉
//! 与交互由自有块渲染器接管（复用 email_html 渲染内核的富样式/图片门控/
//! 探针——保真度零损失）。数据通道：IR 顶层块 → [`serialize_marker_html`]
//! 索引占位元素 → gpui-base mailview-ext 补丁的 `html_block_parser` 认领为
//! 自定义块（`data: usize` = 块索引）→ `html_block_renderer("mail-block")`
//! 按索引回读 IR 渲染。正文内容不进入标记 HTML（URL/富样式/表格网格均不
//! 暴露给 TextView 内建解析）。
//!
//! 渐进回落：文档含未迁移块类型（S1 为 Table/Quote，S2/S3 迁移）→ 整篇走
//! 旧 [`email_html::render_html_document`] 路径，行为零变。
//!
//! 状态缓存：`TextViewState` 按内容感知键缓存在 DetailStore（标记 HTML 与
//! 深浅通道无关——块内容渲染期经闭包回读当前通道文档），经补丁的
//! `html_with_extensions` 构造，解析器参与首次解析（无二次重解析、无首帧
//! 空白闪）。

use std::sync::Arc;

use gpui_kit::base::{MarkdownNode, TextView, TextViewState};
use gpui_kit::{AnyElement, App, AppContext as _, Entity, IntoElement as _, ParentElement as _, Styled as _};
use markup5ever_rcdom::NodeData;
use postium_html::{serialize_marker_html, Block, HtmlDocument, SafeUrl};

use super::email_html::{self, ImageCtx, ProbeExt as _, QuoteFold};
use super::html_selection::HtmlSelection;

/// S1 支持集递归判定：Table/Quote 暂留旧路径（S2/S3 迁入）。
fn block_supported(block: &Block) -> bool {
    match block {
        Block::Table { .. } | Block::Quote { .. } => false,
        Block::Container { children, .. } => children.iter().all(block_supported),
        Block::List { items, .. } => items.iter().all(|group| group.iter().all(block_supported)),
        _ => true,
    }
}

/// 文档是否可走 MailView 路径（全部块类型已迁移；空文档不值得建宿主）。
pub fn supported(doc: &HtmlDocument) -> bool {
    !doc.blocks.is_empty() && doc.blocks.iter().all(block_supported)
}

/// 标记认领解析器：`<div data-mb="{i}">` → `MarkdownNode { name: "mail-block",
/// data: i }`。纯索引提取（无捕获、无副作用），供
/// `TextViewState::html_with_extensions` 首次解析与元素每帧重注册共用——
/// 两侧必须同形（形状一致才不触发重解析，见补丁 has_same_html_block_parsers）。
pub(crate) fn extract_mail_block(node: &markup5ever_rcdom::Node) -> Option<MarkdownNode> {
    let NodeData::Element { ref name, ref attrs, .. } = node.data else {
        return None;
    };
    let index = attrs
        .borrow()
        .iter()
        .find(|attr| &*attr.name.local == "data-mb")
        .and_then(|attr| attr.value.parse::<usize>().ok())?;
    // name 一致性约束：认领即自定义块，渲染分派按 name 查注册表。
    (name.local.trim() == "div").then(|| MarkdownNode::new("mail-block", index))
}

/// MailView 帧渲染：宿主 TextView + 认领解析器 + 块渲染器接线。
///
/// `state`：DetailStore 缓存的 TextViewState（`html_with_extensions` 构造）。
/// `doc`：当前深/浅通道文档（块渲染闭包按索引回读，通道切换零重建）。
///
/// 每帧重注册的解析器/渲染器闭包与状态内注册形状一致（长度比较不触发重
/// 解析）；渲染闭包捕获本帧 RenderCtx（主题快照 + 三源图片 + 划选 + 折叠
/// 上下文），块渲染语义与旧路径逐块等价。
#[allow(clippy::too_many_arguments)]
pub fn render(
    state: &Entity<TextViewState>,
    doc: &HtmlDocument,
    dark: bool,
    images: ImageCtx,
    selection: HtmlSelection,
    quotes: Option<QuoteFold>,
    on_link: Arc<dyn Fn(&SafeUrl) + Send + Sync + 'static>,
    cx: &mut App,
) -> AnyElement {
    let rcx = email_html::mail_frame_ctx(dark, on_link, images, selection, quotes, cx);
    let blocks = Arc::new(doc.blocks.clone());
    let tv = TextView::new(state)
        .selectable(false)
        .html_block_parser(extract_mail_block)
        .html_block_renderer("mail-block", move |node: &MarkdownNode, _window, _cx| {
            let index = node
                .data::<usize>()
                .copied()
                .unwrap_or(usize::MAX);
            blocks
                .get(index)
                .map(|block| email_html::mail_render_block(block, &rcx))
                .unwrap_or_else(|| gpui_kit::div().into_any_element())
        });
    gpui_kit::div()
        .w_full()
        .when_probe("html-root")
        .child(tv)
        .into_any_element()
}

/// 供 DetailStore 构造缓存态的便捷入口：标记 HTML + 扩展（认领解析器）→
/// TextViewState（补丁 `html_with_extensions`：解析器参与首次解析）。
pub(crate) fn build_text_view_state(
    doc: &HtmlDocument,
    cx: &mut App,
) -> Entity<TextViewState> {
    let marker_html = serialize_marker_html(&doc.blocks);
    let mut extensions = gpui_kit::base::MarkdownExtensions::default();
    extensions.push_html_block_parser(extract_mail_block);
    cx.new(|cx| TextViewState::html_with_extensions(&marker_html, Arc::new(extensions), cx))
}
