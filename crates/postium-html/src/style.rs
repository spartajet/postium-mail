//!
//! # 行内样式解析映射表（spec §3 映射表）
//!
//! 输入为 css-inline 内联后元素 `style` 属性的规整 CSS 声明串，按白名单映射到
//! [`crate::types`] 的中间表示类型。解析不引入 cssparser：声明按 `;` 分割、
//! 键值以首个 `:` 分割（键大小写不敏感、值去首尾空白与 `!important` 后缀）。
//!
//! **白名单即安全边界**：映射表之外的属性一律静默忽略——`background-image`、
//! `background: url(...)` 等携带 `url()` 的资源向量正是经此丢弃（spec §4.4
//! 邮件正文远程资源不放行的解析期机制，见 [`parse_block_style`]）。
//!
//! ## 近似约定
//!
//! - 颜色：`#rgb` / `#rrggbb` / `#rrggbbaa` / `rgb()` / `rgba()` / 18 个常见色名
//!   （CSS 基础 16 色 + `grey` 别名 + `orange`）；
//! - font-size 基准 16px：`pt` × 96/72、`%` × 16/100；em/rem/关键词不在映射表，忽略；
//! - margin/padding 简写按 CSS TRouBLe 顺序展开 1–4 值；百分比槽位在
//!   [`Spacing`](crate::types::Spacing)（纯 px）中不可换算，原位记 0（已知限制）；
//! - 边框只保留宽度与颜色（非 solid 样式一律按 solid 近似，spec §3）；浏览器仅在
//!   border-style 生效时渲染边框，故宽度/颜色单独出现不构成边框；缺宽度取
//!   medium ≈ 3px，缺色按 currentColor 近似（本块 `color`，否则黑）；
//! - `margin`/`margin-left`/`margin-right` 含 `auto` → [`BlockStyle::margin_auto`]
//!   居中标记（spec §3"auto 视为居中"），auto 侧记 0，GPUI 渲染层映射水平居中；
//! - `max-width` 解析进 [`BlockStyle::max_width`]，由图片渲染层做宽度钳制。

use crate::types::{Align, BlockStyle, Border, InlineStyle, Rgba, Spacing, Width};

/// font-size 的 % 换算基准（裁定：基准 16px）。
const BASE_FONT_SIZE_PX: f32 = 16.0;

/// 解析颜色：`#rgb` / `#rrggbb` / `#rrggbbaa` / `rgb()` / `rgba()` / 常见色名。
///
/// 色名表为 CSS 基础 16 色 + `grey` 别名 + `orange`（见 [`NAMED_COLORS`]）；
/// `rgba()` 的 alpha 兼容 0–1 浮点（CSS 标准）与 0–255 遗留刻度；
/// `rgb()` 分量越界截断到 0–255。其余输入（em 关键字、`currentColor`、畸形值）返回 None。
pub fn parse_color(s: &str) -> Option<Rgba> {
    let lowered = s.trim().to_ascii_lowercase();
    if let Some(hex) = lowered.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(inner) = lowered.strip_prefix("rgb(") {
        return parse_rgb_args(inner.strip_suffix(')')?, false);
    }
    if let Some(inner) = lowered.strip_prefix("rgba(") {
        return parse_rgb_args(inner.strip_suffix(')')?, true);
    }
    NAMED_COLORS
        .iter()
        .find(|(name, _)| *name == lowered.as_str())
        .map(|(_, rgba)| *rgba)
}

