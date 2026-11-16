//!
//! # 中间表示类型（Intermediate Representation）
//!
//! 定义 HTML 邮件解析后的中间表示（spec §3），与 GPUI 类型解耦：
//! GPUI 渲染层（postium-ui）只消费本模块的类型，不再接触原始 HTML。
//!
//! ## 约定
//!
//! - 颜色统一为 [`Rgba`]（RGBA 四元组）；
//! - 尺寸统一为 [`Width`]（px / 百分比）；
//! - 间距统一为 [`Spacing`]（上/右/下/左，CSS TRouBLe 顺序，单位 px）；
//! - `display:none` 的剪枝在树构建期完成，[`BlockStyle::hidden`] 仅为解析期标记；
//! - 所有类型均可 `Serialize`（Task 8 JSON 快照测试依赖）。

use std::sync::Arc;

use serde::Serialize;

/// RGBA 颜色：`[r, g, b, a]`，各分量 0–255。
pub type Rgba = [u8; 4];

/// 四向间距（px）：`[上, 右, 下, 左]`，遵循 CSS margin/padding 简写展开顺序。
pub type Spacing = [f32; 4];

/// 尺寸：绝对像素或百分比（相对父容器）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Width {
    /// 固定像素值。
    Px(f32),
    /// 百分比（0–100+）。
    Percent(f32),
}

/// 水平对齐方式（text-align / align 属性）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Align {
    /// 左对齐（默认）。
    Left,
    /// 居中（含 `align="center"`、`<center>` 与 `text-align:center`）。
    Center,
    /// 右对齐。
    Right,
    /// 两端对齐。
    Justify,
}

/// 单边边框：宽度 + 颜色（样式仅接受 solid 系，非 solid 按 solid 近似，spec §3 映射表）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Border {
    /// 边框宽度（px）。
    pub width: f32,
    /// 边框颜色。
    pub color: Rgba,
}

/// 块级样式（容器/段落/表格单元等共用的样式子集，spec §3 映射表）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BlockStyle {
    /// 背景色（`background-color`）。
    pub background_color: Option<Rgba>,
    /// 前景文字色（`color`，可继承给行内 run）。
    pub color: Option<Rgba>,
    /// 水平对齐（`text-align`）。
    pub text_align: Option<Align>,
    /// 外边距（px）。
    pub margin: Spacing,
    /// `margin` / `margin-left` / `margin-right` 含 `auto`：水平居中标记
    /// （spec §3"auto 视为居中"，GPUI 渲染层映射）；auto 侧在 [`BlockStyle::margin`] 记 0。
    pub margin_auto: bool,
    /// 内边距（px）。
    pub padding: Spacing,
    /// 宽度（px / %）。
    pub width: Option<Width>,
    /// 高度（px / %）。
    pub height: Option<Width>,
    /// 最大宽度（px / %）：渲染层宽度钳制上限（响应式邮件高频属性）。
    pub max_width: Option<Width>,
    /// 上边框。
    pub border_top: Option<Border>,
    /// 右边框。
    pub border_right: Option<Border>,
    /// 下边框。
    pub border_bottom: Option<Border>,
    /// 左边框。
    pub border_left: Option<Border>,
    /// `display:none` 标记：树构建期据此剪枝（Task 4）。
    pub hidden: bool,
}

/// 行内样式（富文本 run 的样式子集，spec §3 映射表）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct InlineStyle {
    /// 文字颜色。
    pub color: Option<Rgba>,
    /// 字号（px；pt/% 解析期换算，基准 16px）。
    pub font_size: Option<f32>,
    /// 粗体（font-weight: bold 或数值 ≥ 600）。
    pub bold: bool,
    /// 斜体（font-style: italic）。
    pub italic: bool,
    /// 下划线（text-decoration 含 underline）。
    pub underline: bool,
    /// 删除线（text-decoration 含 line-through）。
    pub strikethrough: bool,
}

/// 图片样式：仅宽高（spec §3）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ImageStyle {
    /// 宽度（px / %），占位框尽量采用。
    pub width: Option<Width>,
    /// 高度（px / %）。
    pub height: Option<Width>,
    /// 最大宽度（px / %）：图片渲染宽度钳制上限。
    pub max_width: Option<Width>,
}

/// 解析完成的邮件正文文档：块级树的根。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct HtmlDocument {
    /// 顶层块序列。
    pub blocks: Vec<Block>,
}

