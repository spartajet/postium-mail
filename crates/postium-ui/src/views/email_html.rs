//! GPUI 邮件 HTML 渲染层：[`HtmlDocument`] 中间表示 → GPUI 元素。
//!
//! 块级（Container/Paragraph/Heading/List/Quote/Hr/Table）与行内 run 序列
//! （同样式段合并）、表格 flex 模拟与链接 InteractiveText 点击接线；
//! 图片三源真实渲染（Remote 拦截放行 / Cid 附件 / DataUri 直解码，Task 13）。
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

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, CursorStyle, DispatchPhase, Element, ElementId, FontStyle,
    FontWeight, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId,
    InteractiveElement as _, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, RenderImage, SharedString,
    StrikethroughStyle, Styled as _, StyledText, TextAlign, TextRun, UnderlineStyle, div,
    font as base_font, img, px, relative, rgba,
};
use postium_html::{
    Align, Block, BlockStyle, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle, Rgba,
    SafeUrl, TableCell, TableRow, Width,
};

use crate::views::html_selection::HtmlSelection;

/// 正文基础字号（px）。邮件作者未声明字号时的兜底。
const BASE_FONT_SIZE: f32 = 14.;
/// 正文基础行高（倍数）。
const BASE_LINE_HEIGHT: f32 = 1.6;

/// 图片渲染上下文（单次渲染调用的入参快照）。
///
/// 三源消费（Task 13）：`Remote` 查 [`ImageCtx::resolved`]（键为 URL），
/// `Cid` 线性扫 [`ImageCtx::attachments`]，`DataUri` 经
/// [`ImageCtx::data_uris`] 解码缓存（键为字节内容 hash）。
pub struct ImageCtx {
    /// 用户是否放行远程图片（默认拦截，spec §4.4 隐私默认）。
    pub enabled: bool,
    /// 已解析的远程图片缓存（键：URL；值 None 表示加载失败占位）。
    /// 渲染只读；放行后的后台拉取由 email_detail/DetailStore 写入。
    pub resolved: Arc<Mutex<HashMap<String, Option<Arc<RenderImage>>>>>,
    /// 内嵌附件图片（键：content-id 小写）。
    pub attachments: Vec<(String, Arc<RenderImage>)>,
    /// DataUri 解码缓存（键：字节内容 64 位 hash；值 None 表示解码失败也
    /// 记账）。T13 遗留 P2：DataUri 每 GPUI 帧重解码（正文含内嵌图时每帧
    /// 一次 PNG/JPEG 解码）——GPUI 无元素级 memo，渲染层自管。键用内容
    /// hash 而非字节指针：文档可被缓存逐出重建，指针键有地址复用误撞面，
    /// 内容键与缓存键（`content_key`）同口径。随 [`HtmlRenderState`] 按邮件
    /// 维度生灭（切换邮件即重置，空间有界）。
    pub data_uris: Arc<Mutex<HashMap<u64, Option<Arc<RenderImage>>>>>,
}