/// 解析块级样式（spec §3 映射表的白名单子集）。
///
/// 映射表之外的属性静默忽略——含 `url()` 的属性（`background-image` 等）由此整体丢弃，
/// 这正是资源向量不放行的解析期机制。声明按 CSS 级联顺序处理，后声明覆盖先声明。
pub fn parse_block_style(css_text: &str) -> BlockStyle {
    let mut style = BlockStyle::default();
    // 边框按侧累积，物化于全部声明处理完后（currentColor 需要最终 color 值）。
    let mut widths: [Option<f32>; 4] = [None; 4];
    let mut colors: [Option<Rgba>; 4] = [None; 4];
    let mut styled = [false; 4];

    for (key, value) in declarations(css_text) {
        match key.as_str() {
            "background-color" => style.background_color = parse_color(&value),
            // 背景简写：仅提取纯色部分，url(...) 资源向量静默丢弃
            "background" => {
                for token in normalize_parens(&value).split_whitespace() {
                    let token = token.to_ascii_lowercase();
                    if token.starts_with("url(") {
                        continue;
                    }
                    if let Some(c) = parse_color(&token) {
                        style.background_color = Some(c);
                        break;
                    }
                }
            }
            "color" => style.color = parse_color(&value),
            "text-align" => style.text_align = parse_align(&value),
            "margin" => {
                if value
                    .split_whitespace()
                    .any(|t| t.eq_ignore_ascii_case("auto"))
                {
                    style.margin_auto = true;
                }
                style.margin = expand_spacing(&value);
            }
            "margin-top" => set_px(&mut style.margin[0], &value),
            "margin-right" => {
                // 左/右 auto 计入居中标记并归零该侧；上下 auto 无水平语义，不计
                if value.eq_ignore_ascii_case("auto") {
                    style.margin_auto = true;
                    style.margin[1] = 0.0;
                } else {
                    set_px(&mut style.margin[1], &value);
                }
            }
            "margin-bottom" => set_px(&mut style.margin[2], &value),
            "margin-left" => {
                if value.eq_ignore_ascii_case("auto") {
                    style.margin_auto = true;
                    style.margin[3] = 0.0;
                } else {
                    set_px(&mut style.margin[3], &value);
                }
            }
            "padding" => style.padding = expand_spacing(&value),
            "padding-top" => set_px(&mut style.padding[0], &value),
            "padding-right" => set_px(&mut style.padding[1], &value),
            "padding-bottom" => set_px(&mut style.padding[2], &value),
            "padding-left" => set_px(&mut style.padding[3], &value),
            "width" => style.width = parse_width(&value),
            "height" => style.height = parse_width(&value),
            "max-width" => style.max_width = parse_width(&value),
            "display" => {
                // 按声明出现顺序消费，后置覆盖前置（CSS 级联语义）：
                // display:none; display:block → 可见，none 标记可被撤销
                style.hidden = value.eq_ignore_ascii_case("none");
            }
            "border" => {
                let decl = parse_border_decl(&value);
                apply_border_decl(&mut widths, &mut colors, &mut styled, None, &decl);
            }
            "border-top" => {
                let decl = parse_border_decl(&value);
                apply_border_decl(&mut widths, &mut colors, &mut styled, Some(0), &decl);
            }
            "border-right" => {
                let decl = parse_border_decl(&value);
                apply_border_decl(&mut widths, &mut colors, &mut styled, Some(1), &decl);
            }
            "border-bottom" => {
                let decl = parse_border_decl(&value);
                apply_border_decl(&mut widths, &mut colors, &mut styled, Some(2), &decl);
            }
            "border-left" => {
                let decl = parse_border_decl(&value);
                apply_border_decl(&mut widths, &mut colors, &mut styled, Some(3), &decl);
            }
            "border-width" => widths = expand_trouble(&value, parse_border_width),
            "border-color" => colors = expand_trouble(&value, parse_color),
            "border-style" => {
                for (i, flag) in expand_trouble(&value, parse_border_style_token)
                    .into_iter()
                    .enumerate()
                {
                    styled[i] = flag.unwrap_or(false);
                }
            }
            _ => {
                // 边框长手 border-<side>-<width|color|style>；其余白名单外静默忽略
                if let Some((side, prop)) = key
                    .strip_prefix("border-")
                    .and_then(|rest| rest.rsplit_once('-'))
                {
                    let i = match side {
                        "top" => 0,
                        "right" => 1,
                        "bottom" => 2,
                        "left" => 3,
                        _ => continue, // 非边框长手（border-radius 等）→ 忽略
                    };
                    match prop {
                        "width" => {
                            if let Some(w) = parse_border_width(&value) {
                                widths[i] = Some(w);
                            }
                        }
                        "color" => {
                            if let Some(c) = parse_color(&value) {
                                colors[i] = Some(c);
                            }
                        }
                        "style" => styled[i] = parse_border_style_token(&value).unwrap_or(false),
                        _ => {}
                    }
                }
            }
        }
    }

    // 物化：浏览器仅在 border-style 生效时渲染边框；缺宽度取 CSS 初始值 medium ≈ 3px，
    // 缺色按 currentColor 近似（本块 color，否则黑）。
    let current_color = style.color.unwrap_or([0, 0, 0, 255]);
    let border = |width: Option<f32>, color: Option<Rgba>| Border {
        width: width.unwrap_or(3.0),
        color: color.unwrap_or(current_color),
    };
    style.border_top = styled[0].then(|| border(widths[0], colors[0]));
    style.border_right = styled[1].then(|| border(widths[1], colors[1]));
    style.border_bottom = styled[2].then(|| border(widths[2], colors[2]));
    style.border_left = styled[3].then(|| border(widths[3], colors[3]));
    style
}

/// 解析行内样式（富文本 run 的白名单子集，spec §3 映射表）。
///
/// font-size 以 16px 为基准换算 pt/%；font-weight 取 bold 或数值 ≥ 600；
/// font-family / line-height 等不在映射表，静默忽略。
pub fn parse_inline_style(css_text: &str) -> InlineStyle {
    let mut style = InlineStyle::default();
    for (key, value) in declarations(css_text) {
        match key.as_str() {
            "color" => style.color = parse_color(&value),
            "font-size" => style.font_size = parse_font_size(&value),
            "font-weight" => {
                if let Some(bold) = parse_font_weight(&value) {
                    style.bold = bold;
                }
            }
            "font-style" => {
                if matches!(value.to_ascii_lowercase().as_str(), "italic" | "oblique") {
                    style.italic = true;
                }
            }
            "text-decoration" => {
                let value = value.to_ascii_lowercase();
                if value.contains("underline") {
                    style.underline = true;
                }
                if value.contains("line-through") {
                    style.strikethrough = true;
                }
            }
            _ => {}
        }
    }
    style
}

