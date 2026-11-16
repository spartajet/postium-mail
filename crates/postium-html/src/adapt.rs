//!
//! # 深色模式智能反转（spec §6）
//!
//! 解析产物 [`HtmlDocument`] 上的纯函数：不接触原始 HTML、不做 IO，UI 层在主题
//! 切换时计算一次并随文档缓存（dark/light 两份按需生成，原始文档不被修改）。
//!
//! ## 触发判定 [`needs_dark_adapt`]
//!
//! 按文档序深度优先扫描，取**首个显式背景**（Container/Paragraph/Table/
//! TableCell 的 `background_color`，alpha=0 的全透明背景不算显式——"透明背景
//! 视为浅底"是邮件正文常态）：亮度 ≥ 0.6 视为浅底文档；找不到任何显式背景
//! 同样视为浅底。首个背景即定调（spec §6 启发式），不做加权综合。
//!
//! ## 重映射 [`remap`]
//!
//! - **背景**：`L' = 1 - L` 后压缩到 [0.10, 0.25] 深灰区间，保持色相（spec §6
//!   "深灰体系"）；
//! - **文字**：亮度提升至 ≥ 0.75（可读区间，"暗底亮字"）；
//! - **饱和色块豁免**：HSV S ≥ 0.35 且亮度 ∈ [0.15, 0.85] 的**背景**不动——
//!   营销邮件的品牌色块（brief 测试用例 `#1a73e8` 蓝底头图）保持原样。豁免只
//!   作用于背景角色：spec §6 动作清单里"文字颜色亮度重映射至可读区间"无例外，
//!   中低亮度的饱和文字照常提亮（暗底可读性优先）；
//! - 边框颜色按背景角色重映射（浅色发丝线在深底上刺目，同属表面装饰）。
//!
//! ## 不变量
//!
//! - `adapt_document(doc, false)` 原样 clone（零变换路径）；
//! - 深底文档（`needs_dark_adapt == false`）原样 clone；
//! - 遍历覆盖全部 IR 形状，但**不触碰任何图片字节**（`ImageRef::DataUri` 的
//!   `Arc` 原样共享，`data:image/svg+xml` 只进栅格解码器的裁定在渲染层 T13）；
//! - MAX_TREE_DEPTH 的静默剪枝是树构建期（builder.rs）语义，本模块消费的
//!   已是剪枝后的 IR，二者无关。

use crate::types::{
    Block, BlockStyle, HtmlDocument, InlineRun, InlineStyle, Rgba, TableCell, TableRow,
};

/// 浅底判定阈值：首个显式背景亮度 ≥ 0.6 视为浅底文档（spec §6 触发条件）。
const LIGHT_BG_LUMA: f32 = 0.6;

/// 背景重映射目标区间：`L' = 1 - L` 压缩到 [0.10, 0.25]（深灰体系，spec §6）。
const BG_LUMA_MIN: f32 = 0.10;
const BG_LUMA_MAX: f32 = 0.25;

/// 文字亮度下限 0.75（brief 契约）；实际提亮目标 0.78——留出 u8 舍入余量
/// （逐通道舍入误差 ≤ 0.5/255 ≈ 0.002），保证舍入后仍 ≥ 0.75。
const FG_LUMA_MIN: f32 = 0.75;
const FG_LUMA_TARGET: f32 = 0.78;

/// 饱和判定（brief）：HSV S ≥ 0.35 且亮度 ∈ [0.15, 0.85] → 有色块（背景）不动。
const SATURATION_MIN: f32 = 0.35;
const SAT_LUMA_MIN: f32 = 0.15;
const SAT_LUMA_MAX: f32 = 0.85;

/// 深色模式适配：`dark = false` 原样 clone；深底文档原样 clone；浅底文档按
/// [`remap`] 重映射全部颜色（背景/边框按背景角色、文字按前景角色）。
pub fn adapt_document(doc: &HtmlDocument, dark: bool) -> HtmlDocument {
    if !dark || !needs_dark_adapt(doc) {
        // 非深色模式 / 已是深底文档：原样 clone（零变换路径，spec §6 缓存语义）
        return doc.clone();
    }
    HtmlDocument {
        blocks: adapt_blocks(&doc.blocks),
    }
}

