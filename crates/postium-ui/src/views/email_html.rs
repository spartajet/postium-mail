//! GPUI 邮件 HTML 渲染层：[`HtmlDocument`] 中间表示 → GPUI 元素。
//!
//! Task 9 上半：块级（Container/Paragraph/Heading/List/Quote/Hr）与行内
//! run 序列（同样式段合并）；表格 flex 模拟与链接点击接线归 Task 10，
//! 图片三源真实渲染归 Task 13（本任务一律占位框）。
//!
//! # 行内 run 选型（实现记录）
//!
//! gpui-base `TextView` 的 TextRun 管线（`TextViewState::parsed_content`
//! 等）为 `pub(super)` 内部实现，无公开的自定义 run 注入口；其高层
//! `html()` 解析器仅支持 `<mark>` 级内联样式，邮件富文本样式会整体丢失
//! （且与本 crate 的 IR 管线语义重复）。故按 spec §5 落到公开 API：
//! 每段行内流聚合为 [`StyledText`]（[`TextRun`] 序列公开可构造）+ div
//! 块级结构。文本划选（`selectable`）依赖 gpui-base TextView 的内部
//! 件制，公开 run 路径无对应能力，留待后续任务沿 TextView 路径补齐。
//!
//! # 颜色与深色模式
//!
//! 渲染层不做颜色反转：dark=true 时调用方传入的是
//! `postium_html::adapt_document` 产出的已适配文档（spec §6），本层
//! 一律按字面映射 IR 颜色；`dark` 仅标记消费口径。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, FontStyle, FontWeight, Hsla, IntoElement as _, ParentElement as _,
    RenderImage, SharedString, StrikethroughStyle, Styled as _, StyledText, TextAlign, TextRun,
    UnderlineStyle, div, font as base_font, px, relative, rgba,
};
use postium_html::{
    Align, Block, BlockStyle, HtmlDocument, ImageStyle, InlineRun, InlineStyle, Rgba, SafeUrl,
    Width,
};

#[cfg(test)]
use gpui_kit::InteractiveElement as _;

/// 正文基础字号（px）。邮件作者未声明字号时的兜底。
const BASE_FONT_SIZE: f32 = 14.;
/// 正文基础行高（倍数）。
const BASE_LINE_HEIGHT: f32 = 1.6;

/// 图片渲染上下文（Task 13 扩展真实加载：Remote 拉取 / Cid 附件 / DataUri 解码）。
#[derive(Default)]
pub struct ImageCtx {
    /// 用户是否放行远程图片（默认拦截，spec §4.4 隐私默认）。
    pub enabled: bool,
    /// 已解析的远程图片缓存（键：URL；值 None 表示加载失败占位）。
    pub resolved: Arc<Mutex<HashMap<String, Option<Arc<RenderImage>>>>>,
    /// 内嵌附件图片（键：content-id 小写）。
    pub attachments: Vec<(String, Arc<RenderImage>)>,
}

/// 渲染期常量：主题衍生默认色板 + 主题字体，一次取齐（帧内不可变）。
struct RenderCtx {
    dark: bool,
    font_family: SharedString,
    foreground: Hsla,
    muted_foreground: Hsla,
    link: Hsla,
    border: Hsla,
    /// 图片/表格占位底色（主题 secondary）。
    placeholder_bg: Hsla,
}

impl RenderCtx {
    fn new(dark: bool, cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            dark,
            font_family: theme.font_family.clone(),
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            link: theme.link,
            border: theme.border,
            placeholder_bg: theme.secondary,
        }
    }

    /// 行内 run 字体：主题字体族 + 粗斜体修饰。
    fn font(&self, bold: bool, italic: bool) -> gpui_kit::Font {
        let mut f = base_font(self.font_family.clone());
        if bold {
            f.weight = FontWeight::BOLD;
        }
        if italic {
            f.style = FontStyle::Italic;
        }
        f
    }
}

/// 块级递归的继承上下文。
#[derive(Clone, Copy, Default)]
struct Inherited {
    /// 块级 `color` 继承（IR BlockStyle.color → 行内默认色）。
    color: Option<Hsla>,
    /// 列表嵌套层级（缩进用）。
    depth: usize,
}

