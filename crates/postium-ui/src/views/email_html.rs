//! GPUI 邮件 HTML 渲染层：[`HtmlDocument`] 中间表示 → GPUI 元素。
//!
//! 块级（Container/Paragraph/Heading/List/Quote/Hr/Table）与行内 run 序列
//! （同样式段合并）、表格 flex 模拟与链接 InteractiveText 点击接线；
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

use std::cell::Cell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex};

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ElementId, FontStyle, FontWeight, Hsla, InteractiveText, IntoElement as _,
    ParentElement as _, RenderImage, SharedString, StrikethroughStyle, Styled as _, StyledText,
    TextAlign, TextRun, UnderlineStyle, div, font as base_font, px, relative, rgba,
};
use postium_html::{
    Align, Block, BlockStyle, HtmlDocument, ImageStyle, InlineRun, InlineStyle, Rgba, SafeUrl,
    TableCell, TableRow, Width,
};

#[cfg(test)]
use gpui_kit::InteractiveElement as _;

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
/// 正文基础字号（px）。邮件作者未声明字号时的兜底。
const BASE_FONT_SIZE: f32 = 14.;
#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
/// 正文基础行高（倍数）。
const BASE_LINE_HEIGHT: f32 = 1.6;

/// 图片渲染上下文（Task 13 扩展真实加载：Remote 拉取 / Cid 附件 / DataUri 解码）。
#[derive(Default)]
pub struct ImageCtx {
    /// 用户是否放行远程图片（默认拦截，spec §4.4 隐私默认）。
    #[allow(dead_code)] // Task 13 消费
    pub enabled: bool,
    /// 已解析的远程图片缓存（键：URL；值 None 表示加载失败占位）。
    #[allow(dead_code)] // Task 13 消费
    pub resolved: Arc<Mutex<HashMap<String, Option<Arc<RenderImage>>>>>,
    /// 内嵌附件图片（键：content-id 小写）。
    #[allow(dead_code)] // Task 13 消费
    pub attachments: Vec<(String, Arc<RenderImage>)>,
}

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
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
    /// 链接点击回调（InteractiveText hitbox → 调用方 open_url / 状态栏预览）。
    on_link: Arc<dyn Fn(&SafeUrl) + 'static>,
    /// 链接元素 id 序号（InteractiveText::new 需唯一 ElementId；每次渲染
    /// 从 0 起算，树结构不变则 id 帧间稳定，元素状态不串）。
    link_ix: Cell<usize>,
}

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
impl RenderCtx {
    fn new(dark: bool, on_link: Arc<dyn Fn(&SafeUrl) + 'static>, cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            dark,
            font_family: theme.font_family.clone(),
            foreground: theme.foreground,
            muted_foreground: theme.muted_foreground,
            link: theme.link,
            border: theme.border,
            placeholder_bg: theme.secondary,
            on_link,
            link_ix: Cell::new(0),
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

    /// 下一个链接元素 id（渲染树内唯一）。
    fn next_link_id(&self) -> ElementId {
        let ix = self.link_ix.get();
        self.link_ix.set(ix + 1);
        ElementId::named_usize("html-link", ix)
    }
}

/// 块级递归的继承上下文。
#[derive(Clone, Copy, Default)]
#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
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
/// - `on_link`：链接点击回调（InteractiveText hitbox 命中链接区间时以
///   [`SafeUrl`] 调用；email_detail 侧 http/mailto → `cx.open_url`）。
#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
pub fn render_html_document(
    doc: &HtmlDocument,
    dark: bool,
    images: ImageCtx,
    on_link: impl Fn(&SafeUrl) + 'static,
    cx: &mut App,
) -> AnyElement {
    // Task 13：按 ImageCtx.enabled / attachments / resolved 渲染三源图片。
    let _ = images;

    let rcx = RenderCtx::new(dark, Arc::new(on_link), cx);
    render_blocks(&doc.blocks, Inherited::default(), &rcx)
        .when_probe("html-root")
        .into_any_element()
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// 块级序列 → 纵向 flex 列。
fn render_blocks(blocks: &[Block], inh: Inherited, rcx: &RenderCtx) -> gpui_kit::Div {
    let mut col = v_flex().w_full();
    for block in blocks {
        col = col.child(render_block(block, inh, rcx));
    }
    col
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
/// 表格行 → 横向 flex（cells 沿主轴排布，交叉轴拉伸等高——UA 行高 = 最高单元格）。
fn render_table_row(row: &TableRow, inh: Inherited, rcx: &RenderCtx) -> gpui_kit::Div {
    let mut r = h_flex().w_full().items_stretch();
    for cell in &row.cells {
        r = r.child(render_table_cell(cell, inh, rcx));
    }
    r
}

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
/// 表格单元格 → 容器（colspan → flex_grow 权重、width → 固定宽/百分比宽、
/// 无显式宽 basis 归零交 grow 相位分配、cell.style → map_block_style；
/// 嵌套块递归 [`render_blocks`]，空单元格占最小行高防塌陷）。
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
    // flex-basis（声明宽 + 自动扩展）。
    let mut c = map_block_style(&cell.style, rcx.dark)
        .flex()
        .flex_col()
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// 行内流 → 文本元素（字号分组的 [`StyledText`]）与行内图片占位的序列。
///
/// `default_size`：run 未声明字号时的兜底字号——由调用方按块类型传入
/// （正文 [`BASE_FONT_SIZE`]、标题按级别 [`heading_size`]），使元素级
/// 字号与包装层语义一致（文本 div 的显式 text_size 覆盖父级级联，
/// 包装层 text_size 不生效，故不走级联）。
///
/// 行内 img 已在树构建期提升为块级（plan Task 5），此处仅为 IR 保留
/// 变体兜底：占位框独立成行。
fn render_inline_flow(
    runs: &[InlineRun],
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    force_bold: bool,
    default_size: f32,
) -> Vec<AnyElement> {
    // 1) 扁平化：同样式文本段合并（含链接内层递归与 br 折行）
    let mut segs: Vec<FlowSeg> = Vec::new();
    flatten_runs(runs, None, force_bold, &mut segs);

    // 2) 连续文本段按字号分组为单个 StyledText（组内相邻同样式 run 再合并）
    let mut els: Vec<AnyElement> = Vec::new();
    let mut group: Vec<FlowSeg> = Vec::new();
    let mut group_size: Option<Option<f32>> = None;
    for seg in segs {
        match seg {
            FlowSeg::Image { style, alt } => {
                flush_text_group(
                    std::mem::take(&mut group),
                    base_color,
                    rcx,
                    default_size,
                    &mut els,
                );
                group_size = None;
                els.push(image_placeholder(&style, &alt, rcx));
            }
            text_seg @ FlowSeg::Text { .. } => {
                let size = match &text_seg {
                    FlowSeg::Text { style, .. } => style.font_size,
                    FlowSeg::Image { .. } => None,
                };
                if group_size.is_some() && group_size != Some(size) {
                    flush_text_group(
                        std::mem::take(&mut group),
                        base_color,
                        rcx,
                        default_size,
                        &mut els,
                    );
                }
                group_size = Some(size);
                group.push(text_seg);
            }
        }
    }
    flush_text_group(group, base_color, rcx, default_size, &mut els);
    els
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// 行内流中间产物。
enum FlowSeg {
    /// 文本段（同样式段已合并；`link` 携带归属链接的 [`SafeUrl`]，Task 10
    /// 供 InteractiveText 区间注册——不同 href 的相邻段不合并）。
    Text {
        text: String,
        style: InlineStyle,
        link: Option<SafeUrl>,
    },
    /// 行内图片占位（Task 13 接管）。
    Image { style: ImageStyle, alt: String },
}

#[allow(dead_code)] // T11 接入 email_detail 前无生产调用方
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
                // 分支注释与 task-9 报告）。
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
                let _ = src; // Task 13：三源真实渲染
                out.push(FlowSeg::Image {
                    style: style.clone(),
                    alt: alt.clone(),
                });
            }
        }
    }
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// 把积累的文本段组渲染为一个文本元素（相邻同样式段合并成单一 [`TextRun`]）。
///
/// 组内含链接段时以 [`InteractiveText`] 承载：链接段按 seg 级标记 + 段长
/// 累加为字节区间（`TextRun.len` / `index_for_position` 均以字节计）注册
/// clickable range，命中即以段归属的 [`SafeUrl`] 调用 `on_link`。
fn flush_text_group(
    group: Vec<FlowSeg>,
    base_color: Option<Hsla>,
    rcx: &RenderCtx,
    default_size: f32,
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

    let d = div()
        .w_full()
        .text_size(px(size.unwrap_or(default_size)))
        .line_height(relative(BASE_LINE_HEIGHT));

    let d = if links.is_empty() {
        d.child(StyledText::new(text.clone()).with_runs(runs))
    } else {
        let ranges: Vec<Range<usize>> = links.iter().map(|(r, _)| r.clone()).collect();
        let urls: Vec<SafeUrl> = links.into_iter().map(|(_, u)| u).collect();
        let on_link = Arc::clone(&rcx.on_link);
        d.child(
            InteractiveText::new(
                rcx.next_link_id(),
                StyledText::new(text.clone()).with_runs(runs),
            )
            .on_click(ranges, move |range_ix, _window, _cx| {
                if let Some(url) = urls.get(range_ix) {
                    on_link(url);
                }
            }),
        )
    };

    // 文本探针（仅测试构建）：以内容前缀为键记录渲染树，
    // e2e 经 `VisualTestContext::debug_bounds` 查询文本可见性。
    #[cfg(test)]
    let d = {
        let key = text_probe_key(&text);
        d.debug_selector(move || key)
    };
    els.push(d.into_any_element());
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// IR 颜色（RGBA 0–255）→ GPUI [`Hsla`]。
fn ir_color(c: Rgba) -> Hsla {
    rgba(u32::from_be_bytes(c)).into()
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
/// IR 对齐 → GPUI 文本对齐（Justify 降级 Left：GPUI 暂无两端对齐）。
fn map_align(a: Align) -> TextAlign {
    match a {
        Align::Left | Align::Justify => TextAlign::Left,
        Align::Center => TextAlign::Center,
        Align::Right => TextAlign::Right,
    }
}

#[allow(dead_code)] // Task 11 接入 email_detail 正文分支
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
#[allow(dead_code)] // Task 11 接入 email_detail 前仅探针路径使用
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
    use super::text_probe_key;

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
}