/// 行内嵌套样式叠加：over 有值的 Option 字段覆盖，无值透传 base；
/// 布尔标记在嵌套中只能叠加、不能撤销（`<b><i>` 语义）。
pub fn merge_inline(base: &InlineStyle, over: &InlineStyle) -> InlineStyle {
    InlineStyle {
        color: over.color.or(base.color),
        font_size: over.font_size.or(base.font_size),
        bold: base.bold || over.bold,
        italic: base.italic || over.italic,
        underline: base.underline || over.underline,
        strikethrough: base.strikethrough || over.strikethrough,
    }
}

/// 常见色名表：CSS 基础 16 色 + `grey` 别名 + `orange`（覆盖裁定清单，不求穷尽）。
const NAMED_COLORS: &[(&str, Rgba)] = &[
    ("aqua", [0, 255, 255, 255]),
    ("black", [0, 0, 0, 255]),
    ("blue", [0, 0, 255, 255]),
    ("fuchsia", [255, 0, 255, 255]),
    ("gray", [128, 128, 128, 255]),
    ("green", [0, 128, 0, 255]),
    ("grey", [128, 128, 128, 255]),
    ("lime", [0, 255, 0, 255]),
    ("maroon", [128, 0, 0, 255]),
    ("navy", [0, 0, 128, 255]),
    ("olive", [128, 128, 0, 255]),
    ("orange", [255, 165, 0, 255]),
    ("purple", [128, 0, 128, 255]),
    ("red", [255, 0, 0, 255]),
    ("silver", [192, 192, 192, 255]),
    ("teal", [0, 128, 128, 255]),
    ("white", [255, 255, 255, 255]),
    ("yellow", [255, 255, 0, 255]),
];

/// 把规整 CSS 声明串切成 `(键(小写), 值(去空白与 !important))` 序列。
///
/// 声明以 `;` 分割、键值以首个 `:` 分割。`url(data:...;base64,...)` 中的 `;`
/// 会被一并切断——其碎片无 `:` 或落入未知键，最终同样被白名单丢弃（资源向量
/// 丢弃机制的容错来源，见模块文档）。
fn declarations(css_text: &str) -> impl Iterator<Item = (String, String)> + '_ {
    css_text.split(';').filter_map(|decl| {
        let (key, value) = decl.split_once(':')?;
        let key = key.trim().to_ascii_lowercase();
        let mut value = value.trim();
        // css-inline 产物无 !important；手写 style 属性可能带
        if let Some(stripped) = value.strip_suffix("!important") {
            value = stripped.trim_end();
        }
        Some((key, value.to_string()))
    })
}

/// 解析 `#` 后的十六进制：`rgb`（×17 位扩展）/`rrggbb`（alpha 255）/`rrggbbaa`。
fn parse_hex(hex: &str) -> Option<Rgba> {
    if !hex.is_ascii() {
        return None; // 防多字节字符按字节切片越界
    }
    match hex.len() {
        3 => {
            let digits: Option<Vec<u8>> = hex
                .chars()
                .map(|c| c.to_digit(16).map(|v| v as u8))
                .collect();
            let [r, g, b] = digits?.try_into().ok()?;
            Some([r * 17, g * 17, b * 17, 255])
        }
        6 | 8 => {
            let n = hex.len() / 2;
            let mut bytes = [0u8; 4];
            for (i, byte) in bytes.iter_mut().take(n).enumerate() {
                *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
            }
            let [r, g, b, a] = bytes;
            Some([r, g, b, if n == 3 { 255 } else { a }])
        }
        _ => None,
    }
}

/// `rgb()` / `rgba()` 参数：整数 0–255（越界截断）；alpha 兼容 0–1 浮点与 0–255 遗留刻度。
fn parse_rgb_args(inner: &str, has_alpha: bool) -> Option<Rgba> {
    let values: Vec<f32> = inner
        .split(',')
        .map(|part| part.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    if values.len() != 3 && !(has_alpha && values.len() == 4) {
        return None;
    }
    let byte = |v: f32| v.clamp(0.0, 255.0).round() as u8;
    let alpha = match values.get(3) {
        Some(&raw) => {
            let scaled = if raw > 1.0 { raw } else { raw * 255.0 };
            byte(scaled)
        }
        None => 255,
    };
    Some([byte(values[0]), byte(values[1]), byte(values[2]), alpha])
}

/// 间距/边框宽度的长度：`px` 或无单位数值（0 等，表格属性风格）；`%`/`em`/`auto` 不适配。
fn parse_px(s: &str) -> Option<f32> {
    let lowered = s.trim().to_ascii_lowercase();
    match lowered.strip_suffix("px") {
        Some(v) => v.trim().parse::<f32>().ok(),
        None => lowered.parse::<f32>().ok(),
    }
}

/// 尺寸：`px` / `%` / 无单位数值；`auto` 视为未声明（None）；非有限/负值拒绝
/// （终审 M-1：与属性路径 `parse_dim_attr` 的 `finite_nonneg` 同语义——
/// `f32::parse` 接受 "NaN"/"inf"，此类值不得经 width/height/max_width 进入
/// IR 直达 GPUI 布局）。
fn parse_width(s: &str) -> Option<Width> {
    let finite_nonneg = |v: &f32| v.is_finite() && *v >= 0.0;
    let lowered = s.trim().to_ascii_lowercase();
    if lowered == "auto" {
        return None;
    }
    if let Some(v) = lowered.strip_suffix('%') {
        return v
            .trim()
            .parse::<f32>()
            .ok()
            .filter(finite_nonneg)
            .map(Width::Percent);
    }
    parse_px(&lowered).filter(finite_nonneg).map(Width::Px)
}

/// font-size：基准 16px（pt × 96/72、% × 16/100）；em/rem/关键词/非正值忽略。
fn parse_font_size(s: &str) -> Option<f32> {
    let lowered = s.trim().to_ascii_lowercase();
    let px = if let Some(v) = lowered.strip_suffix('%') {
        v.trim().parse::<f32>().ok()? * BASE_FONT_SIZE_PX / 100.0
    } else if let Some(v) = lowered.strip_suffix("pt") {
        v.trim().parse::<f32>().ok()? * 96.0 / 72.0
    } else {
        lowered
            .strip_suffix("px")
            .and_then(|v| v.trim().parse::<f32>().ok())?
    };
    (px > 0.0).then_some(px)
}

/// font-weight：`bold` → true，`normal` → false，数值 ≥ 600 → true，其余（bolder 等）忽略。
fn parse_font_weight(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "bold" => Some(true),
        "normal" => Some(false),
        other => other.parse::<f32>().ok().map(|weight| weight >= 600.0),
    }
}