/// 渲染整封 HTML 邮件文档（spec §5 GPUI 渲染层入口）。
///
/// - `dark`：调用方按主题传入；颜色消费的是已适配文档的字面值（见模块注释）。
/// - `images`：图片上下文（Task 13 接管三源渲染）。
/// - `on_link`：链接点击回调（Task 10 接线 `InteractiveText` hitbox）。
pub fn render_html_document(
    doc: &HtmlDocument,
    dark: bool,
    images: ImageCtx,
    on_link: impl Fn(&SafeUrl) + 'static,
    cx: &mut App,
) -> AnyElement {
    // Task 10：链接 run 区间注册 InteractiveText hitbox 并调用本回调；
    // Task 13：按 ImageCtx.enabled / attachments / resolved 渲染三源图片。
    let _ = (images, on_link);

    let rcx = RenderCtx::new(dark, cx);
    render_blocks(&doc.blocks, Inherited::default(), &rcx)
        .when_probe("html-root")
        .into_any_element()
}

/// 块级序列 → 纵向 flex 列。
fn render_blocks(blocks: &[Block], inh: Inherited, rcx: &RenderCtx) -> gpui_kit::Div {
    let mut col = v_flex().w_full();
    for block in blocks {
        col = col.child(render_block(block, inh, rcx));
    }
    col
}

/// 单个块级元素 → GPUI 元素。
fn render_block(block: &Block, inh: Inherited, rcx: &RenderCtx) -> AnyElement {
    match block {
        Block::Container { style, children } => {
            let child_inh = Inherited {
                color: style.color.map(ir_color).or(inh.color),
                depth: inh.depth,
            };
            map_block_style(style, rcx.dark)
                .flex()
                .flex_col()
                .when_some(style.text_align, |d, a| d.text_align(map_align(a)))
                .child(render_blocks(children, child_inh, rcx))
                .into_any_element()
        }
        Block::Paragraph { style, runs, align } => {
            let base_color = style.color.map(ir_color).or(inh.color);
            map_block_style(style, rcx.dark)
                .flex()
                .flex_col()
                .text_align(map_align(*align))
                .children(render_inline_flow(runs, base_color, rcx, false))
                .into_any_element()
        }
        Block::Heading { level, align, runs } => {
            // UA 近似：标题默认字号按级别、默认加粗；作者 CSS 字号已烘焙进
            // runs（T7 裁定①），显式声明经段分组覆盖元素默认。
            map_block_style(&BlockStyle::default(), rcx.dark)
                .flex()
                .flex_col()
                .text_align(map_align(*align))
                .text_size(px(heading_size(*level)))
                .children(render_inline_flow(runs, inh.color, rcx, true))
                .into_any_element()
        }
        Block::List { ordered, items } => render_list(*ordered, items, inh, rcx),
        Block::Quote { children } => v_flex()
            .w_full()
            .border_l(px(3.))
            .border_color(rcx.border)
            .pl(px(12.))
            .child(render_blocks(children, inh, rcx))
            .into_any_element(),
        Block::Table { .. } => {
            // Task 10 接管：Table→v_flex、Row→h_flex、Cell→容器
            // （colspan → flex_grow 权重、cell.width → flex-basis）。
            div()
                .w_full()
                .h(px(24.))
                .rounded_sm()
                .bg(rcx.placeholder_bg)
                .border_1()
                .border_color(rcx.border)
                .into_any_element()
        }
        Block::Image { src, alt, style } => {
            let _ = src; // Task 13：三源（Remote/Cid/DataUri）真实渲染接管占位框
            image_placeholder(style, alt, rcx)
        }
        Block::Hr => div()
            .w_full()
            .h(px(1.))
            .bg(rcx.border)
            .when_probe("html-hr")
            .into_any_element(),
    }
}