/// 是否需要深色适配：首个显式背景亮度 ≥ 0.6 视为浅底文档；无任何显式背景
/// （透明）视为浅底（邮件正文常态）。
pub fn needs_dark_adapt(doc: &HtmlDocument) -> bool {
    first_explicit_background(&doc.blocks).is_none_or(|bg| luma(bg) >= LIGHT_BG_LUMA)
}

/// 按文档序深度优先取首个显式背景（alpha=0 的全透明背景不算显式，继续向后找）。
fn first_explicit_background(blocks: &[Block]) -> Option<Rgba> {
    let own = |style: &BlockStyle| style.background_color.filter(|color| color[3] > 0);
    blocks.iter().find_map(|block| match block {
        Block::Container { style, children } => {
            own(style).or_else(|| first_explicit_background(children))
        }
        Block::Paragraph { style, .. } => own(style),
        // 标题无 style 字段；图片/分隔线无背景语义
        Block::Heading { .. } | Block::Image { .. } | Block::Hr => None,
        Block::List { items, .. } => items
            .iter()
            .find_map(|item| first_explicit_background(item)),
        Block::Table { style, rows } => own(style).or_else(|| {
            rows.iter().find_map(|row| {
                row.cells.iter().find_map(|cell| {
                    own(&cell.style).or_else(|| first_explicit_background(&cell.blocks))
                })
            })
        }),
        Block::Quote { children } => first_explicit_background(children),
    })
}

/// 块序列整体适配（顺序保持，逐块重建为新树——原始文档不被修改）。
fn adapt_blocks(blocks: &[Block]) -> Vec<Block> {
    blocks.iter().map(adapt_block).collect()
}

/// 单块适配：遍历覆盖全部 IR 形状；图片（块级/行内）样式无颜色字段、来源
/// 字节零触碰（`Arc` 引用共享，`data:image/svg+xml` 的解码裁定在渲染层 T13）。
fn adapt_block(block: &Block) -> Block {
    match block {
        Block::Container { style, children } => Block::Container {
            style: adapt_block_style(style),
            children: adapt_blocks(children),
        },
        Block::Paragraph { style, runs, align } => Block::Paragraph {
            style: adapt_block_style(style),
            runs: adapt_runs(runs),
            align: *align,
        },
        Block::Heading { level, align, runs } => Block::Heading {
            level: *level,
            align: *align,
            runs: adapt_runs(runs),
        },
        Block::List { ordered, items } => Block::List {
            ordered: *ordered,
            items: items.iter().map(|item| adapt_blocks(item)).collect(),
        },
        Block::Table { style, rows } => Block::Table {
            style: adapt_block_style(style),
            rows: rows.iter().map(adapt_table_row).collect(),
        },
        Block::Quote { children } => Block::Quote {
            children: adapt_blocks(children),
        },
        Block::Image { src, alt, style } => Block::Image {
            src: src.clone(),
            alt: alt.clone(),
            style: style.clone(),
        },
        Block::Hr => Block::Hr,
    }
}

fn adapt_table_row(row: &TableRow) -> TableRow {
    TableRow {
        cells: row.cells.iter().map(adapt_table_cell).collect(),
    }
}

fn adapt_table_cell(cell: &TableCell) -> TableCell {
    TableCell {
        blocks: adapt_blocks(&cell.blocks),
        colspan: cell.colspan,
        rowspan: cell.rowspan,
        width: cell.width,
        style: adapt_block_style(&cell.style),
    }
}

/// 块级样式适配：背景按背景角色；边框色同属表面装饰按背景角色（浅色发丝线
/// 在深底上刺目）；块级 color 是文字色的声明位置，按前景角色。
fn adapt_block_style(style: &BlockStyle) -> BlockStyle {
    let mut adapted = style.clone();
    adapted.background_color = adapted.background_color.map(|color| remap(color, true));
    adapted.color = adapted.color.map(|color| remap(color, false));
    for border in [
        &mut adapted.border_top,
        &mut adapted.border_right,
        &mut adapted.border_bottom,
        &mut adapted.border_left,
    ]
    .into_iter()
    .flatten()
    {
        border.color = remap(border.color, true);
    }
    adapted
}