impl Default for ImageCtx {
    fn default() -> Self {
        Self {
            enabled: false,
            resolved: Arc::new(Mutex::new(HashMap::new())),
            attachments: Vec::new(),
            data_uris: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// 详情侧按邮件维度持有的图片渲染状态（email_detail/DetailStore 持有，
/// 切换邮件即重置——放行态的持久记忆由 DetailStore 装载回调按发件人白名单
/// 恢复，见 detail.rs 线上反馈①）。
///
/// 与 [`ImageCtx`] 的分工：本结构是状态（随邮件装载生灭、被后台拉取
/// 任务写入），[`ImageCtx`] 是单帧渲染快照（[`HtmlRenderState::to_ctx`]
/// 克隆派生，渲染树只读消费）。
#[derive(Default)]
pub struct HtmlRenderState {
    /// 远程图片是否已放行（「加载图片」按钮翻转）。
    pub enabled: bool,
    /// 已解析的远程图片缓存（键：URL；后台拉取任务写回）。
    pub resolved: Arc<Mutex<HashMap<String, Option<Arc<RenderImage>>>>>,
    /// 内嵌附件图片（键：content-id 小写，装载期解码一次）。
    pub attachments: Vec<(String, Arc<RenderImage>)>,
    /// DataUri 解码缓存（键：字节内容 hash，[`ImageCtx::data_uris`] 同口径）。
    pub data_uris: Arc<Mutex<HashMap<u64, Option<Arc<RenderImage>>>>>,
}

impl HtmlRenderState {
    /// 派生单帧渲染快照（Arc 共享 resolved/data_uris、克隆附件列表——每帧廉价）。
    pub fn to_ctx(&self) -> ImageCtx {
        ImageCtx {
            enabled: self.enabled,
            resolved: Arc::clone(&self.resolved),
            attachments: self.attachments.clone(),
            data_uris: Arc::clone(&self.data_uris),
        }
    }
}

/// 渲染期常量：主题衍生默认色板 + 主题字体 + 链接回调，一次取齐（帧内不可变）。
struct RenderCtx {
    dark: bool,
    font_family: SharedString,
    foreground: Hsla,
    muted_foreground: Hsla,
    link: Hsla,
    border: Hsla,
    /// 图片/表格占位底色（主题 secondary）。
    placeholder_bg: Hsla,
    /// 划选高亮底色（主题 selection，线上反馈①：正文可选中）。
    selection_bg: Hsla,
    /// 链接点击回调（SelectableText hitbox → 调用方打开动作；email_detail
    /// 侧为 open::that_detached 系统处理，见其 open_safe_url）。
    on_link: Arc<dyn Fn(&SafeUrl) + 'static>,
    /// 图片三源上下文（Remote 查 resolved / Cid 扫 attachments / DataUri 解码）。
    images: ImageCtx,
    /// 文本划选状态（组注册表 + 锚点/游标，线上反馈①）。
    selection: HtmlSelection,
}

impl RenderCtx {
    fn new(
        dark: bool,
        on_link: Arc<dyn Fn(&SafeUrl) + 'static>,
        images: ImageCtx,
        selection: HtmlSelection,
        cx: &App,
    ) -> Self {
        let theme = cx.theme();
        Self {
            dark,
            font_family: theme.font_family.clone(),
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            link: theme.link,
            border: theme.border,
            placeholder_bg: theme.secondary,
            selection_bg: theme.selection,
            on_link,
            images,
            selection,
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
/// - `images`：图片三源上下文（Remote/Cid/DataUri，Task 13）。
/// - `selection`：文本划选状态（线上反馈①——每帧重建组注册表，锚点/游标
///   由 SelectableText 的鼠标事件维护）。
/// - `on_link`：链接点击回调（SelectableText hitbox 命中链接区间时以
///   [`SafeUrl`] 调用；email_detail 侧接线为 `open::that_detached`——
///   Http/https 系统浏览器、mailto 系统邮件客户端）。
pub fn render_html_document(
    doc: &HtmlDocument,
    dark: bool,
    images: ImageCtx,
    selection: HtmlSelection,
    on_link: impl Fn(&SafeUrl) + 'static,
    cx: &mut App,
) -> AnyElement {
    selection.begin_frame();
    let rcx = RenderCtx::new(dark, Arc::new(on_link), images, selection, cx);
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
                .children(render_inline_flow(
                    runs,
                    base_color,
                    rcx,
                    false,
                    BASE_FONT_SIZE,
                    *align,
                ))
                .into_any_element()
        }
        Block::Heading { level, align, runs } => {
            // UA 近似：标题默认字号/加粗为渲染层默认，按级别参数化传入文本
            // 元素（文本 div 的显式 text_size 会覆盖父级级联，故不走包装层）；
            // 作者 CSS 字号已烘焙进 runs（T7 裁定①），显式声明经段分组覆盖
            // 该默认。
            // 边缘（IR 限制，渲染层无修正点）：`<h1 style="font-weight:normal">`
            // 的显式 normal 与未声明在 InlineStyle.bold: bool 上不可区分
            // （builder parse_font_weight: normal → false，且 merge_inline 为
            // `base.bold || over.bold` OR 语义），TextRun.font.weight 必填无
            // "未设置"态——UA 粗体默认仍会覆盖显式 normal，需 IR 三态扩展
            // （牵动 T7 裁定①与 T8 快照基线）方可修复，见 task-9 报告。
            // T14 终审裁定：维持现状，不做 IR 三态化（收益仅 font-weight:
            // normal 显式声明的极边缘形态，不值快照基线重置）。
            map_block_style(&BlockStyle::default(), rcx.dark)
                .flex()
                .flex_col()
                .text_align(map_align(*align))
                .children(render_inline_flow(
                    runs,
                    inh.color,
                    rcx,
                    true,
                    heading_size(*level),
                    *align,
                ))
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
        Block::Table { style, rows } => {
            let child_inh = Inherited {
                color: style.color.map(ir_color).or(inh.color),
                depth: inh.depth,
            };
            // 表格 flex 模拟（spec §5）：Table→v_flex，行→h_flex。
            map_block_style(style, rcx.dark)
                .flex()
                .flex_col()
                .when_some(style.text_align, |d, a| d.text_align(map_align(a)))
                .children(rows.iter().map(|row| render_table_row(row, child_inh, rcx)))
                .into_any_element()
        }
        Block::Image { src, alt, style } => render_image_ref(src, alt, style, rcx),
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

/// 表格行 → 横向 flex（cells 沿主轴排布，交叉轴拉伸等高——UA 行高 = 最高单元格）。
fn render_table_row(row: &TableRow, inh: Inherited, rcx: &RenderCtx) -> gpui_kit::Div {
    let mut r = h_flex().w_full().items_stretch();
    for cell in &row.cells {
        r = r.child(render_table_cell(cell, inh, rcx));
    }
    r
}

/// 表格单元格 → 容器（colspan → flex_grow 权重、width → 固定宽/百分比宽、
/// 无显式宽 basis 归零交 grow 相位分配、cell.style → map_block_style；
/// 嵌套块递归 [`render_blocks`]，空单元格占最小行高防塌陷）。
///
/// 内容垂直居中（线上反馈②）：浏览器 td 默认 `valign=middle`，单元格内容
/// 在行高内垂直居中——flex_col 主轴 justify_center 对齐该语义。
///
/// `min_w(0)`（线上反馈②第二波）：taffy 对 flex 子项有 `min-width:auto`
/// （= min-content）钳制——每行按"本行内容最小宽"起算列宽，行间内容不同
/// 则竖线漂移错位。解除钳制后列宽完全由构建期统一下发的 width/basis 决定，
/// 行间共线（窄列内容自然折行，与浏览器一致）。
fn render_table_cell(cell: &TableCell, inh: Inherited, rcx: &RenderCtx) -> gpui_kit::Div {
    let child_inh = Inherited {
        color: cell.style.color.map(ir_color).or(inh.color),
        depth: inh.depth,
    };
    // colspan 权重 = colspan 值（IR 保证 ≥1；max(1.) 防 0 权重塌陷）。
    // 无显式宽的单元格 flex_basis 归零（评审修复）：map_block_style 的
    // w_full 会把每格 basis 顶到行宽 100%，Σbasis 溢行使 taffy 恒走 shrink
    // 相位（按 basis 比例收缩）且自由空间归零——grow 相位从不被咨询，
    // colspan 权重失效（[1,2] 行渲染成 50/50）。basis 归零后 shrink 无从
    // 发生，grow 相位按权重接管分配。有显式 width 的格保持 width→
    // flex-basis（声明宽 + 自动扩展）；列宽的跨行协调由 builder 网格
    // 归一化统一下发到各行同列（逐行独立 flex 下对齐竖线的前提）。
    let mut c = map_block_style(&cell.style, rcx.dark)
        .flex()
        .flex_col()
        .justify_center()
        .min_w(px(0.))
        .flex_grow(f32::from(cell.colspan).max(1.))
        .when_some(cell.style.text_align, |d, a| d.text_align(map_align(a)));
    match cell.width {
        Some(Width::Px(v)) => c = c.w(px(v)),
        Some(Width::Percent(p)) => c = c.w(relative((p / 100.).clamp(0., 1.))),
        None => c = c.flex_basis(px(0.)),
    }
    if cell.blocks.is_empty() {
        c = c.min_h(px(BASE_FONT_SIZE * BASE_LINE_HEIGHT));
    } else {
        c = c.child(render_blocks(&cell.blocks, child_inh, rcx));
    }
    c
}

/// 行内流 → 文本元素序列。两种形态：
///
/// - **纯文本流**（无图片）：按字号分组的 [`StyledText`] 逐组纵排（既有行为，
///   单组段落视觉不变）；
/// - **图文混排**（线上反馈③第二波）：单一带换行的横向 flex（wrap-flow），
///   文本组与图片按序横排、行满换行——浏览器行内流的近似。此前行内图片
///   在构建期被提升为块级，把「链接 + 分隔小图」混排的横向菜单切成逐项
///   竖排（分隔图独占整行高）；现在图片留在段落 runs 内，此处横向消费。
///
/// `default_size`：run 未声明字号时的兜底字号——由调用方按块类型传入
/// （正文 [`BASE_FONT_SIZE`]、标题按级别 [`heading_size`]），使元素级
/// 字号与包装层语义一致（文本 div 的显式 text_size 覆盖父级级联，
/// 包装层 text_size 不生效，故不走级联）。`align` 供 wrap-flow 的
/// 行内水平对齐（justify）消费。
fn render_inline_flow(
    runs: &[InlineRun],
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    force_bold: bool,
    default_size: f32,
    align: Align,
) -> Vec<AnyElement> {
    // 1) 扁平化：同样式文本段合并（含链接内层递归与 br 折行）
    let mut segs: Vec<FlowSeg> = Vec::new();
    flatten_runs(runs, None, force_bold, &mut segs);
    let has_image = segs.iter().any(|s| matches!(s, FlowSeg::Image { .. }));
    // 混字号判定：字号异组分段即多组——组走 inline 形态并入 wrap-flow
    //（线上反馈③第三波：导航链接 12px 与链接间空白默认字号异组，逐组块级
    //  纵排把每个链接独占一行）
    let has_mixed_sizes = {
        // f32 无 Hash——以 Vec 判重
        let mut distinct: Vec<Option<f32>> = Vec::new();
        for seg in &segs {
            if let FlowSeg::Text { style, .. } = seg
                && !distinct.contains(&style.font_size)
            {
                distinct.push(style.font_size);
            }
        }
        distinct.len() > 1
    };
    let wrap_flow = has_image || has_mixed_sizes;

    // 2) 连续文本段按字号分组为单个 StyledText（组内相邻同样式 run 再合并）；
    //    wrap-flow 时组走 inline 形态（max_w_full 而非占满整行），横向共容器
    let mut els: Vec<AnyElement> = Vec::new();
    let mut group: Vec<FlowSeg> = Vec::new();
    let mut group_size: Option<Option<f32>> = None;
    let flush = |group: &mut Vec<FlowSeg>, els: &mut Vec<AnyElement>| {
        flush_text_group(
            std::mem::take(group),
            base_color,
            rcx,
            default_size,
            wrap_flow,
            els,
        );
    };
    for seg in segs {
        match seg {
            FlowSeg::Image { src, style, alt } => {
                flush(&mut group, &mut els);
                group_size = None;
                els.push(render_image_ref(&src, &alt, &style, rcx));
            }
            text_seg @ FlowSeg::Text { .. } => {
                let size = match &text_seg {
                    FlowSeg::Text { style, .. } => style.font_size,
                    FlowSeg::Image { .. } => None,
                };
                if group_size.is_some() && group_size != Some(size) {
                    flush(&mut group, &mut els);
                }
                group_size = Some(size);
                group.push(text_seg);
            }
        }
    }
    flush(&mut group, &mut els);

    // 3) wrap-flow 流（含图片或混字号）并入单一 wrap 容器（横排 + 行满换行）；
    //    单一字号组的纯文本流保持既有块级形态（视觉等价，零回归；
    //    T9 备案"异字号不共段折行"在此闭环）
    if !wrap_flow {
        return els;
    }
    let mut flow = h_flex().flex_wrap().items_baseline().w_full();
    match align {
        Align::Center => flow = flow.justify_center(),
        Align::Right => flow = flow.justify_end(),
        Align::Left | Align::Justify => {}
    }
    for el in els {
        flow = flow.child(el);
    }
    vec![flow.into_any_element()]
}

/// 行内流中间产物。
enum FlowSeg {
    /// 文本段（同样式段已合并；`link` 携带归属链接的 [`SafeUrl`]，Task 10
    /// 供 InteractiveText 区间注册——不同 href 的相邻段不合并）。
    Text {
        text: String,
        style: InlineStyle,
        link: Option<SafeUrl>,
    },
    /// 行内图片（Task 13 三源真实渲染；IR 保留变体兜底——行内 img 已在
    /// 树构建期提升为块级）。
    Image {
        src: ImageRef,
        style: ImageStyle,
        alt: String,
    },
}

/// 行内 run 序列扁平化：相邻同样式段合并、链接内层携带 href、br 折行。
fn flatten_runs(
    runs: &[InlineRun],
    link: Option<&SafeUrl>,
    force_bold: bool,
    out: &mut Vec<FlowSeg>,
) {
    for run in runs {
        match run {
            InlineRun::Text { text, style } => {
                let mut style = style.clone();
                // UA 粗体默认（标题）。边缘：`<h1 style="font-weight:normal">`
                // 的显式 normal 与未声明在 InlineStyle.bold: bool 上不可区分
                // （builder parse_font_weight: normal → false；merge_inline 为
                // OR 语义），TextRun.font.weight 必填无"未设置"态——渲染层无
                // 修正点，需 IR 三态扩展方可落实"显式声明优先"（见 Heading
                // 分支注释与 task-9 报告）。T14 终审裁定：维持现状，不做
                // IR 三态化。
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
                    && ll.as_ref() == link
                    && *ls == style
                {
                    last.push_str(text);
                } else {
                    out.push(FlowSeg::Text {
                        text: text.clone(),
                        style,
                        link: link.cloned(),
                    });
                }
            }
            InlineRun::Link { runs, href } => flatten_runs(runs, Some(href), force_bold, out),
            InlineRun::Br => {
                // 折行归并进前段；但不得并入链接段（换行符带下划线会画出线尾）
                let joins_last = matches!(out.last(), Some(FlowSeg::Text { link: None, .. }));
                if joins_last {
                    if let Some(FlowSeg::Text { text, .. }) = out.last_mut() {
                        text.push('\n');
                    }
                } else {
                    out.push(FlowSeg::Text {
                        text: "\n".into(),
                        style: InlineStyle::default(),
                        link: None,
                    });
                }
            }
            InlineRun::Image { src, alt, style } => {
                out.push(FlowSeg::Image {
                    src: src.clone(),
                    style: style.clone(),
                    alt: alt.clone(),
                });
            }
        }
    }
}

/// 把积累的文本段组渲染为一个文本元素（相邻同样式段合并成单一 [`TextRun`]）。
///
/// `inline`（wrap-flow 内的文本组）：宽度取内容自适应（max_w_full 防单组
/// 溢出行宽，组内文本在钳制宽内自然折行），与图片同线横排；块级形态
/// （纯文本流）保持占满整行。
///
/// 文本以 [`SelectableText`] 承载（线上反馈①）：链接段按 seg 级标记 +
/// 段长累加为字节区间（`TextRun.len` / `index_for_position` 均以字节计）
/// 注册 clickable range，按下/抬起同区间命中即以段归属的 [`SafeUrl`] 调用
/// `on_link`；同时向 [`HtmlSelection`] 上报按下/拖动/抬起，选中区间在
/// run 上切分并施加高亮背景。
#[allow(clippy::too_many_arguments)]
fn flush_text_group(
    group: Vec<FlowSeg>,
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    default_size: f32,
    inline: bool,
    els: &mut Vec<AnyElement>,
) {
    // 组文本、TextRun 序列（相邻同样式 run 合并）与链接字节区间
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut links: Vec<(Range<usize>, SafeUrl)> = Vec::new();
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
        let run = text_run(style, link.is_some(), base_color, rcx);
        let seg_len = seg_text.len();
        let seg_start = text.len();
        text.push_str(seg_text);
        if size.is_none() {
            size = style.font_size;
        }
        match runs.last_mut() {
            Some(last) if same_run_style(last, &run) => last.len += seg_len,
            _ => runs.push(TextRun {
                len: seg_len,
                ..run
            }),
        }
        if let Some(url) = link {
            links.push((seg_start..text.len(), url.clone()));
        }
    }
    if text.is_empty() {
        return;
    }

    // 划选（线上反馈①）：注册组序号，选中字节区间切 run 施加高亮背景
    let group_ix = rcx.selection.register_group(&text);
    if let Some(range) = rcx.selection.range_for_group(group_ix) {
        runs = apply_selection_background(runs, range, rcx.selection_bg);
    }

    let d = if inline {
        // wrap-flow 内的文本组：宽度自适应内容、上钳行宽（组内折行），
        // 不占满整行——与图片同线横排的前提
        div().max_w_full()
    } else {
        div().w_full()
    }
    .text_size(px(size.unwrap_or(default_size)))
    .line_height(relative(BASE_LINE_HEIGHT));

    let d = d.child(SelectableText {
        id: ElementId::named_usize("html-sel", group_ix),
        text: StyledText::new(text).with_runs(runs),
        group: group_ix,
        selection: rcx.selection.clone(),
        ranges: links.iter().map(|(r, _)| r.clone()).collect(),
        urls: links.into_iter().map(|(_, u)| u).collect(),
        on_link: Arc::clone(&rcx.on_link),
    });

    // 文本探针（仅测试构建）：以内容前缀为键记录渲染树，
    // e2e 经 `VisualTestContext::debug_bounds` 查询文本可见性。
    #[cfg(test)]
    let d = {
        let key = text_probe_key(&{
            // 探针键沿用组全文（含划选无关），从注册表取回避免再拼
            let mut probe = String::new();
            for seg in &group {
                if let FlowSeg::Text { text, .. } = seg {
                    probe.push_str(text);
                }
            }
            probe
        });
        d.debug_selector(move || key)
    };
    els.push(d.into_any_element());
}

/// 选中区间切分 [`TextRun`]：交叠 run 拆为 前/选中/后 三段，选中段施加
/// 高亮背景（字节偏移来自 index_for_position，必在字符边界；区间边界
/// 落在 run 边界时相应段为空自动省略）。
fn apply_selection_background(runs: Vec<TextRun>, sel: Range<usize>, bg: Hsla) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut cursor = 0usize;
    for run in runs {
        let start = cursor;
        let end = cursor + run.len;
        cursor = end;
        let lo = start.max(sel.start);
        let hi = end.min(sel.end);
        if hi > lo {
            if lo > start {
                out.push(TextRun {
                    len: lo - start,
                    ..run.clone()
                });
            }
            out.push(TextRun {
                len: hi - lo,
                background_color: Some(bg),
                ..run.clone()
            });
            if end > hi {
                out.push(TextRun {
                    len: end - hi,
                    ..run
                });
            }
        } else {
            out.push(run);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// SelectableText：可划选 + 可点击链接的文本元素（线上反馈①）
//
// 镜像 gpui InteractiveText 的元素管线（StyledText 委托 + hitbox +
// window.on_mouse_event），在其上叠加：
// - 划选：按下设锚点、拖动移游标（仅拖选态）、抬起定格；选中区间由
//   flush_text_group 在 run 上施加高亮背景（重渲染经 window.refresh）；
// - 链接：按下/抬起同落一个 clickable 区间才触发（不与拖选冲突——
//   拖选产生的位移会使 down/up 落入不同字节区间，天然不触发）。
// ---------------------------------------------------------------------------

/// 可划选文本元素（元素态：链接点击的按下字节位，跨帧保持）。
struct SelectableText {
    id: ElementId,
    text: StyledText,
    group: usize,
    selection: HtmlSelection,
    ranges: Vec<Range<usize>>,
    urls: Vec<SafeUrl>,
    on_link: Arc<dyn Fn(&SafeUrl) + 'static>,
}

#[derive(Default)]
struct SelectableTextState {
    down_index: Rc<Cell<Option<usize>>>,
}

impl Element for SelectableText {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut gpui_kit::Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: gpui_kit::Bounds<gpui_kit::Pixels>,
        _state: &mut (),
        window: &mut gpui_kit::Window,
        cx: &mut App,
    ) -> Hitbox {
        self.text
            .prepaint(None, inspector_id, bounds, &mut (), window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: gpui_kit::Bounds<gpui_kit::Pixels>,
        _: &mut (),
        hitbox: &mut Hitbox,
        window: &mut gpui_kit::Window,
        cx: &mut App,
    ) {
        let text_layout = self.text.layout().clone();
        let group = self.group;
        let selection = self.selection.clone();
        let ranges = std::mem::take(&mut self.ranges);
        let urls = std::mem::take(&mut self.urls);
        let on_link = Arc::clone(&self.on_link);

        window.with_element_state::<SelectableTextState, _>(
            global_id.unwrap(),
            |state, window| {
                let down_index = state.unwrap_or_default().down_index;

                // 光标：链接上手指、正文文本 I-beam
                if hitbox.is_hovered(window)
                    && let Ok(ix) = text_layout.index_for_position(window.mouse_position())
                {
                    let style = if ranges.iter().any(|r| r.contains(&ix)) {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::IBeam
                    };
                    window.set_cursor_style(style, hitbox);
                }

                {
                    let hitbox = hitbox.clone();
                    let layout = text_layout.clone();
                    let selection = selection.clone();
                    let down_index = Rc::clone(&down_index);
                    window.on_mouse_event(
                        move |event: &MouseDownEvent, phase, window: &mut gpui_kit::Window, _| {
                            if phase == DispatchPhase::Bubble
                                && event.button == MouseButton::Left
                                && hitbox.is_hovered(window)
                                && let Ok(ix) = layout.index_for_position(event.position)
                            {
                                down_index.set(Some(ix));
                                selection.on_down(group, ix);
                                window.refresh();
                            }
                        },
                    );
                }
                {
                    let hitbox = hitbox.clone();
                    let layout = text_layout.clone();
                    let selection = selection.clone();
                    window.on_mouse_event(
                        move |event: &MouseMoveEvent, phase, window: &mut gpui_kit::Window, _| {
                            if phase == DispatchPhase::Bubble
                                && selection.is_dragging()
                                && hitbox.is_hovered(window)
                                && let Ok(ix) = layout.index_for_position(event.position)
                            {
                                selection.on_move(group, ix);
                                window.refresh();
                            }
                        },
                    );
                }
                {
                    let hitbox = hitbox.clone();
                    let layout = text_layout.clone();
                    let selection = selection.clone();
                    let down_index = Rc::clone(&down_index);
                    window.on_mouse_event(
                        move |event: &MouseUpEvent, phase, window: &mut gpui_kit::Window, _| {
                            if phase != DispatchPhase::Bubble
                                || event.button != MouseButton::Left
                            {
                                return;
                            }
                            if hitbox.is_hovered(window)
                                && let Ok(up_ix) = layout.index_for_position(event.position)
                                && let Some(down) = down_index.get()
                                && let Some(range_ix) = ranges
                                    .iter()
                                    .position(|r| r.contains(&down) && r.contains(&up_ix))
                                && let Some(url) = urls.get(range_ix)
                            {
                                on_link(url);
                            }
                            down_index.take();
                            selection.on_up();
                            window.refresh();
                        },
                    );
                }

                self.text
                    .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
                ((), SelectableTextState { down_index })
            },
        );
    }
}

impl IntoElement for SelectableText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

/// [`TextRun`] 同样式判据（比较样式字段、不含 `len`——len 是入列时回填的
/// 字节数，参与比较会使合并判据恒假；`with_runs` 要求 run 恰好覆盖全文，
/// 相邻同样式 run 合并保证 run 序列最简）。
fn same_run_style(a: &TextRun, b: &TextRun) -> bool {
    a.font == b.font
        && a.color == b.color
        && a.background_color == b.background_color
        && a.underline == b.underline
        && a.strikethrough == b.strikethrough
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

/// 图片三源分发（Task 13，spec §4.4 / §5）：
///
/// - `DataUri`：字节已在 [`ImageRef`] 中，经 [`ImageCtx::data_uris`] 解码缓存
///   显示（每帧命中缓存零重解码，T13 遗留 P2；解码失败同样记账 → 后续帧
///   直接占位）；缓存未命中时解码并写入。
/// - `Cid`：按 content-id 在 [`ImageCtx::attachments`] 中匹配（装载期
///   已由 DetailStore 解码）；未匹配 → 占位框。
/// - `Remote`：放行且已拉取成功 → 真图；未放行 / 加载中 / 加载失败 → 占位框。
/// - `Invalid`：来源非法（协议不在白名单）→ 占位框。
fn render_image_ref(src: &ImageRef, alt: &str, style: &ImageStyle, rcx: &RenderCtx) -> AnyElement {
    let resolved: Option<Arc<RenderImage>> = match src {
        ImageRef::DataUri { bytes, .. } => decode_render_image_cached(bytes, &rcx.images.data_uris),
        ImageRef::Cid { content_id } => rcx
            .images
            .attachments
            .iter()
            .find(|(cid, _)| cid == content_id)
            .map(|(_, img)| Arc::clone(img)),
        ImageRef::Remote { url } if rcx.images.enabled => rcx
            .images
            .resolved
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(url)
            .cloned()
            .flatten(),
        ImageRef::Remote { .. } | ImageRef::Invalid => None,
    };
    match resolved {
        Some(img) => render_loaded_image(&img, style, rcx),
        None => image_placeholder(style, alt, rcx),
    }
}

/// 已解析图片 → GPUI [`img`] 元素（宽度取 style.width、高度取 style.height、
/// max_width 钳制，与占位框同口径防布局跳动，spec §5）。
fn render_loaded_image(
    image: &Arc<RenderImage>,
    style: &ImageStyle,
    rcx: &RenderCtx,
) -> AnyElement {
    let _ = rcx;
    // 尺寸样式直接落在 Img（Styled）上：无显式宽时保持图片固有尺寸。
    let mut el = img(image.clone()).overflow_hidden();
    match style.width {
        Some(Width::Px(v)) => el = el.w(px(v)),
        Some(Width::Percent(p)) => el = el.w(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    match style.height {
        Some(Width::Px(v)) => el = el.h(px(v)),
        Some(Width::Percent(p)) => el = el.h(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    match style.max_width {
        Some(Width::Px(v)) => el = el.max_w(px(v)),
        Some(Width::Percent(p)) => el = el.max_w(relative((p / 100.).clamp(0., 1.))),
        None => {}
    }
    // 已加载真图探针：e2e 断言「无真图渲染」的负向键（占位框为 html-img）。
    el.debug_selector(|| "html-img-loaded".to_string())
        .into_any_element()
}

/// 图片占位框（未放行/加载中/加载失败/Cid 未匹配/Invalid/DataUri 解码失败）：
/// 固定高度灰框 + alt 文本，宽度尽量取 style.width（防加载后布局跳动，spec §5）。
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

/// 图片字节 → GPUI [`RenderImage`]（Task 13 三源解码入口）。
///
/// 白名单格式 png/jpeg/gif/webp（与 Cargo feature 口径一致）；解码为
/// RGBA 后交换 R/B 成 BGRA（GPUI [`RenderImage`] 帧缓冲为 BGRA 字节序，
/// gpui-pre `decode_static_image` 同款处理），帧缓冲单帧——GIF/WebP 动图
/// 取首帧（v1 取舍，见 task 报告）。解码失败返回 None（调用方以占位框兜底）。
pub fn decode_render_image(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    use image::ImageFormat;

    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    // 格式白名单：未知格式（SVG 等非 raster）与白名单外格式一律拒绝
    if !matches!(
        reader.format()?,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::WebP
    ) {
        return None;
    }
    // 像素尺寸上限（终审裁定）：10MB 下载上限不约束解压后尺寸（PNG 炸弹
    // 可膨胀至数十 GB 分配），解码前按头部声明尺寸拒绝（into_dimensions
    // 消耗 reader，尺寸通过后按已知格式重建）
    const MAX_PIXELS: u64 = 64 * 1024 * 1024; // 64MP ≈ 8K×8K
    let format = reader.format()?;
    let (w, h) = reader.into_dimensions().ok()?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return None;
    }
    let decoded = image::ImageReader::with_format(std::io::Cursor::new(bytes), format)
        .decode()
        .ok()?;
    let mut raw = decoded.to_rgba8().into_raw();
    for [r, _, b, _] in raw.as_chunks_mut::<4>().0 {
        std::mem::swap(r, b); // RGBA → BGRA
    }
    let buffer = image::RgbaImage::from_raw(decoded.width(), decoded.height(), raw)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

/// DataUri 解码缓存键：字节内容 64 位 hash（与 DetailStore 的 `content_key`
/// 同为 std DefaultHasher 口径；内容键无指针地址复用误撞面，见
/// [`ImageCtx::data_uris`] 注记）。
fn data_uri_key(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// 带缓存的 DataUri 解码（渲染层每帧调用，命中零解码）。
///
/// 成功与失败（None）都入账：畸形 DataUri 的解码失败也只发生一次，后续帧
/// 直接从缓存取占位结论。锁毒化走 `into_inner` 兜底（项目既有口径）。
fn decode_render_image_cached(
    bytes: &[u8],
    cache: &Mutex<HashMap<u64, Option<Arc<RenderImage>>>>,
) -> Option<Arc<RenderImage>> {
    let key = data_uri_key(bytes);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = cache.get(&key) {
        return hit.clone();
    }
    let decoded = decode_render_image(bytes);
    cache.insert(key, decoded.clone());
    decoded
}

/// 收集文档中的远程图片 URL（去重、保序）——「加载图片」放行后逐图拉取
/// 的清单，以及正文顶部条的存在性判据（非空 ⇔ 存在 Remote 图）。
///
/// 块级与行内图片都走查（行内 img 常规已提升为块级，此处覆盖 IR 保留变体）；
/// 嵌套容器/引用块/表格单元格/列表项递归。
pub fn collect_remote_image_urls(doc: &HtmlDocument) -> Vec<String> {
    fn walk_blocks(
        blocks: &[Block],
        out: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        for block in blocks {
            match block {
                Block::Image { src, .. } => push_remote(src, out, seen),
                Block::Container { children, .. } | Block::Quote { children } => {
                    walk_blocks(children, out, seen)
                }
                Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => {
                    walk_runs(runs, out, seen)
                }
                Block::List { items, .. } => {
                    for item in items {
                        walk_blocks(item, out, seen);
                    }
                }
                Block::Table { rows, .. } => {
                    for row in rows {
                        for cell in &row.cells {
                            walk_blocks(&cell.blocks, out, seen);
                        }
                    }
                }
                Block::Hr => {}
            }
        }
    }
    fn walk_runs(
        runs: &[InlineRun],
        out: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        for run in runs {
            match run {
                InlineRun::Image { src, .. } => push_remote(src, out, seen),
                InlineRun::Link { runs, .. } => walk_runs(runs, out, seen),
                InlineRun::Text { .. } | InlineRun::Br => {}
            }
        }
    }
    fn push_remote(
        src: &ImageRef,
        out: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        if let ImageRef::Remote { url } = src
            && seen.insert(url.clone())
        {
            out.push(url.clone());
        }
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    walk_blocks(&doc.blocks, &mut out, &mut seen);
    out
}

/// 归一化 content-id 匹配键（CID 附件侧 → [`ImageRef::Cid`] 侧口径）：
/// 去尖括号 + 小写。HTML 侧由 builder 完成 trim + 小写（`cid:` 后原文），
/// DB 侧 content_id 可能带 `<…>` 或大小写不一，统一到小写无括号形态。
pub fn normalize_content_id(cid: &str) -> String {
    cid.trim().trim_matches(['<', '>']).to_ascii_lowercase()
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
    if key.is_empty() {
        // 纯空白/符号文本（br 独立成组、符号分隔线等）：字母数字前缀为空，
        // 多组会碰撞出同键 → 内容 hash 兜底保证键唯一（T9 交接）。
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut hasher);
        format!("html-txt-sym-{:016x}", hasher.finish())
    } else {
        format!("html-txt-{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain_run(len: usize) -> TextRun {
        TextRun {
            len,
            font: base_font(SharedString::default()),
            color: rgba(0x000000FF).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    /// 划选 run 切分（线上反馈①）：交叠 run 拆前/选中/后三段、选中段带
    /// 高亮背景；区间边界对齐 run 边界时不产空段；无交叠原样返回。
    #[test]
    fn selection_background_splits_runs() {
        let bg: Hsla = rgba(0x3344CC55).into();
        let mk_runs = || vec![plain_run(6), plain_run(6)];
        // 选中横跨两 run 中段（3..9）：按 run 各自拆前/选中/后
        //（不同 run 样式可能不同，选中片段不跨 run 合并）
        let split = apply_selection_background(mk_runs(), 3..9, bg);
        assert_eq!(
            split.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![3, 3, 3, 3],
            "run1: 前 3 + 选中 3；run2: 选中 3 + 后 3"
        );
        assert_eq!(
            split
                .iter()
                .map(|r| r.background_color)
                .collect::<Vec<_>>(),
            vec![None, Some(bg), Some(bg), None],
            "仅选中片段带背景"
        );
        // 区间与 run 边界对齐：不产空段
        let split = apply_selection_background(mk_runs(), 0..6, bg);
        assert_eq!(split.iter().map(|r| r.len).collect::<Vec<_>>(), vec![6, 6]);
        assert_eq!(split[0].background_color, Some(bg));
        assert_eq!(split[1].background_color, None);
        // 无交叠：原样
        let split = apply_selection_background(mk_runs(), 20..24, bg);
        assert_eq!(split.iter().map(|r| r.len).collect::<Vec<_>>(), vec![6, 6]);
        assert!(split.iter().all(|r| r.background_color.is_none()));
    }

    /// T9 交接：纯空白/符号文本的字母数字前缀为空，旧实现一律退化为
    /// "html-txt-"——br 独立成组（"\n"）、符号分隔线等组互相碰撞同键，
    /// e2e debug_bounds 查询会命中错误元素。现走内容 hash 兜底，键唯一。
    #[test]
    fn probe_key_symbol_only_groups_are_unique() {
        let nl = text_probe_key("\n");
        let dashes = text_probe_key("--- ··· ---");
        let equals = text_probe_key("==========");
        for k in [&nl, &dashes, &equals] {
            assert!(
                k.starts_with("html-txt-sym-"),
                "符号文本应走 hash 兜底: {k}"
            );
        }
        assert_ne!(nl, dashes, "不同符号文本键不得碰撞");
        assert_ne!(dashes, equals, "不同符号文本键不得碰撞");
        assert_ne!(nl, equals, "不同符号文本键不得碰撞");

        // 常规文本路径不受影响：字母数字前缀键保持原语义
        assert_eq!(text_probe_key("普通文本ABC"), "html-txt-普通文本ABC");
        assert_ne!(text_probe_key("甲"), text_probe_key("乙"));
    }

    /// 最小合法 PNG：1×1 红色像素（手工构造字节，免依赖编码器 feature）。
    fn tiny_png() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let chunk = |v: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]| {
            v.extend_from_slice(&(data.len() as u32).to_be_bytes());
            v.extend_from_slice(kind);
            v.extend_from_slice(data);
            let mut crc_data = kind.to_vec();
            crc_data.extend_from_slice(data);
            v.extend_from_slice(&crc32(&crc_data).to_be_bytes());
        };
        // IHDR: width=1 height=1 bitdepth=8 color=6(RGBA) 压缩/滤波/隔行=0
        chunk(&mut v, b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        // IDAT: zlib(stored deflate) of [filter=0, R=255, G=0, B=0, A=255]
        let mut zlib = vec![0x78, 0x01];
        zlib.extend_from_slice(&[0x01, 0x05, 0x00, 0xFA, 0xFF, 0x00, 255, 0, 0, 255]);
        let (mut s1, mut s2) = (1u32, 0u32);
        for b in [0u8, 255, 0, 0, 255] {
            s1 = (s1 + b as u32) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        zlib.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
        chunk(&mut v, b"IDAT", &zlib);
        chunk(&mut v, b"IEND", &[]);
        v
    }

    /// CRC32（PNG chunk 校验字段）。
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, t) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB88320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *t = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for b in data {
            crc = table[((crc ^ u32::from(*b)) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    /// 解码：1×1 PNG → RenderImage（RGBA→BGRA 字节序交换）。
    #[test]
    fn decodes_png_to_render_image() {
        let png = tiny_png();
        let img = decode_render_image(&png).expect("合法 PNG 应解码成功");
        assert_eq!(
            img.size(0),
            gpui_kit::size(gpui_kit::DevicePixels(1), gpui_kit::DevicePixels(1))
        );
        let bytes = img.as_bytes(0).expect("应有帧字节");
        // 原 RGBA(255,0,0,255) → BGRA(0,0,255,255)
        assert_eq!(bytes, &[0, 0, 255, 255], "R/B 应互换为 BGRA 字节序");
    }

    /// 解码拒绝：非图片字节 / 残缺数据 → None（调用方占位框兜底）。
    #[test]
    fn rejects_invalid_bytes() {
        assert!(decode_render_image(b"not an image at all").is_none());
        assert!(decode_render_image(b"").is_none());
        // GIF 魔数在白名单内但内容残缺 → 解码失败 → None
        assert!(decode_render_image(b"GIF89a\x01\x00\x01\x00broken").is_none());
    }

    /// T13 遗留 P1（T14 落地）：DataUri 解码缓存——同字节第二帧命中缓存
    /// （Arc 指针相同即零重解码）、解码失败同样入账不重复尝试。
    #[test]
    fn data_uri_decode_cache_hits_and_memoizes_failures() {
        let png = tiny_png();
        let cache: Mutex<HashMap<u64, Option<Arc<RenderImage>>>> = Mutex::new(HashMap::new());

        let first = decode_render_image_cached(&png, &cache).expect("合法 PNG 应解码成功");
        let second = decode_render_image_cached(&png, &cache).expect("缓存命中");
        assert!(
            Arc::ptr_eq(&first, &second),
            "第二次解码应命中缓存（同一 RenderImage 实例）"
        );
        assert_eq!(cache.lock().unwrap().len(), 1, "同字节只入账一次");

        // 失败入账：畸形字节第二次调用不再解码，仍返回 None
        let broken = b"GIF89a\x01\x00\x01\x00broken";
        assert!(decode_render_image_cached(broken, &cache).is_none());
        assert!(decode_render_image_cached(broken, &cache).is_none());
        assert_eq!(
            cache.lock().unwrap().len(),
            2,
            "失败结论应记账（成功 PNG + 失败 GIF 各一条）"
        );

        // 不同字节不同键：互不串缓存
        let other = decode_render_image_cached(b"not an image at all", &cache);
        assert!(other.is_none());
        assert_eq!(cache.lock().unwrap().len(), 3);
    }

    /// 缓存键内容感知：同长度不同内容的字节 hash 不同（防手滑把键做成常量）。
    #[test]
    fn data_uri_key_distinguishes_content() {
        assert_ne!(data_uri_key(b"alpha"), data_uri_key(b"omega"));
        assert_eq!(data_uri_key(b"same"), data_uri_key(b"same"));
    }

    /// 远程 URL 收集：块级/嵌套全覆盖、去重保序、非 Remote 不收。
    #[test]
    fn collects_remote_urls_dedup_in_order() {
        let img = |src: ImageRef| Block::Image {
            src,
            alt: String::new(),
            style: postium_html::ImageStyle::default(),
        };
        let doc = HtmlDocument {
            blocks: vec![
                img(ImageRef::Remote {
                    url: "https://a/1.png".into(),
                }),
                img(ImageRef::Cid {
                    content_id: "x@y".into(),
                }),
                img(ImageRef::DataUri {
                    mime: "image/png".into(),
                    bytes: vec![1].into(),
                }),
                img(ImageRef::Invalid),
                Block::Quote {
                    children: vec![Block::Container {
                        style: postium_html::BlockStyle::default(),
                        children: vec![img(ImageRef::Remote {
                            url: "https://b/2.png".into(),
                        })],
                    }],
                },
                img(ImageRef::Remote {
                    url: "https://a/1.png".into(),
                }),
            ],
        };
        let urls = collect_remote_image_urls(&doc);
        assert_eq!(
            urls,
            vec!["https://a/1.png".to_string(), "https://b/2.png".to_string()],
            "应去重保序且跳过 Cid/DataUri/Invalid"
        );
    }

    /// content-id 归一化：去尖括号 + 小写（与 builder `cid:` 后 trim+小写对齐）。
    #[test]
    fn normalizes_content_id() {
        assert_eq!(normalize_content_id("<ABC@def>"), "abc@def");
        assert_eq!(normalize_content_id("abc@DEF"), "abc@def");
        assert_eq!(normalize_content_id("  x@y  "), "x@y");
    }
}