/// 列表：标记列（• / 序号）+ 内容列（项内块序列递归）。
fn render_list(ordered: bool, items: &[Vec<Block>], inh: Inherited, rcx: &RenderCtx) -> AnyElement {
    let mut col = v_flex().w_full().pl(px(24. * (inh.depth as f32 + 1.)));
    for (i, item_blocks) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}.", i + 1)
        } else {
            "•".to_string()
        };
        col = col.child(
            h_flex()
                .w_full()
                .items_start()
                .gap_1()
                .child(
                    div()
                        .min_w(px(14.))
                        .text_size(px(BASE_FONT_SIZE))
                        .text_color(rcx.muted_foreground)
                        .child(marker),
                )
                .child(
                    render_blocks(
                        item_blocks,
                        Inherited {
                            color: inh.color,
                            depth: inh.depth + 1,
                        },
                        rcx,
                    )
                    .flex_1(),
                ),
        );
    }
    col.into_any_element()
}

/// 行内流 → 文本元素（字号分组的 [`StyledText`]）与行内图片占位的序列。
///
/// 行内 img 已在树构建期提升为块级（plan Task 5），此处仅为 IR 保留
/// 变体兜底：占位框独立成行。
fn render_inline_flow(
    runs: &[InlineRun],
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    force_bold: bool,
) -> Vec<AnyElement> {
    // 1) 扁平化：同样式文本段合并（含链接内层递归与 br 折行）
    let mut segs: Vec<FlowSeg> = Vec::new();
    flatten_runs(runs, false, force_bold, &mut segs);

    // 2) 连续文本段按字号分组为单个 StyledText（组内相邻同样式 run 再合并）
    let mut els: Vec<AnyElement> = Vec::new();
    let mut group: Vec<FlowSeg> = Vec::new();
    let mut group_size: Option<Option<f32>> = None;
    for seg in segs {
        match seg {
            FlowSeg::Image { style, alt } => {
                flush_text_group(std::mem::take(&mut group), base_color, rcx, &mut els);
                group_size = None;
                els.push(image_placeholder(&style, &alt, rcx));
            }
            text_seg @ FlowSeg::Text { .. } => {
                let size = match &text_seg {
                    FlowSeg::Text { style, .. } => style.font_size,
                    FlowSeg::Image { .. } => None,
                };
                if group_size.is_some() && group_size != Some(size) {
                    flush_text_group(std::mem::take(&mut group), base_color, rcx, &mut els);
                }
                group_size = Some(size);
                group.push(text_seg);
            }
        }
    }
    flush_text_group(group, base_color, rcx, &mut els);
    els
}

/// 行内流中间产物。
enum FlowSeg {
    /// 文本段（同样式段已合并；`link` 标记链接归属，Task 10 供区间注册）。
    Text {
        text: String,
        style: InlineStyle,
        link: bool,
    },
    /// 行内图片占位（Task 13 接管）。
    Image { style: ImageStyle, alt: String },
}

/// 行内 run 序列扁平化：相邻同样式段合并、链接内层加链接标记、br 折行。
fn flatten_runs(runs: &[InlineRun], link: bool, force_bold: bool, out: &mut Vec<FlowSeg>) {
    for run in runs {
        match run {
            InlineRun::Text { text, style } => {
                let mut style = style.clone();
                if force_bold {
                    style.bold = true;
                }
                if text.is_empty() {
                    continue;
                }
                if let Some(FlowSeg::Text {
                    text: last,
                    style: ls,
                    link: ll,
                }) = out.last_mut()
                    && *ll == link
                    && *ls == style
                {
                    last.push_str(text);
                } else {
                    out.push(FlowSeg::Text {
                        text: text.clone(),
                        style,
                        link,
                    });
                }
            }
            InlineRun::Link { runs, .. } => flatten_runs(runs, true, force_bold, out),
            InlineRun::Br => {
                // 折行归并进前段；但不得并入链接段（换行符带下划线会画出线尾）
                let joins_last = matches!(out.last(), Some(FlowSeg::Text { link: false, .. }));
                if joins_last {
                    if let Some(FlowSeg::Text { text, .. }) = out.last_mut() {
                        text.push('\n');
                    }
                } else {
                    out.push(FlowSeg::Text {
                        text: "\n".into(),
                        style: InlineStyle::default(),
                        link: false,
                    });
                }
            }
            InlineRun::Image { src, alt, style } => {
                let _ = src; // Task 13：三源真实渲染
                out.push(FlowSeg::Image {
                    style: style.clone(),
                    alt: alt.clone(),
                });
            }
        }
    }
}