/// 行内 run 适配：Text 的 color 按前景角色；Link 递归内层 runs（href 不动）；
/// 行内图片无颜色、Br 无样式。
fn adapt_runs(runs: &[InlineRun]) -> Vec<InlineRun> {
    runs.iter()
        .map(|run| match run {
            InlineRun::Text { text, style } => InlineRun::Text {
                text: text.clone(),
                style: InlineStyle {
                    color: style.color.map(|color| remap(color, false)),
                    ..style.clone()
                },
            },
            InlineRun::Link { href, runs } => InlineRun::Link {
                href: href.clone(),
                runs: adapt_runs(runs),
            },
            InlineRun::Image { src, alt, style } => InlineRun::Image {
                src: src.clone(),
                alt: alt.clone(),
                style: style.clone(),
            },
            InlineRun::Br => InlineRun::Br,
        })
        .collect()
}

/// 单色重映射：`bg = true` 走背景通道（亮度翻转后压缩到深灰区间），否则走
/// 文字通道（提亮至 ≥ 0.75）。饱和色块（背景角色）原样返回。
fn remap(color: Rgba, bg: bool) -> Rgba {
    let [r, g, b, a] = color;
    let l = luma(color);
    // 饱和色块豁免仅作用于背景（spec §6 动作清单：文字一律重映射至可读区间，
    // "饱和色块与图片不动"指色块）；营销蓝 #1a73e8 等品牌背景保持原样
    if bg && is_saturated(color) {
        return color;
    }
    // 目标亮度：背景 = 1 - L 压缩到深灰区间；文字 = 提亮目标与现值取大
    //（提亮目标对契约下限 0.75 硬保证——0.03 差值即 u8 舍入余量；已达标文字
    // target == l → 走恒等分支原样返回）
    let target = if bg {
        (1.0 - l).clamp(BG_LUMA_MIN, BG_LUMA_MAX)
    } else {
        FG_LUMA_TARGET.max(FG_LUMA_MIN).max(l)
    };
    // 亮度对齐保持色相：压暗走等比缩放（等价向黑插值，通道比例不变、无钳制）；
    // 提亮走向白插值——等比放大在满亮通道处被 255 钳制后无法达标（纯蓝/藏青
    // 类数学上不可达），向白插值无钳制、色相方向保持，饱和度随亮度向白衰减
    //（"保饱和度"取保色彩感语义：暗饱和文字的可读性优先）
    let mut rgb = if l > target {
        [r, g, b].map(|c| (f32::from(c) * target / l).round() as u8)
    } else if l < target {
        let k = (target - l) / (1.0 - l);
        [r, g, b].map(|c| (f32::from(c) + (255.0 - f32::from(c)) * k).round() as u8)
    } else {
        [r, g, b]
    };
    if bg {
        // u8 舍入漂移（≤ 0.002）可能擦出区间边缘：微调回 [0.10, 0.25] 保证契约
        nudge_luma_into_band(&mut rgb, BG_LUMA_MIN, BG_LUMA_MAX);
    }
    // 文字路径无需微调：向白插值舍入前亮度精确等于目标 0.78，逐通道舍入误差
    // ≤ 0.5/255 ≈ 0.002，结果 ≥ 0.778 > 0.75 下限（构造性保证，无需回调）
    [rgb[0], rgb[1], rgb[2], a]
}

/// 把 u8 舍入后的通道亮度微调回区间（步进序按亮度权重 g → r → b，单步
/// Δg ≈ 0.0023，数步内收敛；构造上总有可动通道——亮度 < lo 必有通道 < 255、
/// 亮度 > hi 必有通道 > 0，循环必然终止）。
fn nudge_luma_into_band(rgb: &mut [u8; 3], lo: f32, hi: f32) {
    for _ in 0..1024 {
        let l = luma3(*rgb);
        if (lo..=hi).contains(&l) {
            return;
        }
        let step = if l > hi { -1 } else { 1 };
        let mut moved = false;
        for &i in &[1, 0, 2] {
            let next = i32::from(rgb[i]) + step;
            if (0..=255).contains(&next) {
                rgb[i] = next as u8;
                moved = true;
                break;
            }
        }
        if !moved {
            return; // 全通道顶格/见底仍出区间：区间与 [0,1] 不相交时才可能，构造上不可达
        }
    }
}