/// 水平对齐关键词（spec §3：left/center/right/justify）。
///
/// `pub(crate)`：`text-align` 样式值与 `align` 属性值同一关键词集，builder 的
/// align 属性解析（T5）复用本表，避免双源。
pub(crate) fn parse_align(s: &str) -> Option<Align> {
    match s.trim().to_ascii_lowercase().as_str() {
        "left" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" => Some(Align::Right),
        "justify" => Some(Align::Justify),
        _ => None,
    }
}

/// 取 style 串中最后一条 `display` 声明的值（小写、去空白；未声明 → None）。
///
/// `pub(crate)`：builder 的行内系 display 判定（线上反馈③：邮件横向菜单的
/// 主流写法是块级标签 + `display:inline-block`）复用声明切分逻辑，与
/// [`parse_block_style`] 的 hidden 判定同源（后声明覆盖前置）。
pub(crate) fn display_value(css_text: &str) -> Option<String> {
    let mut last = None;
    for (key, value) in declarations(css_text) {
        if key == "display" {
            last = Some(value.to_ascii_lowercase());
        }
    }
    last
}

/// display 值是否为行内系（inline / inline-block / inline-flex）：元素按
/// 行内透传参与父级行内流（不产块）。
pub(crate) fn is_inline_display(css_text: &str) -> bool {
    display_value(css_text).is_some_and(|v| {
        matches!(v.as_str(), "inline" | "inline-block" | "inline-flex" | "inline-table")
    })
}

/// 边框宽度：px / 无单位 / 关键字（thin=1、medium=3、thick=5）。
fn parse_border_width(s: &str) -> Option<f32> {
    match s.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        other => parse_px(other),
    }
}

/// solid 系样式关键字（非 solid 一律按 solid 近似，spec §3）；none/hidden/auto → false。
fn parse_border_style_token(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        "none" | "hidden" | "auto" => Some(false),
        _ => None,
    }
}

/// `border[-<side>]` 简写值的解析产物。
#[derive(Default)]
struct BorderDecl {
    /// 宽度（None → 物化期取 medium ≈ 3px）。
    width: Option<f32>,
    /// 颜色（None → 物化期取 currentColor）。
    color: Option<Rgba>,
    /// 出现了边框样式关键字（宽度/颜色单独出现不构成可见边框）。
    styled: bool,
    /// `none`/`hidden`：整体清除该范围边框。
    none: bool,
}

/// 分词解析 `border` 简写值：宽度、颜色、样式关键字；括号内空白先规整。
fn parse_border_decl(value: &str) -> BorderDecl {
    let mut decl = BorderDecl::default();
    for token in normalize_parens(value).split_whitespace() {
        match token.to_ascii_lowercase().as_str() {
            "none" | "hidden" => decl.none = true,
            "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
                decl.styled = true;
            }
            _ => {
                // 宽度 token 交给 parse_color 必然失败，无需短路
                if decl.width.is_none() {
                    decl.width = parse_border_width(token);
                }
                if decl.color.is_none() {
                    decl.color = parse_color(token);
                }
            }
        }
    }
    decl
}

/// 把简写产物按级联写入侧状态；`side = None` 为全边。`none` 复位该侧（CSS 清除语义）。
fn apply_border_decl(
    widths: &mut [Option<f32>; 4],
    colors: &mut [Option<Rgba>; 4],
    styled: &mut [bool; 4],
    side: Option<usize>,
    decl: &BorderDecl,
) {
    let sides: &[usize] = match side {
        Some(i) => &[i],
        None => &[0, 1, 2, 3],
    };
    for &i in sides {
        if decl.none {
            widths[i] = None;
            colors[i] = None;
            styled[i] = false;
        } else {
            if let Some(w) = decl.width {
                widths[i] = Some(w);
            }
            if let Some(c) = decl.color {
                colors[i] = Some(c);
            }
            if decl.styled {
                styled[i] = true;
            }
        }
    }
}