/// 把积累的文本段组渲染为一个文本元素（相邻同样式段合并成单一 [`TextRun`]）。
fn flush_text_group(
    group: Vec<FlowSeg>,
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    els: &mut Vec<AnyElement>,
) {
    // 组文本与 TextRun 序列（相邻同样式 run 合并）
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut size: Option<f32> = None;
    for seg in &group {
        let FlowSeg::Text {
            text: seg_text,
            style,
            link,
        } = seg
        else {
            continue;
        };
        let run = text_run(style, *link, base_color, rcx);
        let seg_len = seg_text.len();
        text.push_str(seg_text);
        if size.is_none() {
            size = style.font_size;
        }
        if let Some(last) = runs.last_mut()
            && *last == run
        {
            last.len += seg_len;
        } else {
            runs.push(TextRun {
                len: seg_len,
                ..run
            });
        }
    }
    if text.is_empty() {
        return;
    }

    let d = div()
        .w_full()
        .text_size(px(size.unwrap_or(BASE_FONT_SIZE)))
        .line_height(relative(BASE_LINE_HEIGHT))
        .child(StyledText::new(text.clone()).with_runs(runs));

    // 文本探针（仅测试构建）：以内容前缀为键记录渲染树，
    // e2e 经 `VisualTestContext::debug_bounds` 查询文本可见性。
    #[cfg(test)]
    let d = {
        let key = text_probe_key(&text);
        d.debug_selector(move || key)
    };
    els.push(d.into_any_element());
}

/// 构建单段 [`TextRun`]（字体/颜色/下划线/删除线；链接段附加链接色与下划线）。
fn text_run(style: &InlineStyle, link: bool, base_color: Option<Hsla>, rcx: &RenderCtx) -> TextRun {
    TextRun {
        len: 0,
        font: rcx.font(style.bold, style.italic),
        color: style
            .color
            .map(ir_color)
            .or(link.then_some(rcx.link))
            .or(base_color)
            .unwrap_or(rcx.foreground),
        background_color: None,
        underline: (style.underline || link).then_some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        strikethrough: style.strikethrough.then_some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        }),
    }
}