/// 感知亮度（Rec.601 luma，0.0–1.0）：本模块统一的"亮度"度量——浅底判定阈值、
/// remap 的翻转/压缩目标与饱和判定的亮度窗口同源，避免多套亮度语义。
fn luma(color: Rgba) -> f32 {
    luma3([color[0], color[1], color[2]])
}

fn luma3(rgb: [u8; 3]) -> f32 {
    (0.299 * f32::from(rgb[0]) + 0.587 * f32::from(rgb[1]) + 0.114 * f32::from(rgb[2])) / 255.0
}

/// 有色块判定（brief 饱和判定）：HSV S ≥ 0.35 且亮度 ∈ [0.15, 0.85]。
///
/// 窗口的"V"取本模块统一的感知亮度而非 HSV 原始 Value：remap 全程以该亮度为
/// 度量（语义同源），且 brief 测试用例 `#1a73e8` 的 HSV V=0.91 落在窗口外、
/// 其感知亮度 0.40 在窗口内——"营销蓝底不动"的语义以 brief 测试用例为准
/// （按 HSV 原始 V 判定则 `saturated_block_untouched` 无法成立）。
fn is_saturated(color: Rgba) -> bool {
    saturation(color) >= SATURATION_MIN && (SAT_LUMA_MIN..=SAT_LUMA_MAX).contains(&luma(color))
}