/// 块级元素（布局以块为单位，flex column 排布）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Block {
    /// 通用容器（div / center 及未知标签的宽容降级，spec §3）。
    Container {
        /// 容器样式。
        style: BlockStyle,
        /// 子块序列。
        children: Vec<Block>,
    },
    /// 段落（p 及裸文本聚合）。
    Paragraph {
        /// 段落样式。
        style: BlockStyle,
        /// 行内 run 序列。
        runs: Vec<InlineRun>,
        /// 对齐方式。
        align: Align,
    },
    /// 标题（h1–h6）。
    Heading {
        /// 级别 1–6。
        level: u8,
        /// 对齐方式（`text-align` 样式优先、`align` 属性兜底；T5 增补——
        /// T4 评审圈定的类型认领空洞，`<h2 align="center">` 不再丢对齐）。
        align: Align,
        /// 行内 run 序列。
        runs: Vec<InlineRun>,
    },
    /// 列表（ul / ol）。
    List {
        /// 是否有序。
        ordered: bool,
        /// 每个列表项是一组块。
        items: Vec<Vec<Block>>,
    },
    /// 表格（嵌套 flex 布局渲染，spec §5）。
    Table {
        /// 表格样式。
        style: BlockStyle,
        /// 行序列。
        rows: Vec<TableRow>,
    },
    /// 引用块（blockquote）。
    Quote {
        /// 子块序列。
        children: Vec<Block>,
    },
    /// 块级图片（直构文档/兼容保留变体；builder 产出的行内图片保留在
    /// 段落 runs 内——`InlineRun::Image`，渲染层 wrap-flow 横向消费，
    /// 线上反馈③第二波：块级提升会把图文混排的横向菜单拆成竖排）。
    Image {
        /// 图片来源（终审白名单后）。
        src: ImageRef,
        /// 替代文字。
        alt: String,
        /// 图片样式。
        style: ImageStyle,
    },
    /// 水平分隔线（hr）。
    Hr,
}

/// 表格行。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableRow {
    /// 单元格序列。
    pub cells: Vec<TableCell>,
}

/// 表格单元格。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TableCell {
    /// 单元格内的块序列（可嵌套表格）。
    pub blocks: Vec<Block>,
    /// 跨列数（colspan，默认 1，上限 16）。
    pub colspan: u8,
    /// 跨行数（rowspan，默认 1，上限 16；线上反馈②）。树构建期的网格
    /// 归一化消费它生成占位单元格对齐列位；渲染层不拉伸（span 单元格
    /// 只在起始行渲染，内容不跨行垂直居中——已知近似）。
    pub rowspan: u8,
    /// 单元格宽度（width 属性 / style，转 flex-basis）。
    pub width: Option<Width>,
    /// 单元格样式。
    pub style: BlockStyle,
}

/// 行内 run（富文本片段；行内嵌套已扁平化为 run 序列，样式叠加）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum InlineRun {
    /// 纯文本片段。
    Text {
        /// 文本内容。
        text: String,
        /// 叠加后的行内样式。
        style: InlineStyle,
    },
    /// 链接（href 经终审白名单；不合格链接降级为纯文本，链接本身不保留）。
    Link {
        /// 安全链接。
        href: SafeUrl,
        /// 链接内 run 序列。
        runs: Vec<InlineRun>,
    },
    /// 行内图片（图文同段，渲染层 wrap-flow 横向消费——线上反馈③第二波，
    /// 不再提升为块级拆散混排流）。
    Image {
        /// 图片来源（终审白名单后）。
        src: ImageRef,
        /// 替代文字。
        alt: String,
        /// 图片样式。
        style: ImageStyle,
    },
    /// 换行（br）。
    Br,
}

/// 图片来源（树构建期终审白名单，spec §4.4）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ImageRef {
    /// 远程图片（http/https；UI 决定是否加载，默认拦截）。
    Remote {
        /// 图片 URL。
        url: String,
    },
    /// 内嵌附件（`cid:` 引用，始终显示）。
    Cid {
        /// Content-ID（小写化）。
        content_id: String,
    },
    /// Data URI 图片（仅 `data:image/*`，解码后的字节）。
    DataUri {
        /// MIME 子类型（如 `image/png`）。
        mime: String,
        /// 解码后的图片字节。
        bytes: Arc<[u8]>,
    },
    /// 非法来源（协议不在白名单，如 `data:text`）：渲染为占位框，plan Task 5。
    Invalid,
}

/// 安全链接（树构建期校验；`javascript:` / `data:` / 其他协议一律丢弃链接、保留文字，spec §4.4）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SafeUrl {
    /// http/https 链接（系统浏览器打开）。
    Http(String),
    /// mailto 链接（系统邮件客户端处理）。
    Mailto(String),
}

/// 管线输入限制（spec §8：超限走纯文本降级）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    /// 原始 HTML 最大字节数。
    pub max_bytes: usize,
    /// 解析树最大节点数。
    pub max_nodes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 1024 * 1024,
            max_nodes: 20_000,
        }
    }
}