/// 图片占位框（Task 13 三源真实渲染接管）：固定高度灰框 + alt 文本，
/// 宽度尽量取 style.width（防加载后布局跳动，spec §5）。
fn image_placeholder(style: &ImageStyle, alt: &str, rcx: &RenderCtx) -> AnyElement {
    let mut d = div()
        .h(px(64.))
        .min_w(px(96.))
        .rounded_sm()
        .border_1()
        .border_color(rcx.border)
        .bg(rcx.placeholder_bg)
        .items_center()
        .justify_center()
        .overflow_hidden();
    match style.width {
        Some(Width::Px(v)) => d = d.w(px(v)),
        Some(Width::Percent(p)) => d = d.w(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    if let Some(Width::Px(v)) = style.height {
        d = d.h(px(v));
    }
    if let Some(Width::Px(v)) = style.max_width {
        d = d.max_w(px(v));
    }
    let d = d.when_probe("html-img");
    if alt.is_empty() {
        return d.into_any_element();
    }
    let label = div()
        .text_size(px(12.))
        .text_color(rcx.muted_foreground)
        .child(alt.to_string());
    #[cfg(test)]
    let label = {
        let key = text_probe_key(alt);
        label.debug_selector(move || key)
    };
    d.child(label).into_any_element()
}

/// [`BlockStyle`] → GPUI div 样式映射（背景/边框/间距/宽度，spec §3 映射表）。
///
/// `dark` 仅标记消费的是已适配文档（`postium_html::adapt` 产出），颜色
/// 一律按字面映射，渲染层不做二次反转（spec §6）。
fn map_block_style(style: &BlockStyle, dark: bool) -> gpui_kit::Div {
    let _ = dark;
    let mut d = div().w_full();
    if let Some(bg) = style.background_color {
        d = d.bg(ir_color(bg));
    }
    // 间距（CSS TRouBLe：上右下左）
    let [mt, mr, mb, ml] = style.margin;
    if mt > 0. {
        d = d.mt(px(mt));
    }
    if mr > 0. {
        d = d.mr(px(mr));
    }
    if mb > 0. {
        d = d.mb(px(mb));
    }
    if ml > 0. {
        d = d.ml(px(ml));
    }
    if style.margin_auto {
        d = d.mx_auto();
    }
    let [pt, pr, pb, pl] = style.padding;
    if pt > 0. {
        d = d.pt(px(pt));
    }
    if pr > 0. {
        d = d.pr(px(pr));
    }
    if pb > 0. {
        d = d.pb(px(pb));
    }
    if pl > 0. {
        d = d.pl(px(pl));
    }
    match style.width {
        Some(Width::Px(v)) => d = d.w(px(v)),
        Some(Width::Percent(p)) => d = d.w(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    match style.max_width {
        Some(Width::Px(v)) => d = d.max_w(px(v)),
        Some(Width::Percent(p)) => d = d.max_w(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    if let Some(Width::Px(v)) = style.height {
        d = d.h(px(v));
    }
    // 单边边框：宽度逐边映射；GPUI 元素单一 border_color，多色异边取首个
    // （保真度注记：邮件高频形态是单侧强调边框，异色四边罕见）
    let mut border_color: Option<Hsla> = None;
    if let Some(b) = style.border_top {
        d = d.border_t(px(b.width));
        border_color.get_or_insert_with(|| ir_color(b.color));
    }
    if let Some(b) = style.border_right {
        d = d.border_r(px(b.width));
        border_color.get_or_insert_with(|| ir_color(b.color));
    }
    if let Some(b) = style.border_bottom {
        d = d.border_b(px(b.width));
        border_color.get_or_insert_with(|| ir_color(b.color));
    }
    if let Some(b) = style.border_left {
        d = d.border_l(px(b.width));
        border_color.get_or_insert_with(|| ir_color(b.color));
    }
    if let Some(c) = border_color {
        d = d.border_color(c);
    }
    d
}

/// IR 颜色（RGBA 0–255）→ GPUI [`Hsla`]。
fn ir_color(c: Rgba) -> Hsla {
    rgba(u32::from_be_bytes(c)).into()
}

/// IR 对齐 → GPUI 文本对齐（Justify 降级 Left：GPUI 暂无两端对齐）。
fn map_align(a: Align) -> TextAlign {
    match a {
        Align::Left | Align::Justify => TextAlign::Left,
        Align::Center => TextAlign::Center,
        Align::Right => TextAlign::Right,
    }
}

/// 标题默认字号（HTML UA 近似：h1 2em → h6 0.67em，基准 16px）。
fn heading_size(level: u8) -> f32 {
    match level {
        1 => 32.,
        2 => 24.,
        3 => 18.7,
        4 => 16.,
        5 => 13.3,
        _ => 10.7,
    }
}

// ---------------------------------------------------------------------------
// 测试探针（仅测试构建生效）：块级锚点与行内文本段以选择器写入渲染树，
// e2e 经 `VisualTestContext::debug_bounds` 查询；正常构建直通零开销。
// ---------------------------------------------------------------------------
trait ProbeExt: Sized {
    fn when_probe(self, selector: &str) -> Self;
}

#[cfg(test)]
impl ProbeExt for gpui_kit::Div {
    fn when_probe(self, selector: &str) -> Self {
        let selector = selector.to_string();
        self.debug_selector(move || selector.clone())
    }
}

#[cfg(not(test))]
impl ProbeExt for gpui_kit::Div {
    fn when_probe(self, _: &str) -> Self {
        self
    }
}

#[cfg(test)]
fn text_probe_key(text: &str) -> String {
    let key: String = text
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(16)
        .collect();
    format!("html-txt-{key}")
}