/// HSV 饱和度 S = (max - min) / max（纯黑无色度，定义为 0）。
fn saturation(color: Rgba) -> f32 {
    let [r, g, b, _] = color;
    let max = f32::from(r.max(g).max(b));
    let min = f32::from(r.min(g).min(b));
    if max <= 0.0 { 0.0 } else { (max - min) / max }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_document;
    use crate::types::{Align, Block, HtmlDocument, ImageRef, InlineRun, Limits, SafeUrl};
    use std::sync::Arc;

    fn build(html: &str) -> HtmlDocument {
        build_document(html, &Limits::default()).unwrap()
    }

    /// 递归收集全部 Text run 的颜色（含 Link 内层 run）。
    fn text_colors(blocks: &[Block]) -> Vec<Rgba> {
        fn walk_blocks(blocks: &[Block], out: &mut Vec<Rgba>) {
            for block in blocks {
                match block {
                    Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => {
                        walk_runs(runs, out)
                    }
                    Block::Container { children, .. } | Block::Quote { children } => {
                        walk_blocks(children, out)
                    }
                    Block::List { items, .. } => {
                        for item in items {
                            walk_blocks(item, out);
                        }
                    }
                    Block::Table { rows, .. } => {
                        for row in rows {
                            for cell in &row.cells {
                                walk_blocks(&cell.blocks, out);
                            }
                        }
                    }
                    Block::Image { .. } | Block::Hr => {}
                }
            }
        }
        fn walk_runs(runs: &[InlineRun], out: &mut Vec<Rgba>) {
            for run in runs {
                match run {
                    InlineRun::Text { style, .. } => {
                        if let Some(color) = style.color {
                            out.push(color);
                        }
                    }
                    InlineRun::Link { runs, .. } => walk_runs(runs, out),
                    InlineRun::Image { .. } | InlineRun::Br => {}
                }
            }
        }
        let mut out = Vec::new();
        walk_blocks(blocks, &mut out);
        out
    }

    /// 递归收集全部块级背景色（Container/Paragraph/Table/TableCell）。
    fn bg_colors(blocks: &[Block]) -> Vec<Rgba> {
        fn walk(blocks: &[Block], out: &mut Vec<Rgba>) {
            for block in blocks {
                match block {
                    Block::Container { style, children } => {
                        if let Some(color) = style.background_color {
                            out.push(color);
                        }
                        walk(children, out);
                    }
                    Block::Paragraph { style, .. } => {
                        if let Some(color) = style.background_color {
                            out.push(color);
                        }
                    }
                    Block::Heading { .. } => {} // 标题无 style 字段
                    Block::List { items, .. } => {
                        for item in items {
                            walk(item, out);
                        }
                    }
                    Block::Table { style, rows } => {
                        if let Some(color) = style.background_color {
                            out.push(color);
                        }
                        for row in rows {
                            for cell in &row.cells {
                                if let Some(color) = cell.style.background_color {
                                    out.push(color);
                                }
                                walk(&cell.blocks, out);
                            }
                        }
                    }
                    Block::Quote { children } => walk(children, out),
                    Block::Image { .. } | Block::Hr => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(blocks, &mut out);
        out
    }

    fn data_uri_bytes(blocks: &[Block]) -> &Arc<[u8]> {
        // 行内图片保留在段落 runs 内（线上反馈③第二波），块级与行内两形态都收
        fn walk(blocks: &[Block]) -> Option<&Arc<[u8]>> {
            blocks.iter().find_map(|block| match block {
                Block::Image {
                    src: ImageRef::DataUri { bytes, .. },
                    ..
                } => Some(bytes),
                Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => walk_runs(runs),
                Block::Container { children, .. } | Block::Quote { children } => walk(children),
                Block::List { items, .. } => items.iter().find_map(|item| walk(item)),
                Block::Table { rows, .. } => rows
                    .iter()
                    .find_map(|row| row.cells.iter().find_map(|cell| walk(&cell.blocks))),
                _ => None,
            })
        }
        fn walk_runs(runs: &[InlineRun]) -> Option<&Arc<[u8]>> {
            runs.iter().find_map(|run| match run {
                InlineRun::Image {
                    src: ImageRef::DataUri { bytes, .. },
                    ..
                } => Some(bytes),
                InlineRun::Link { runs, .. } => walk_runs(runs),
                _ => None,
            })
        }
        walk(blocks).expect("文档应含 DataUri 图片")
    }

    // ---- brief 测试（逐字采用）----

    #[test]
    fn white_bg_dark_text_adapts() {
        // 白底(255)黑字(17) → 背景入 0.10-0.25 区间、文字亮度 ≥ 0.75
        let doc = build(
            r#"<div style="background-color:#ffffff"><p style="color:#111111">正文</p></div>"#,
        );
        assert!(needs_dark_adapt(&doc));
        let adapted = adapt_document(&doc, true);
        let Block::Container { style, children } = &adapted.blocks[0] else {
            panic!("顶层应为 Container")
        };
        let bg = style.background_color.expect("白底应保留背景字段");
        assert!(
            (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(bg)),
            "背景 {bg:?} 亮度 {} 应入 0.10–0.25 区间",
            luma(bg)
        );
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        let fg = style.color.expect("黑字应保留颜色字段");
        assert!(luma(fg) >= FG_LUMA_MIN, "文字 {fg:?} 亮度应 ≥ 0.75");
    }

    #[test]
    fn saturated_block_untouched() {
        // 蓝底白字营销头图区(#1a73e8 背景) → 背景色不变
        let doc = build(
            r#"<div style="background-color:#1a73e8"><p style="color:#ffffff">头图标题</p></div>"#,
        );
        let adapted = adapt_document(&doc, true);
        let Block::Container { style, children } = &adapted.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert_eq!(style.background_color, Some([0x1a, 0x73, 0xe8, 255]));
        // 白字亮度已达标且非饱和，同样原样
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 255, 255, 255]));
    }

    #[test]
    fn dark_document_untouched() {
        // 深底文档 needs_dark_adapt=false → clone 原样
        let doc = build(
            r#"<div style="background-color:#1e1e1e"><p style="color:#dddddd">暗色正文</p></div>"#,
        );
        assert!(!needs_dark_adapt(&doc));
        assert_eq!(adapt_document(&doc, true), doc.clone());
        // dark=false 恒等原样 clone（深底文档同样成立）
        assert_eq!(adapt_document(&doc, false), doc.clone());
    }

    // ---- needs_dark_adapt：首个显式背景定调 ----

    #[test]
    fn needs_dark_adapt_scans_first_explicit_background() {
        // 白色顶层容器 → 浅底
        assert!(needs_dark_adapt(&build(
            r#"<div style="background-color:#ffffff"><p>x</p></div>"#
        )));
        // 深色顶层容器 → 深底
        assert!(!needs_dark_adapt(&build(
            r#"<div style="background-color:#1e1e1e"><p>x</p></div>"#
        )));
        // 无任何显式背景（邮件正文常态）→ 透明视为浅底
        assert!(needs_dark_adapt(&build("<p>x</p>")));
        // 全透明背景（alpha=0）不算显式背景
        assert!(needs_dark_adapt(&build(
            r#"<div style="background-color:rgba(0,0,0,0)"><p>x</p></div>"#
        )));
        // 顶层无背景：按文档序首个显式背景决定——深层暗块 → 深底、浅块 → 浅底
        assert!(!needs_dark_adapt(&build(
            r#"<div><div style="background-color:#222222"><p>x</p></div></div>"#
        )));
        assert!(needs_dark_adapt(&build(
            r#"<div><div style="background-color:#fafafa"><p>x</p></div></div>"#
        )));
        // 表格单元格背景计入扫描（表格布局邮件的主流形态）
        assert!(needs_dark_adapt(&build(
            r#"<table><tr><td style="background-color:#ffffff"><p>x</p></td></tr></table>"#
        )));
        assert!(!needs_dark_adapt(&build(
            r#"<table><tr><td style="background-color:#1a1a1a"><p>x</p></td></tr></table>"#
        )));
    }

    // ---- remap：表驱动（背景区间 / 文字提亮 / 饱和豁免 / alpha 保持）----

    #[test]
    fn remap_background_band_table_driven() {
        // 非饱和背景 → 亮度入 [0.10, 0.25] 且色相保持（通道序不变）；alpha 原样
        const CASES: &[Rgba] = &[
            [255, 255, 255, 255], // 白
            [238, 238, 238, 255], // #eee
            [221, 221, 221, 255], // #ddd
            [128, 128, 128, 255], // 中性灰
            [240, 230, 210, 255], // 暖米色（低饱和度，S=0.125）
            [0, 0, 0, 255],       // 纯黑块（浅底文档里的暗块 → 深灰体系）
            [0, 0, 128, 255],     // 藏青（亮度 < 0.15 不算有色块 → 翻转）
            [255, 255, 0, 255],   // 亮黄（亮度 > 0.85 不算有色块 → 压暗）
        ];
        for color in CASES {
            let out = remap(*color, true);
            assert_eq!(out[3], color[3], "alpha 应保持 {color:?}");
            let l = luma(out);
            assert!(
                (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&l),
                "背景 {color:?} → {out:?} 亮度 {l} 应入 [0.10, 0.25]"
            );
            // 色相保持：严格有序通道的序不变；并列/灰阶色允许区间微调的 ±1 步进
            //（band 契约优先于并列通道的精确保持——见 nudge_luma_into_band）
            let spread = |c: &Rgba| c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2]);
            if color[0] > color[1] && color[1] > color[2] {
                assert!(
                    out[0] > out[1] && out[1] > out[2],
                    "通道序应保持 {color:?} → {out:?}"
                );
            } else {
                assert!(
                    i32::from(spread(&out)) <= i32::from(spread(color)) + 1,
                    "并列/灰阶色相漂移应 ≤ 1：{color:?} → {out:?}"
                );
            }
        }
        // 饱和背景豁免：品牌色块原样
        const SATURATED: &[Rgba] = &[
            [0x1a, 0x73, 0xe8, 255], // #1a73e8（brief 用例，营销蓝）
            [0, 128, 0, 255],        // green
            [128, 0, 128, 255],      // purple
        ];
        for color in SATURATED {
            assert_eq!(remap(*color, true), *color, "饱和背景应不动 {color:?}");
        }
        // 半透明背景：RGB 重映射、alpha 原样
        let out = remap([255, 255, 255, 128], true);
        assert_eq!(out[3], 128);
        assert!((BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(out)));
    }

    #[test]
    fn remap_foreground_table_driven() {
        // 暗文字一律提亮至 ≥ 0.75（饱和文字不豁免——spec §6 文字无例外）；
        // 已达标文字原样；alpha 原样
        const DARK: &[Rgba] = &[
            [17, 17, 17, 255],    // #111
            [51, 51, 51, 255],    // #333
            [0, 0, 0, 255],       // 纯黑
            [0, 0, 238, 255],     // 默认链接蓝（亮度 < 0.15 不豁免）
            [255, 0, 0, 255],     // 纯红（饱和但属文字角色 → 提亮）
            [102, 102, 102, 255], // #666
        ];
        for color in DARK {
            let out = remap(*color, false);
            assert!(
                luma(out) >= FG_LUMA_MIN,
                "文字 {color:?} → {out:?} 应 ≥ 0.75"
            );
            assert_eq!(out[3], color[3], "alpha 应保持 {color:?}");
        }
        // 已亮文字原样（≥ 目标 0.78）
        const BRIGHT: &[Rgba] = &[
            [255, 255, 255, 255],
            [220, 220, 220, 255],
            [200, 200, 200, 255], // 0.784 ≥ 0.78
        ];
        for color in BRIGHT {
            assert_eq!(remap(*color, false), *color, "已亮文字应不动 {color:?}");
        }
    }

    // ---- 浅底文档全形状遍历 ----

    #[test]
    fn light_document_adapts_every_shape_and_keeps_images() {
        let doc = build(
            r#"<div style="background-color:#ffffff;color:#111111">
              <p>前文 <a href="https://example.com" style="color:#333333">链接</a><br>换行</p>
              <h2 style="color:#222222;text-align:center">标题</h2>
              <ul><li style="color:#444444">列表项</li></ul>
              <blockquote><p style="color:#555555">引用</p></blockquote>
              <table><tr><td style="background-color:#dddddd;color:#666666">单元格</td></tr></table>
              <p><img src="data:image/png;base64,iVBORw0KGgo=" alt="图表"></p>
              <p style="background-color:#c0c0c0"></p>
              <hr>
            </div>"#,
        );
        assert!(needs_dark_adapt(&doc));
        let adapted = adapt_document(&doc, true);

        // 全部文字提亮至可读区间（原色 #111–#666 均为暗灰）
        let colors = text_colors(&adapted.blocks);
        assert_eq!(colors.len(), 7, "七个文本 run 的颜色都应保留");
        for color in &colors {
            assert!(luma(*color) >= FG_LUMA_MIN, "文字 {color:?} 应 ≥ 0.75");
        }
        // 全部块级背景落入深灰区间
        let backgrounds = bg_colors(&adapted.blocks);
        assert_eq!(
            backgrounds.len(),
            3,
            "白底/空段灰底/单元格深灰三处背景都应保留"
        );
        for color in &backgrounds {
            assert!(
                (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(*color)),
                "背景 {color:?} 应入 [0.10, 0.25]"
            );
        }

        // 结构与资源逐一保持：Container → [Paragraph, Heading, List, Quote, Table, Image, Paragraph(空段), Hr]
        let Block::Container { children, .. } = &adapted.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert_eq!(children.len(), 8);
        // 链接：href 原样、内层 run 递归重映射
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Link {
            href: SafeUrl::Http(url),
            runs: link_runs,
        } = &runs[1]
        else {
            panic!("应为 Link")
        };
        assert_eq!(url, "https://example.com");
        let InlineRun::Text { style, .. } = &link_runs[0] else {
            panic!("链接内应为 Text")
        };
        assert!(luma(style.color.expect("链接文字应保留颜色")) >= FG_LUMA_MIN);
        // 标题：级别/对齐/文字颜色
        let Block::Heading {
            level, align, runs, ..
        } = &children[1]
        else {
            panic!("应为 Heading")
        };
        assert_eq!(*level, 2);
        assert_eq!(*align, Align::Center);
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert!(luma(style.color.expect("标题文字应保留颜色")) >= FG_LUMA_MIN);
        // 列表项与引用（深度递归）
        let Block::List { ordered, items, .. } = &children[2] else {
            panic!("应为 List")
        };
        assert!(!ordered);
        let Block::Paragraph { runs, .. } = &items[0][0] else {
            panic!("列表项应为段落")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert!(luma(style.color.expect("列表项文字应保留颜色")) >= FG_LUMA_MIN);
        let Block::Quote { children: quote } = &children[3] else {
            panic!("应为 Quote")
        };
        let Block::Paragraph { runs, .. } = &quote[0] else {
            panic!("引用内应为段落")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert!(luma(style.color.expect("引用文字应保留颜色")) >= FG_LUMA_MIN);
        // 表格单元格：cell 背景入区间（bg_colors 已覆盖），结构在
        assert!(matches!(&children[4], Block::Table { .. }));
        // 图片：DataUri 字节零触碰——Arc 指针相同即零拷贝共享，内容无从改变
        assert!(Arc::ptr_eq(
            data_uri_bytes(&doc.blocks),
            data_uri_bytes(&adapted.blocks)
        ));
        // 行内图片保留在段落 runs 内（线上反馈③第二波）
        let Block::Paragraph { runs, .. } = &children[5] else {
            panic!("应为图片段落")
        };
        let InlineRun::Image { alt, .. } = &runs[0] else {
            panic!("应为 Image run")
        };
        assert_eq!(alt, "图表");
        // 空段落（间距惯用法）：自身 style 背景经块级适配通道
        let Block::Paragraph { style, runs, .. } = &children[6] else {
            panic!("应为空段 Paragraph")
        };
        assert!(runs.is_empty());
        assert!(
            (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(style.background_color.unwrap())),
            "空段背景应入深灰区间"
        );
        // 分隔线
        assert_eq!(children[7], Block::Hr);
    }

    #[test]
    fn light_mode_returns_equal_clone() {
        // dark=false：浅底文档也原样 clone（零变换路径）
        let doc = build(
            r#"<div style="background-color:#ffffff"><p style="color:#111111">正文</p></div>"#,
        );
        assert_eq!(adapt_document(&doc, false), doc.clone());
    }

    #[test]
    fn saturated_background_exempt_in_light_document() {
        // 浅底文档内的饱和色块：白底适配、蓝块与块上白字原样（豁免的真实路径——
        // brief 的 saturated_block_untouched 场景里蓝块即首个背景，走深底文档
        // clone 分支；此处锁定浅底文档中饱和豁免本身）
        let doc = build(
            r#"<div style="background-color:#ffffff">
              <p style="color:#111111">正文</p>
              <div style="background-color:#1a73e8;color:#ffffff"><p>按钮文字</p></div>
            </div>"#,
        );
        assert!(needs_dark_adapt(&doc));
        let adapted = adapt_document(&doc, true);
        let Block::Container { style, children } = &adapted.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert!(
            (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(style.background_color.unwrap())),
            "外层白底应入深灰区间"
        );
        let Block::Container {
            style: banner,
            children: banner_children,
        } = &children[1]
        else {
            panic!("应为蓝色块 Container")
        };
        assert_eq!(banner.background_color, Some([0x1a, 0x73, 0xe8, 255]));
        let Block::Paragraph { runs, .. } = &banner_children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 255, 255, 255]));
    }

    #[test]
    fn border_colors_remap_as_background() {
        // 浅色发丝线在深底上刺目 → 边框按背景角色重映射入深灰区间；饱和边框豁免
        let doc = build(
            r#"<div style="background-color:#ffffff;border:1px solid #eeeeee;border-top:2px solid #1a73e8"><p>x</p></div>"#,
        );
        let adapted = adapt_document(&doc, true);
        let Block::Container { style, .. } = &adapted.blocks[0] else {
            panic!("顶层应为 Container")
        };
        for border in [
            &style.border_top,
            &style.border_right,
            &style.border_bottom,
            &style.border_left,
        ] {
            let border = border.as_ref().expect("四边边框都应保留");
            if border.color == [0x1a, 0x73, 0xe8, 255] {
                continue; // 饱和边框豁免（border-top 覆盖了全边简写的 top 侧）
            }
            assert!(
                (BG_LUMA_MIN..=BG_LUMA_MAX).contains(&luma(border.color)),
                "边框色 {:#?} 应入 [0.10, 0.25]",
                border.color
            );
        }
    }

    // ---- 亮度/饱和度单元基准 ----

    #[test]
    fn luma_reference_values() {
        // Rec.601 加权：白 1.0、黑 0.0、纯红 0.299、#1a73e8 ≈ 0.399
        assert_eq!(luma([255, 255, 255, 255]), 1.0);
        assert_eq!(luma([0, 0, 0, 255]), 0.0);
        assert!((luma([255, 0, 0, 255]) - 0.299).abs() < 1e-6);
        assert!((luma([0x1a, 0x73, 0xe8, 255]) - 0.399).abs() < 0.001);
        // 饱和判定窗口边界：纯黑/纯白/亮黄不在窗口，#1a73e8 在
        assert!(!is_saturated([0, 0, 0, 255]));
        assert!(!is_saturated([255, 255, 255, 255]));
        assert!(!is_saturated([255, 255, 0, 255]));
        assert!(is_saturated([0x1a, 0x73, 0xe8, 255]));
        assert!(!is_saturated([128, 128, 128, 255]));
    }
}