/// TRouBLe 顺序 1–4 值展开：`[上, 右, 下, 左]`。无法解析的 token 跳过（宽容降级）。
fn expand_trouble<T: Copy>(value: &str, parse: impl Fn(&str) -> Option<T>) -> [Option<T>; 4] {
    let values: Vec<T> = value.split_whitespace().filter_map(parse).collect();
    match values.as_slice() {
        [a] => [Some(*a); 4],
        [a, b] => [*a, *b, *a, *b].map(Some),
        [a, b, c] => [*a, *b, *c, *b].map(Some),
        [a, b, c, d, ..] => [*a, *b, *c, *d].map(Some),
        _ => [None; 4],
    }
}

/// 间距展开（[`Spacing`] 为纯 px）：`auto` 记 0（居中由 `margin_auto` 承载）；
/// 百分比槽位**原位记 0**（无布局上下文不可换算，但占住 TRouBLe 位置——裁定：
/// `margin:10px 5%` → `[10,0,10,0]` 而非复制首个有效值）；em 等其余不可解析
/// token 跳过（宽容降级，其余值照常 TRouBLe 展开，已知限制）。
fn expand_spacing(value: &str) -> Spacing {
    expand_trouble(value, |token| {
        if token.eq_ignore_ascii_case("auto") || token.ends_with('%') {
            Some(0.0)
        } else {
            parse_px(token)
        }
    })
    .map(|v| v.unwrap_or(0.0))
}

/// 单边 px 赋值（值不可解析时保持原值）。
fn set_px(slot: &mut f32, value: &str) {
    if let Some(px) = parse_px(value) {
        *slot = px;
    }
}

/// 去除括号内空白（`rgb(221, 221, 221)` → `rgb(221,221,221)`），供按空白分词的简写解析。
fn normalize_parens(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                out.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                out.push(c);
            }
            ' ' | '\t' | '\n' | '\r' if depth > 0 => {}
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Align, ImageStyle, Width};

    // ---- brief 测试（逐字采用，格式按 rustfmt 展开）----

    #[test]
    fn colors() {
        assert_eq!(parse_color("#f00"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("#ff000080"), Some([255, 0, 0, 128]));
        assert_eq!(parse_color("red"), Some([255, 0, 0, 255]));
        assert_eq!(parse_color("rgb(1,2,3)"), Some([1, 2, 3, 255]));
    }

    #[test]
    fn margin_shorthand_expands() {
        let s = parse_block_style("margin: 10px 5px");
        assert_eq!(s.margin, [10.0, 5.0, 10.0, 5.0]);
    }

    #[test]
    fn inline_merge_overrides_partial() {
        // base 红色 + over bold → 红 + 粗；over 无色 → 保持 base
        let base = parse_inline_style("color: red");
        let over = parse_inline_style("font-weight: bold");
        let merged = merge_inline(&base, &over);
        assert_eq!(merged.color, Some([255, 0, 0, 255]));
        assert!(merged.bold);
        let empty = parse_inline_style("");
        let merged = merge_inline(&base, &empty);
        assert_eq!(merged.color, Some([255, 0, 0, 255]));
    }

    // ---- 补充表驱动测试 ----

    #[test]
    fn named_color_table() {
        const CASES: &[(&str, Rgba)] = &[
            ("aqua", [0, 255, 255, 255]),
            ("black", [0, 0, 0, 255]),
            ("blue", [0, 0, 255, 255]),
            ("fuchsia", [255, 0, 255, 255]),
            ("gray", [128, 128, 128, 255]),
            ("green", [0, 128, 0, 255]),
            ("grey", [128, 128, 128, 255]),
            ("lime", [0, 255, 0, 255]),
            ("maroon", [128, 0, 0, 255]),
            ("navy", [0, 0, 128, 255]),
            ("olive", [128, 128, 0, 255]),
            ("orange", [255, 165, 0, 255]),
            ("purple", [128, 0, 128, 255]),
            ("red", [255, 0, 0, 255]),
            ("silver", [192, 192, 192, 255]),
            ("teal", [0, 128, 128, 255]),
            ("white", [255, 255, 255, 255]),
            ("yellow", [255, 255, 0, 255]),
        ];
        assert!(CASES.len() >= 16);
        for (name, expected) in CASES {
            assert_eq!(parse_color(name), Some(*expected), "color {name}");
            assert_eq!(
                parse_color(&name.to_ascii_uppercase()),
                Some(*expected),
                "COLOR {name}"
            );
        }
        // 非法输入
        assert_eq!(parse_color(""), None);
        assert_eq!(parse_color("#12"), None);
        assert_eq!(parse_color("#gggggg"), None);
        assert_eq!(parse_color("notacolor"), None);
    }

    #[test]
    fn rgb_rgba_and_clamping() {
        assert_eq!(parse_color("rgb(1, 2, 3)"), Some([1, 2, 3, 255]));
        assert_eq!(parse_color("RGBA(255, 0, 0, 0.5)"), Some([255, 0, 0, 128]));
        assert_eq!(parse_color("rgba(0, 0, 0, 1)"), Some([0, 0, 0, 255]));
        // 0–255 遗留 alpha 刻度容错
        assert_eq!(parse_color("rgba(1,2,3,128)"), Some([1, 2, 3, 128]));
        // 分量越界截断到 0–255
        assert_eq!(parse_color("rgb(300, -5, 3)"), Some([255, 0, 3, 255]));
        // 分量数不符
        assert_eq!(parse_color("rgb(1,2)"), None);
        assert_eq!(parse_color("rgb(1,2,3,4)"), None);
    }

    #[test]
    fn spacing_trouble_expansion() {
        let s = parse_block_style("padding: 4px");
        assert_eq!(s.padding, [4.0; 4]);
        let s = parse_block_style("padding: 1px 2px");
        assert_eq!(s.padding, [1.0, 2.0, 1.0, 2.0]);
        let s = parse_block_style("padding: 1px 2px 3px");
        assert_eq!(s.padding, [1.0, 2.0, 3.0, 2.0]);
        let s = parse_block_style("padding: 1px 2px 3px 4px");
        assert_eq!(s.padding, [1.0, 2.0, 3.0, 4.0]);
        // 单边长手
        let s = parse_block_style("margin-top: 8px; margin-left: 2px");
        assert_eq!(s.margin, [8.0, 0.0, 0.0, 2.0]);
    }

    #[test]
    fn spacing_percent_slots_zeroed_in_place() {
        // 混合 px+% 槽位：百分比原位记 0、占住 TRouBLe 位置（裁定：不再复制首个有效值）
        const CASES: &[(&str, [f32; 4])] = &[
            ("10px 5%", [10.0, 0.0, 10.0, 0.0]),
            ("1% 2px 3% 4px", [0.0, 2.0, 0.0, 4.0]),
            ("5%", [0.0; 4]),
            ("10px 20px 30% 40px", [10.0, 20.0, 0.0, 40.0]),
            ("2px 4% 6px", [2.0, 0.0, 6.0, 0.0]), // 三值展开 [a,b,c,b]
            ("1em 2px", [2.0, 2.0, 2.0, 2.0]),    // em 跳过 → 单值复制（已知限制）
            ("10em", [0.0; 4]),                   // 全部跳过 → 全 0
            ("10px auto", [10.0, 0.0, 10.0, 0.0]),
        ];
        for (value, want) in CASES {
            let css = format!("margin: {value}");
            assert_eq!(parse_block_style(&css).margin, *want, "{css}");
        }
        // padding 同语义
        assert_eq!(
            parse_block_style("padding: 10% 5px").padding,
            [0.0, 5.0, 0.0, 5.0]
        );
    }

    #[test]
    fn display_none_is_overridden_by_later_declaration() {
        // 后置声明覆盖前置标记（按出现顺序消费，CSS 级联语义）
        assert!(parse_block_style("display: none").hidden);
        assert!(!parse_block_style("display: none; display: block").hidden);
        assert!(!parse_block_style("display: none; display: flex").hidden);
        assert!(parse_block_style("display: block; display: none").hidden);
        // 非 none 取值不构成隐藏
        assert!(!parse_block_style("display: block").hidden);
        assert!(!parse_block_style("display: grid").hidden);
    }

    #[test]
    fn inline_display_detection() {
        // 线上反馈③：块级标签 + display:inline-block 的横向菜单写法
        assert!(is_inline_display("display:inline-block"));
        assert!(is_inline_display("DISPLAY: Inline-Block ; color:red"));
        assert!(is_inline_display("display:inline"));
        assert!(is_inline_display("display:inline-flex"));
        // 后声明覆盖（block 覆盖 inline-block → 不再行内）
        assert!(!is_inline_display("display:inline-block; display:block"));
        // 块级/未声明/none 均不构成行内
        assert!(!is_inline_display("display:block"));
        assert!(!is_inline_display("display:none"));
        assert!(!is_inline_display("color:red"));
        assert!(!is_inline_display(""));
        // display_value：最后一条声明胜出、小写化
        assert_eq!(
            display_value("color:red; display:FLEX"),
            Some("flex".to_string())
        );
        assert_eq!(display_value("color:red"), None);
    }

    #[test]
    fn margin_auto_detection() {
        // （css, margin_auto, margin）——auto 侧记 0，其余 px 值照常解析
        const CASES: &[(&str, bool, [f32; 4])] = &[
            ("margin: 0 auto", true, [0.0; 4]),
            ("margin: auto", true, [0.0; 4]),
            ("margin: 10px auto", true, [10.0, 0.0, 10.0, 0.0]),
            ("margin-left: auto", true, [0.0; 4]),
            ("margin-right: auto", true, [0.0; 4]),
            ("margin: 10px", false, [10.0; 4]),
            ("margin: 0", false, [0.0; 4]),
            ("margin-top: auto", false, [0.0; 4]), // 上下 auto 无水平居中语义
        ];
        for (css, want_auto, want_margin) in CASES {
            let s = parse_block_style(css);
            assert_eq!(s.margin_auto, *want_auto, "margin_auto {css}");
            assert_eq!(s.margin, *want_margin, "margin {css}");
        }
        // 长手 auto 覆盖先前 px（该侧归零），其余侧保留
        let s = parse_block_style("margin: 10px; margin-left: auto");
        assert!(s.margin_auto);
        assert_eq!(s.margin, [10.0, 10.0, 10.0, 0.0]);
    }

    #[test]
    fn block_style_whitelist() {
        let s = parse_block_style(
            "background-color: #eee; COLOR: Red; text-align: center; width: 600px; height: 50%; display: none",
        );
        assert_eq!(s.background_color, Some([238, 238, 238, 255]));
        assert_eq!(s.color, Some([255, 0, 0, 255]));
        assert_eq!(s.text_align, Some(Align::Center));
        assert_eq!(s.width, Some(Width::Px(600.0)));
        assert_eq!(s.height, Some(Width::Percent(50.0)));
        assert!(s.hidden);
        // background 简写提取纯色、url 丢弃；无单位数值；auto
        let s = parse_block_style("background: url(https://x/y.png) silver; width: 0");
        assert_eq!(s.background_color, Some([192, 192, 192, 255]));
        assert_eq!(s.width, Some(Width::Px(0.0)));
        let s = parse_block_style("height: auto; text-align: right; text-align: justify");
        assert_eq!(s.height, None);
        assert_eq!(s.text_align, Some(Align::Justify)); // 后声明覆盖
    }

    #[test]
    fn max_width_parsing() {
        const CASES: &[(&str, Option<Width>)] = &[
            ("max-width: 600px", Some(Width::Px(600.0))),
            ("max-width: 100%", Some(Width::Percent(100.0))),
            ("max-width: 0", Some(Width::Px(0.0))),
            ("max-width: auto", None),
            ("max-width: 10em", None), // em 不在映射表
        ];
        for (css, want) in CASES {
            assert_eq!(parse_block_style(css).max_width, *want, "{css}");
        }
        // 与 width 并存互不干扰；ImageStyle 同步获得字段（图片渲染层消费）
        let s = parse_block_style("width: 100%; max-width: 600px");
        assert_eq!(s.width, Some(Width::Percent(100.0)));
        assert_eq!(s.max_width, Some(Width::Px(600.0)));
        assert_eq!(ImageStyle::default().max_width, None);
    }

    #[test]
    fn width_rejects_non_finite_and_negative() {
        // 终审 M-1：样式路径与属性路径（parse_dim_attr 的 finite_nonneg）同
        // 护栏——`f32::parse` 接受 "NaN"/"inf"，此类值与负值不得经
        // width/height/max_width 进入 IR 直达 GPUI 布局
        const CASES: &[(&str, Option<Width>)] = &[
            ("width: NaNpx", None),
            ("width: NaN", None),
            ("width: infpx", None),
            ("width: -5px", None),
            ("width: -5%", None),
            ("width: -5", None),
            // 语义对照：正常值与 0（有限非负下界）不受护栏影响
            ("width: 600px", Some(Width::Px(600.0))),
            ("width: 0", Some(Width::Px(0.0))),
            ("width: 100%", Some(Width::Percent(100.0))),
        ];
        for (css, want) in CASES {
            assert_eq!(parse_block_style(css).width, *want, "{css}");
        }
        // height/max-width 同一解析入口
        assert_eq!(parse_block_style("height: NaNpx").height, None);
        assert_eq!(parse_block_style("height: -5px").height, None);
        assert_eq!(parse_block_style("max-width: infpx").max_width, None);
        assert_eq!(parse_block_style("max-width: -5%").max_width, None);
    }

    #[test]
    fn border_parsing() {
        // 全边简写
        let s = parse_block_style("border: 1px solid #ddd");
        let expected = Border {
            width: 1.0,
            color: [221, 221, 221, 255],
        };
        assert_eq!(s.border_top, Some(expected));
        assert_eq!(s.border_right, Some(expected));
        assert_eq!(s.border_bottom, Some(expected));
        assert_eq!(s.border_left, Some(expected));

        // 单边简写 + 非 solid 样式按 solid 近似
        let s = parse_block_style("border-bottom: 2px dashed red");
        assert_eq!(
            s.border_bottom,
            Some(Border {
                width: 2.0,
                color: [255, 0, 0, 255]
            })
        );
        assert_eq!(s.border_top, None);

        // border-width/border-color TRouBLe 展开（border-style 置边框存在）
        let s = parse_block_style(
            "border-style: solid; border-width: 0 0 1px 0; border-color: red blue",
        );
        assert_eq!(
            s.border_top,
            Some(Border {
                width: 0.0,
                color: [255, 0, 0, 255]
            })
        );
        assert_eq!(
            s.border_right,
            Some(Border {
                width: 0.0,
                color: [0, 0, 255, 255]
            })
        );
        assert_eq!(
            s.border_bottom,
            Some(Border {
                width: 1.0,
                color: [255, 0, 0, 255]
            })
        );
        assert_eq!(
            s.border_left,
            Some(Border {
                width: 0.0,
                color: [0, 0, 255, 255]
            })
        );

        // 缺宽度按 medium ≈ 3px；缺色按 currentColor 近似（本块 color，否则黑）
        let s = parse_block_style("border-top: solid red");
        assert_eq!(
            s.border_top,
            Some(Border {
                width: 3.0,
                color: [255, 0, 0, 255]
            })
        );
        let s = parse_block_style("border: solid");
        assert_eq!(
            s.border_top,
            Some(Border {
                width: 3.0,
                color: [0, 0, 0, 255]
            })
        );

        // 仅宽度/仅颜色（无样式）→ 浏览器不渲染边框 → 忽略
        let s = parse_block_style("border-width: 1px; border-color: red");
        assert_eq!(s.border_top, None);

        // border:none 清除，后置单边样式恢复
        let s = parse_block_style("border: 1px solid red; border: none; border-top-style: solid");
        assert_eq!(
            s.border_top,
            Some(Border {
                width: 3.0,
                color: [0, 0, 0, 255]
            })
        );
        assert_eq!(s.border_left, None);

        // 边框长手 + currentColor 近似（取本块 color）
        let s = parse_block_style("color: green; border-left-style: solid; border-left-width: 3px");
        assert_eq!(
            s.border_left,
            Some(Border {
                width: 3.0,
                color: [0, 128, 0, 255]
            })
        );

        // 简写中带空格的 rgb() 仍可解析
        let s = parse_block_style("border: 1px solid rgb(221, 221, 221)");
        assert_eq!(
            s.border_top,
            Some(Border {
                width: 1.0,
                color: [221, 221, 221, 255]
            })
        );
    }

    #[test]
    fn inline_style_font_mapping() {
        let s = parse_inline_style(
            "color: #333; font-size: 12pt; font-weight: bold; font-style: italic; text-decoration: underline",
        );
        assert_eq!(s.color, Some([51, 51, 51, 255]));
        assert_eq!(s.font_size, Some(16.0)); // 12pt × 96/72 = 16px
        assert!(s.bold && s.italic && s.underline);
        assert!(!s.strikethrough);

        let s = parse_inline_style(
            "font-size: 120%; font-weight: 700; text-decoration: line-through underline",
        );
        assert_eq!(s.font_size, Some(19.2)); // 120% × 16/100
        assert!(s.bold);
        assert!(s.underline && s.strikethrough);

        // 数值 < 600 非粗体；line-height/font-family 等静默忽略
        let s = parse_inline_style("font-weight: 400; line-height: 1.5; font-family: Arial");
        assert!(!s.bold);
    }

    #[test]
    fn inline_font_size_units() {
        const CASES: &[(&str, Option<f32>)] = &[
            ("16px", Some(16.0)),
            ("12pt", Some(16.0)),
            ("100%", Some(16.0)),
            ("14.5px", Some(14.5)),
            ("1.5em", None),  // em 相对父级，不在映射表
            ("medium", None), // 关键词忽略
            ("-3px", None),   // 非正字号忽略
            ("", None),
        ];
        for (input, expected) in CASES {
            let s = parse_inline_style(&format!("font-size: {input}"));
            assert_eq!(s.font_size, *expected, "font-size {input}");
        }
    }

    #[test]
    fn inline_merge_passthrough_and_accumulate() {
        let base = parse_inline_style("color: red; font-size: 14px; font-style: italic");
        let over = parse_inline_style("font-size: 16px; text-decoration: underline");
        let m = merge_inline(&base, &over);
        assert_eq!(m.color, Some([255, 0, 0, 255])); // over 无色 → 透传 base
        assert_eq!(m.font_size, Some(16.0)); // over 有值 → 覆盖
        assert!(m.italic && m.underline && !m.bold && !m.strikethrough);
        // 空叠加不改双方
        let none = InlineStyle::default();
        assert_eq!(merge_inline(&base, &none), base);
        assert_eq!(merge_inline(&none, &base), base);
    }

    #[test]
    fn url_resource_vectors_are_dropped() {
        // background-image 等含 url() 的属性不在白名单 → 静默忽略（资源向量丢弃机制）
        let s = parse_block_style(
            "color: blue; background-image: url(https://evil.example/track.png); list-style-image: url(2.png)",
        );
        assert_eq!(s.color, Some([0, 0, 255, 255]));
        assert_eq!(s.background_color, None);
        // data: URI 内含 ';'，声明被切断后颜色碎片丢失——资源向量优先丢弃
        let s = parse_block_style("background: url(data:image/png;base64,iVBORw0KGgo=) red");
        assert_eq!(s.background_color, None);
    }

    #[test]
    fn unknown_properties_silently_ignored() {
        let s = parse_block_style(
            "position: fixed; float: left; font-family: Arial; line-height: 1.5; border-radius: 4px; -moz-border-radius: 4px",
        );
        assert_eq!(s, BlockStyle::default());
        let s = parse_inline_style(
            "letter-spacing: 2px; font-family: serif; background-color: red; text-transform: uppercase",
        );
        assert_eq!(s, InlineStyle::default());
    }

    #[test]
    fn important_suffix_stripped() {
        let s = parse_block_style("color: red !important; margin-top: 10px!important");
        assert_eq!(s.color, Some([255, 0, 0, 255]));
        assert_eq!(s.margin[0], 10.0);
    }

    #[test]
    fn empty_and_malformed_css_yield_defaults() {
        assert_eq!(parse_block_style(""), BlockStyle::default());
        assert_eq!(
            parse_block_style("   ; : ; background "),
            BlockStyle::default()
        );
        assert_eq!(parse_inline_style("::"), InlineStyle::default());
        assert_eq!(parse_inline_style("color"), InlineStyle::default());
    }
}
