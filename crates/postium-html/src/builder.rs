//!
//! # 文档树构建——块级结构 + 行内扁平化（spec §3，plan Task 4/5）
//!
//! [`build_document`] 是渲染管线的最后一环：把内联 + 净化后的 HTML 解析为
//! [`HtmlDocument`](crate::types::HtmlDocument) 中间表示。
//!
//! ## 处理步骤
//!
//! 1. html5ever + rcdom 宽松解析（解析错误逐个恢复，不整体失败）；
//! 2. 内容提取：css-inline 会把片段补全为完整文档，而 html5ever 对裸片段同样
//!    补全 html/head/body 壳——统一从 Document 根遍历即可：head 一族（style /
//!    title / meta / link / script）连同子树跳过，html / body 作为结构壳透明
//!    提升。注意管线路径下 sanitize（ammonia 以 fragment 解析）已先行剥壳，
//!    head 一族跳过主要是对 `build_document` 直接调用的防御性兜底；
//! 3. 按块级白名单表递归构建：
//!
//!    | 标签 | 产物 |
//!    |---|---|
//!    | div / center / 未知标签 | [`Block::Container`]（`<center>` 附带居中对齐） |
//!    | p | [`Block::Paragraph`] |
//!    | h1–h6 | [`Block::Heading`]（含对齐字段） |
//!    | ul / ol + li | [`Block::List`] |
//!    | blockquote | [`Block::Quote`] |
//!    | pre | [`Block::Container`]（内容空白原样保留，不折叠） |
//!    | hr | [`Block::Hr`] |
//!    | br | [`InlineRun::Br`]（并入所在段落） |
//!    | table 一族 | [`Block::Table`]：tbody/thead/tfoot 透明容器、tr→TableRow、
//!      td/th→TableCell（colspan/rowspan/宽度/高度/bgcolor 属性兜底解析、自身
//!      样式，字体属性族进 runs base）、cell 内递归既有块级构建（嵌套表格
//!      自然成立）；行序列经网格归一化（线上反馈②）：rowspan 占位对齐列位、
//!      列宽跨行协调、border 属性网格线、cellpadding；caption/colgroup/col
//!      跳过 |
//!    | script / style / head / title 等元数据 | 连同子树整体跳过 |
//!
//! 4. 行内扁平化（Task 5）：行内元素样式经 [`parse_inline_style`] 解析、
//!    [`merge_inline`] 逐层叠加为 run 样式；`<a>` → [`InlineRun::Link`]（href
//!    经 [`classify_url`] 终审白名单，不合格链接整体丢弃、内容降级纯文字）；
//!    `img` → 图片（src 经 [`classify_image_src`] 终审白名单）。行内 img 保留
//!    在段落 runs 内（线上反馈③第二波：提升为块级会把「链接 + 行内小图」
//!    混排的横向菜单切成逐项竖排；渲染层 wrap-flow 消费同段内的 Image run）；
//!    同样式相邻 Text 合并；
//! 5. 空白折叠（Task 5，HTML 块级上下文语义）：连续空白（空格/制表/换行）折叠
//!    为单空格，段首/段尾与强制换行边界的空白删除；`pre` 例外（内容原样保留）；
//!    `&nbsp;`（U+00A0）属内容空白，不折叠不删除；
//! 6. 继承与对齐：块级 style 的字体属性族（color/font-size/font-weight/
//!    font-style/text-decoration）统一作为后代文本的 base [`InlineStyle`]（行内
//!    样式经 merge 叠加；T7 裁定①推广到全部块级 p/div/heading/li/table 一处
//!    收口，此前仅 color 单属性 + td/th 双通道）；对齐 `text-align` 样式优先、
//!    `align` 属性兜底；
//! 7. `display:none` 子树剪枝（style 属性先经 [`parse_block_style`] 解析）；
//!    树遍历深度超 [`MAX_TREE_DEPTH`] 的子树静默剪枝；
//! 8. body 的 background-color（style / bgcolor 属性）应用到顶层首个无自身背景
//!    的 Container（T4 M-4 最小合理版本；管线路径 body 壳已被 sanitize 剥除，
//!    仅对 `build_document` 直接调用成立）；
//! 9. 节点计数超过 [`Limits::max_nodes`] → [`HtmlError::TooManyNodes`]；
//!    无有效块 → [`HtmlError::Empty`]。

use html5ever::tendril::TendrilSink;
use html5ever::{Attribute, ParseOpts, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use base64::Engine;
use base64::alphabet::STANDARD as BASE64_ALPHABET;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};

use crate::error::HtmlError;
use crate::style::{
    is_floated, is_inline_display, merge_inline, parse_align, parse_block_style, parse_color,
    parse_inline_style,
};
use crate::types::{
    Align, Block, BlockStyle, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle, Limits,
    Rgba, SafeUrl, TableCell, TableRow, Width,
};

/// 树遍历深度上限（含块/行内递归）。
///
/// 病理嵌套防护（与浏览器 ~512 层的防护同数量级）：超出上限的子树剪枝，防止
/// 超深文档打爆渲染栈；上限内的正常嵌套邮件（几十层）不受影响。
///
/// 栈占用说明：深度上限的两端约束——
/// - **下限（真实邮件）**：层层转发的引用链可嵌套数千层，上限必须远低于
///   溢出阈值；
/// - **上限（主线程渲染）**：解析侧递归已由 [`crate::pipeline::render_pipeline`]
///   的 32MB 大栈线程承担，但渲染层（adapt 遍历、GPUI 嵌套元素树构建/Drop）
///   仍在 Windows 默认 1MB 栈的主线程上——256 层 × debug 膨胀帧在 1MB 内
///   有余量，且远超真实邮件嵌套（HubSpot 营销邮件约 20–40 层）。
/// 剪枝语义的测试在显式大栈线程中执行（见 `deep_nesting_is_bounded_not_panicking`）。
const MAX_TREE_DEPTH: usize = 256;

/// 解析 HTML 为中间表示（管线最后一环）。
///
/// 节点计数超限 → [`HtmlError::TooManyNodes`]（携带实际计数）；净化/解析后无有效块
/// → [`HtmlError::Empty`]（调用方降级纯文本）。
pub fn build_document(html: &str, limits: &Limits) -> Result<HtmlDocument, HtmlError> {
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(html.to_owned());
    let body_bg = body_background_color(&dom.document);
    let mut builder = TreeBuilder { limits, visited: 0 };
    // 解析器总是补全 html/head/body 壳（css-inline 的完整文档与裸片段两种输入同构），
    // 从 Document 根遍历即可：head 一族被跳过，html/body 作为结构壳透明提升。
    let root_base = InlineStyle::default();
    let mut blocks = builder.child_blocks(&dom.document, 0, Align::Left, &root_base, Ws::Fold)?;
    if blocks.is_empty() {
        return Err(HtmlError::Empty);
    }
    // body 背景色兜底到顶层首个 Container（容器自身背景优先，T4 M-4 的最小合理版本：
    // 覆盖 `<body style="background:…"><div>…` 的主导邮件形态；首块非 Container 的
    // 文档不应用，文档级背景通道归渲染层 T7 裁定）
    if let Some(bg) = body_bg
        && let Some(Block::Container { style, .. }) = blocks.first_mut()
    {
        style.background_color.get_or_insert(bg);
    }
    Ok(HtmlDocument { blocks })
}

/// body 的背景色：style 的 background-color，`bgcolor` 属性兜底（遗留邮件惯用）。
///
/// 仅对 `build_document` 直接收到的完整文档生效；管线路径 sanitize 已剥壳，
/// body 属性在进入本函数前已被丢弃（管线级 body 背景保留归后续任务裁定）。
fn body_background_color(node: &Handle) -> Option<Rgba> {
    for child in node.children.borrow().iter() {
        if let NodeData::Element { name, attrs, .. } = &child.data {
            match name.local.as_ref() {
                "body" => {
                    let attrs = attrs.borrow();
                    let style =
                        parse_block_style(attr_str(attrs.as_slice(), "style").unwrap_or(""));
                    return style
                        .background_color
                        .or_else(|| attr_str(attrs.as_slice(), "bgcolor").and_then(parse_color));
                }
                "html" => return body_background_color(child),
                _ => {}
            }
        }
    }
    None
}

/// 块级白名单表的产物种类。
#[derive(Debug, Clone, Copy)]
enum BlockKind {
    /// div / 未知标签；`center = true` 时附带居中对齐（`<center>` 语义）。
    Container { center: bool },
    /// p。
    Paragraph,
    /// h1–h6。
    Heading(u8),
    /// ul / ol。
    List { ordered: bool },
    /// blockquote。
    Quote,
    /// pre：内容空白原样保留（不折叠不修剪）。
    Pre,
    /// hr。
    Hr,
    /// table：tbody/thead/tfoot 透明、tr→TableRow、td/th→TableCell。
    Table,
}

/// 标签分类（见模块文档的白名单表）。
enum Kind {
    /// 元数据（script/style/head/title/meta/link/base/noscript/template）：
    /// 连同子树整体跳过。
    Skip,
    /// 结构壳（html/body）：不产生块，子内容直接提升。
    Pass,
    /// br：并入所在段落为 [`InlineRun::Br`]。
    Br,
    /// 常见行内元素：样式叠加 + a → Link / img → Image 终审白名单分类。
    Inline,
    /// 块级白名单。
    Block(BlockKind),
}

/// 标签分类函数；未知标签宽容降级为 [`BlockKind::Container`]（spec §3）。
fn classify(local: &str) -> Kind {
    match local {
        "script" | "style" | "head" | "title" | "meta" | "link" | "base" | "noscript"
        | "template" => Kind::Skip,
        "html" | "body" => Kind::Pass,
        "br" => Kind::Br,
        "p" => Kind::Block(BlockKind::Paragraph),
        "h1" => Kind::Block(BlockKind::Heading(1)),
        "h2" => Kind::Block(BlockKind::Heading(2)),
        "h3" => Kind::Block(BlockKind::Heading(3)),
        "h4" => Kind::Block(BlockKind::Heading(4)),
        "h5" => Kind::Block(BlockKind::Heading(5)),
        "h6" => Kind::Block(BlockKind::Heading(6)),
        "ul" => Kind::Block(BlockKind::List { ordered: false }),
        "ol" => Kind::Block(BlockKind::List { ordered: true }),
        "blockquote" => Kind::Block(BlockKind::Quote),
        "pre" => Kind::Block(BlockKind::Pre),
        "hr" => Kind::Block(BlockKind::Hr),
        "table" => Kind::Block(BlockKind::Table),
        "div" => Kind::Block(BlockKind::Container { center: false }),
        "center" => Kind::Block(BlockKind::Container { center: true }),
        // 常见行内元素（img 在行内收集期分类为图片：段级提升为块级、链接内保留行内）
        "span" | "a" | "b" | "strong" | "i" | "em" | "u" | "s" | "strike" | "del" | "ins"
        | "small" | "big" | "sub" | "sup" | "font" | "code" | "tt" | "q" | "cite" | "abbr"
        | "acronym" | "label" | "mark" | "img" => Kind::Inline,
        // 其余（tbody/tr/td/th 由表格构建器按层级取用；mjml-* 等模板标签）→ 未知标签宽容降级
        _ => Kind::Block(BlockKind::Container { center: false }),
    }
}

/// 空白处理模式：块级上下文按 HTML 语义折叠；pre 内容原样保留。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ws {
    /// HTML 块级上下文：连续空白折叠为单空格、行首/行尾/换行边界空白删除。
    Fold,
    /// pre：文本节点原样保留。
    Preserve,
}

/// 树构建器：rcdom 遍历 + 节点计数。
struct TreeBuilder<'a> {
    limits: &'a Limits,
    /// 已遍历节点数（元素 + 文本，跳过/剪枝的子树不计）。
    visited: usize,
}

impl<'a> TreeBuilder<'a> {
    /// 节点计数：超过 [`Limits::max_nodes`] 立即终止（携带实际计数）。
    fn check_node(&mut self) -> Result<(), HtmlError> {
        self.visited += 1;
        if self.visited > self.limits.max_nodes {
            return Err(HtmlError::TooManyNodes(self.visited));
        }
        Ok(())
    }

    /// 把 `node` 的子节点序列构建为块序列（含混合内容聚合）。
    ///
    /// `inherited` 为父容器解析后的水平对齐（text-align 继承语义）；`text_base`
    /// 为父级累积的文本 base 样式（块级 color 继承语义）。
    fn child_blocks(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
        ws: Ws,
    ) -> Result<Vec<Block>, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }
        let mut blocks = Vec::new();
        let mut inline = InlineBuffer::new(ws);
        // 相邻 float 兄弟的累积列（浏览器浮动列布局近似——收集到非 float
        // 边界物化为单行 Table，保留各列宽/内边距水平并排，线上反馈③第三波：
        // banner 的 logo 列/导航列靠 float:left|right 并排，逐块竖排即用户
        // 看到"banner 不对"）
        let mut float_cells: Vec<TableCell> = Vec::new();
        let flush_floats = |blocks: &mut Vec<Block>, cells: &mut Vec<TableCell>| {
            if cells.is_empty() {
                return;
            }
            blocks.push(Block::Table {
                style: BlockStyle::default(),
                rows: vec![TableRow {
                    cells: std::mem::take(cells),
                }],
            });
        };
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => {
                    if !contents.borrow().trim_matches(is_html_ws).is_empty() {
                        flush_floats(&mut blocks, &mut float_cells);
                    }
                    inline.push_text(&contents.borrow(), text_base)
                }
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Pass => {
                            // html/body 结构壳：子块直接提升，无自身样式
                            flush_floats(&mut blocks, &mut float_cells);
                            blocks.extend(self.child_blocks(
                                child,
                                depth + 1,
                                inherited,
                                text_base,
                                ws,
                            )?);
                        }
                        Kind::Inline => {
                            let attrs = attrs.borrow();
                            self.collect_inline_element(
                                child,
                                attrs.as_slice(),
                                &mut inline,
                                text_base,
                                ws,
                                depth + 1,
                            )?;
                        }
                        Kind::Br => inline.push_br(),
                        Kind::Block(kind) => {
                            let attrs = attrs.borrow();
                            let style_text = attr_str(attrs.as_slice(), "style").unwrap_or("");
                            if is_floated(style_text) {
                                // float:left|right（review M-3：float 优先于
                                // display:inline-block——浏览器语义，列宽得以
                                // 保留）：入浮动列累积组（见 float_cells）。
                                // 首个 float 前先物化行内缓冲（review I-4）：
                                // 「文字A float 文字B」若 A 留在缓冲，物化顺序
                                // 会变成 [float表][A+B合并段]——A 被搬到 float
                                // 之后；先 flush 保住 [A][float表][B] 文档序
                                if float_cells.is_empty() {
                                    blocks.extend(inline.flush_blocks(inherited));
                                }
                                let built = self.build_element_blocks(
                                    child,
                                    kind,
                                    attrs.as_slice(),
                                    depth,
                                    inherited,
                                    text_base,
                                )?;
                                if let Some(cell) = float_cell_of(built) {
                                    float_cells.push(cell);
                                }
                            } else if is_inline_display(style_text) {
                                // 行内系 display（inline/inline-block/inline-flex，
                                // 线上反馈③）：块级标签按行内透传并入父级行内流——
                                // 邮件横向菜单的主流写法，逐项成块会竖排
                                self.collect_inline_element(
                                    child,
                                    attrs.as_slice(),
                                    &mut inline,
                                    text_base,
                                    ws,
                                    depth + 1,
                                )?;
                            } else {
                                flush_floats(&mut blocks, &mut float_cells);
                                blocks.extend(inline.flush_blocks(inherited));
                                blocks.extend(self.build_element_blocks(
                                    child,
                                    kind,
                                    attrs.as_slice(),
                                    depth,
                                    inherited,
                                    text_base,
                                )?);
                            }
                        }
                    }
                }
                _ => {} // 注释 / DOCTYPE / PI：跳过
            }
        }
        flush_floats(&mut blocks, &mut float_cells);
        blocks.extend(inline.flush_blocks(inherited));
        Ok(blocks)
    }

    /// 构建单个块级元素为块序列；`display:none` 子树返回空序列剪枝。
    /// 段落含行内图片时切分为多块（见 [`split_image_blocks`]）。
    fn build_element_blocks(
        &mut self,
        node: &Handle,
        kind: BlockKind,
        attrs: &[Attribute],
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
    ) -> Result<Vec<Block>, HtmlError> {
        let style_text = attr_str(attrs, "style").unwrap_or("");
        let mut style = parse_block_style(style_text);
        if style.hidden {
            return Ok(Vec::new()); // display:none 子树整体剪枝
        }
        // 对齐：text-align 样式优先，align 属性兜底（T5 评审③；物化进
        // style.text_align 统一通道，children 经 own_align 继承）
        if style.text_align.is_none() {
            style.text_align = attr_str(attrs, "align").and_then(parse_align);
        }
        // 自身解析后的对齐（无声明则继承父级）；<center> 补 Center 语义
        let center = matches!(kind, BlockKind::Container { center: true });
        let own_align = style
            .text_align
            .or_else(|| center.then_some(Align::Center))
            .unwrap_or(inherited);
        // 块级字体属性族 → 后代文本 base 行内样式（T7 裁定①：color/font-size/
        // font-weight/font-style/text-decoration 五族经 parse_inline_style 统一
        // 映射，p/div/heading/table 等全部块级一处收口——此前仅 color 单属性
        // （T5 评审②），td/th 的双通道同语义（T6）；Heading 的块级 color 由同
        // 一合并覆盖，不设第二份；行内声明经 merge_inline 照常叠加覆盖）
        let child_base = merge_inline(text_base, &parse_inline_style(style_text));
        let blocks = match kind {
            BlockKind::Container { center } => {
                // <center> 语义物化进自身样式（GPUI 渲染层消费 text_align）
                if center && style.text_align.is_none() {
                    style.text_align = Some(Align::Center);
                }
                vec![Block::Container {
                    style,
                    children: self.child_blocks(
                        node,
                        depth + 1,
                        own_align,
                        &child_base,
                        Ws::Fold,
                    )?,
                }]
            }
            BlockKind::Pre => vec![Block::Container {
                style,
                children: self.child_blocks(
                    node,
                    depth + 1,
                    own_align,
                    &child_base,
                    Ws::Preserve,
                )?,
            }],
            BlockKind::Paragraph => {
                let runs = self.collect_runs_normalized(node, depth + 1, &child_base, Ws::Fold)?;
                // 空 p 保留（邮件中以空段落做间距的惯用法）；行内图片保留在
                // runs 内（线上反馈③第二波——提升切分会拆散横向菜单混排流）
                vec![Block::Paragraph {
                    runs,
                    align: own_align,
                    style,
                }]
            }
            BlockKind::Heading(level) => vec![Block::Heading {
                level,
                align: own_align,
                runs: self.collect_runs_normalized(node, depth + 1, &child_base, Ws::Fold)?,
            }],
            BlockKind::List { ordered } => vec![Block::List {
                ordered,
                items: self.build_list_items(node, depth + 1, own_align, &child_base, Ws::Fold)?,
            }],
            BlockKind::Quote => vec![Block::Quote {
                children: self.child_blocks(node, depth + 1, own_align, &child_base, Ws::Fold)?,
            }],
            BlockKind::Table => {
                // 遗留呈现属性兜底（T6 预批准补 + 线上反馈②）：style 声明优先，
                // `width` 属性兜底宽度（与 td/img 同序）、`bgcolor` 属性兜底背景色
                // （具名色走 parse_color 的基础 16 色表——遗留邮件惯用
                // `<table bgcolor="…">`）；`border`/`bordercolor`/`cellpadding`
                // 喂网格归一化（border="1" 的 1px 全框线是遗留邮件表格的主流画法）
                let table_border = attr_str(attrs, "border").and_then(parse_table_border_attr);
                let border_color = attr_str(attrs, "bordercolor").and_then(parse_color);
                let cellpadding = attr_str(attrs, "cellpadding").and_then(parse_cellpadding_attr);
                let raw_rows = self.build_table_rows(node, depth + 1, own_align, &child_base)?;
                let grid = TableGrid {
                    border: table_border,
                    border_color: border_color.unwrap_or(TABLE_GRID_BORDER_COLOR),
                    cellpadding,
                };
                vec![Block::Table {
                    style: BlockStyle {
                        width: style
                            .width
                            .or_else(|| attr_str(attrs, "width").and_then(parse_dim_attr)),
                        background_color: style
                            .background_color
                            .or_else(|| attr_str(attrs, "bgcolor").and_then(parse_color)),
                        ..style
                    },
                    rows: normalize_table_grid(raw_rows, &grid),
                }]
            }
            BlockKind::Hr => vec![Block::Hr],
        };
        Ok(blocks)
    }

    /// 列表项构建：li → 子块序列；裸行内内容聚合为独立列表项（宽容处理）。
    fn build_list_items(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
        ws: Ws,
    ) -> Result<Vec<Vec<Block>>, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }
        let mut items: Vec<Vec<Block>> = Vec::new();
        let mut inline = InlineBuffer::new(ws);
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => inline.push_text(&contents.borrow(), text_base),
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    if local == "li" {
                        let attrs = attrs.borrow();
                        let style_text = attr_str(attrs.as_slice(), "style").unwrap_or("");
                        let style = parse_block_style(style_text);
                        if style.hidden {
                            continue;
                        }
                        let flushed = inline.flush_blocks(inherited);
                        if !flushed.is_empty() {
                            items.push(flushed);
                        }
                        let align = style.text_align.unwrap_or(inherited);
                        // li 同块级字体属性族 base 通道（T7 裁定①"全部块级"）
                        let li_base = merge_inline(text_base, &parse_inline_style(style_text));
                        items.push(self.child_blocks(child, depth + 1, align, &li_base, ws)?);
                        continue;
                    }
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Inline => {
                            let attrs = attrs.borrow();
                            self.collect_inline_element(
                                child,
                                attrs.as_slice(),
                                &mut inline,
                                text_base,
                                ws,
                                depth + 1,
                            )?;
                        }
                        Kind::Br => inline.push_br(),
                        Kind::Pass => {
                            let flushed = inline.flush_blocks(inherited);
                            if !flushed.is_empty() {
                                items.push(flushed);
                            }
                            items.push(self.child_blocks(
                                child,
                                depth + 1,
                                inherited,
                                text_base,
                                ws,
                            )?);
                        }
                        Kind::Block(kind) => {
                            let flushed = inline.flush_blocks(inherited);
                            if !flushed.is_empty() {
                                items.push(flushed);
                            }
                            let attrs = attrs.borrow();
                            let built = self.build_element_blocks(
                                child,
                                kind,
                                attrs.as_slice(),
                                depth,
                                inherited,
                                text_base,
                            )?;
                            if !built.is_empty() {
                                items.push(built); // 非 li 的块级子元素宽容为独立列表项
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let flushed = inline.flush_blocks(inherited);
        if !flushed.is_empty() {
            items.push(flushed);
        }
        Ok(items)
    }

    // ---- 表格构建（T6）----

    /// table 子层级遍历：tbody/thead/tfoot 透明容器递归（行按文档顺序平铺）、
    /// tr → 行；其余子元素（caption/colgroup/col 及意外元素）与文本节点跳过——
    /// html5ever 的 foster parenting 已把表格内的非空白游离内容移出表格外，
    /// 此处只剩空白文本节点（无块语义）。
    fn build_table_rows(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
    ) -> Result<Vec<TableRow>, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }
        let mut rows = Vec::new();
        for child in node.children.borrow().iter() {
            self.check_node()?;
            if let NodeData::Element { name, attrs, .. } = &child.data {
                match name.local.as_ref() {
                    "tbody" | "thead" | "tfoot" => rows.extend(self.build_table_rows(
                        child,
                        depth + 1,
                        inherited,
                        text_base,
                    )?),
                    "tr" => {
                        let attrs = attrs.borrow();
                        // display:none 行剪枝（types.rs 约定：树构建期完成）
                        if parse_block_style(attr_str(attrs.as_slice(), "style").unwrap_or(""))
                            .hidden
                        {
                            continue;
                        }
                        rows.push(self.build_table_row(child, depth + 1, inherited, text_base)?)
                    }
                    _ => {}
                }
            }
        }
        Ok(rows)
    }

    /// tr → TableRow：td/th → 单元格，其余子元素忽略（html5ever 已滤）。
    fn build_table_row(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
    ) -> Result<TableRow, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(TableRow { cells: Vec::new() });
        }
        let mut cells = Vec::new();
        for child in node.children.borrow().iter() {
            self.check_node()?;
            if let NodeData::Element { name, attrs, .. } = &child.data {
                let local = name.local.as_ref();
                if local == "td" || local == "th" {
                    let attrs = attrs.borrow();
                    if let Some(cell) = self.build_table_cell(
                        child,
                        attrs.as_slice(),
                        depth + 1,
                        inherited,
                        text_base,
                    )? {
                        cells.push(cell);
                    }
                }
            }
        }
        Ok(TableRow { cells })
    }

    /// td/th → [`TableCell`]：
    ///
    /// - colspan/rowspan：默认 1，数值钳制 1–16（types.rs 契约；0/负/非数字回退
    ///   默认）——rowspan 经网格归一化生成占位单元格对齐列位（线上反馈②：
    ///   此前 rowspan 忽略，span 行少一格整体左移，「续费价格」列显示成日期）；
    /// - 宽度：style `width` 优先、`width` 属性兜底（与 img 尺寸同序）；
    ///   高度：style `height` 优先、`height` 属性兜底（单元格定高是遗留邮件
    ///   行高的主流写法）；
    /// - 自身样式：style 属性 → BlockStyle；对齐 text-align 样式优先、align 属性兜底；
    /// - 背景：style `background-color` 优先、`bgcolor` 属性兜底（T6 预批准补——
    ///   表格单元格底色是遗留邮件的主流写法，具名色走 [`parse_color`] 基础 16 色表）；
    /// - 字体属性族进 runs base（T5 评审捎带②）：style 声明中的 color/font-size/
    ///   font-weight/font-style/text-decoration 经 [`parse_inline_style`] 映射、
    ///   [`merge_inline`] 叠加为内容 runs 的 base——表格单元格文字样式的主流来源
    ///   （行内声明可覆盖，cell 内嵌套块经 text_base 透传继承）；
    /// - `display:none` 单元格整体剪枝。
    fn build_table_cell(
        &mut self,
        node: &Handle,
        attrs: &[Attribute],
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
    ) -> Result<Option<TableCell>, HtmlError> {
        let style_text = attr_str(attrs, "style").unwrap_or("");
        let mut style = parse_block_style(style_text);
        if style.hidden {
            return Ok(None);
        }
        if style.background_color.is_none() {
            style.background_color = attr_str(attrs, "bgcolor").and_then(parse_color);
        }
        if style.height.is_none() {
            style.height = attr_str(attrs, "height").and_then(parse_dim_attr);
        }
        let colspan = attr_str(attrs, "colspan")
            .and_then(parse_colspan)
            .unwrap_or(1);
        let rowspan = attr_str(attrs, "rowspan")
            .and_then(parse_colspan)
            .unwrap_or(1);
        let mut width = style.width;
        if width.is_none() {
            width = attr_str(attrs, "width").and_then(parse_dim_attr);
        }
        let own_align = style
            .text_align
            .or_else(|| attr_str(attrs, "align").and_then(parse_align))
            .unwrap_or(inherited);
        let cell_base = merge_inline(text_base, &parse_inline_style(style_text));
        let blocks = self.child_blocks(node, depth + 1, own_align, &cell_base, Ws::Fold)?;
        Ok(Some(TableCell {
            blocks,
            colspan,
            rowspan,
            width,
            style,
        }))
    }

    /// 收集块级元素（p/h1–h6）内容为行内 run 序列并做段落级规整。
    fn collect_runs_normalized(
        &mut self,
        node: &Handle,
        depth: usize,
        base: &InlineStyle,
        ws: Ws,
    ) -> Result<Vec<InlineRun>, HtmlError> {
        let mut inline = InlineBuffer::new(ws);
        self.collect_runs_into(node, &mut inline, base, ws, depth)?;
        Ok(inline.finish())
    }

    /// 处理单个行内元素：display:none 剪枝 → 样式叠加 → a/img 分类。
    ///
    /// - `a`：href 经 [`classify_url`] 终审——合格包为 [`InlineRun::Link`]（内容
    ///   递归收集进独立缓冲，嵌套 span 样式照常叠加）；不合格（javascript:/data:
    ///   /相对路径等）链接整体丢弃，内容降级纯文字继续并入当前序列；
    /// - `img`：src 经 [`classify_image_src`] 终审，产 [`InlineRun::Image`]（段级
    ///   上下文在段落物化时提升为 [`Block::Image`]）；
    /// - 其余行内元素：透明容器，样式叠加后递归。
    fn collect_inline_element(
        &mut self,
        node: &Handle,
        attrs: &[Attribute],
        inline: &mut InlineBuffer,
        base: &InlineStyle,
        ws: Ws,
        depth: usize,
    ) -> Result<(), HtmlError> {
        // 调用方保证 node 为元素节点；局部名从节点自取（省一个参数位）
        let NodeData::Element { name, .. } = &node.data else {
            return Ok(());
        };
        let local = name.local.as_ref();
        let style_text = attr_str(attrs, "style").unwrap_or("");
        let block_style = parse_block_style(style_text);
        if block_style.hidden {
            return Ok(()); // display:none 行内子树剪枝
        }
        let mut own = tag_inline_style(local, style_text, attrs);
        // 行内底色（review M-2）：style 声明的纯色背景随行内样式叠加——
        // span/mark 高亮、按钮式菜单项底色此前在行内通道整体丢失
        own.background = own.background.or(block_style.background_color);
        let merged = merge_inline(base, &own);
        if local == "img" {
            inline.push_wrapped(InlineRun::Image {
                src: classify_image_src(attr_str(attrs, "src").unwrap_or("")),
                alt: attr_str(attrs, "alt").unwrap_or("").to_string(),
                style: image_style(attrs),
            });
            return Ok(());
        }
        if local == "a" {
            return match classify_url(attr_str(attrs, "href").unwrap_or("")) {
                Some(href) => {
                    let mut inner = InlineBuffer::new(ws);
                    self.collect_runs_into(node, &mut inner, &merged, ws, depth)?;
                    let runs = inner.finish();
                    if runs_meaningful(&runs) {
                        inline.push_wrapped(InlineRun::Link { href, runs });
                    }
                    Ok(())
                }
                // 不合格协议：链接不保留，内容降级为纯文字继续收集（spec §4.4）
                None => self.collect_runs_into(node, inline, &merged, ws, depth),
            };
        }
        self.collect_runs_into(node, inline, &merged, ws, depth)
    }

    /// 收集行内内容：文本节点经空白折叠进入缓冲，行内元素按
    /// [`Self::collect_inline_element`] 分类，块级/壳元素按混合内容规则宽容解包
    /// （正常解析下 p/h 内不含块级子元素——解析器先行闭合；手工构造的畸形树不丢内容）。
    fn collect_runs_into(
        &mut self,
        node: &Handle,
        inline: &mut InlineBuffer,
        base: &InlineStyle,
        ws: Ws,
        depth: usize,
    ) -> Result<(), HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(());
        }
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => inline.push_text(&contents.borrow(), base),
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Br => inline.push_br(),
                        Kind::Inline => {
                            let attrs = attrs.borrow();
                            self.collect_inline_element(
                                child,
                                attrs.as_slice(),
                                inline,
                                base,
                                ws,
                                depth + 1,
                            )?;
                        }
                        Kind::Block(_) | Kind::Pass => {
                            let hidden = attr_str(&attrs.borrow(), "style")
                                .map(parse_block_style)
                                .is_some_and(|style| style.hidden);
                            if !hidden {
                                self.collect_runs_into(child, inline, base, ws, depth + 1)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// 链接终审白名单（spec §4.4）：仅 http/https/mailto——大小写不敏感、前导/尾随
/// 空白 trim。其余协议（`javascript:` / `data:` / `vbscript:` / 相对路径等）一律
/// None，调用方把链接整体丢弃、内容降级纯文字。
///
/// 属性值已经 html5ever 实体解码（`&#58;` → `:` 的混淆形态到达此处时已是
/// 解码后的真实协议串，同样被拒）。白名单外即使形态合法也不放行。
fn classify_url(raw: &str) -> Option<SafeUrl> {
    let url = raw.trim();
    let lowered = url.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        Some(SafeUrl::Http(url.to_string()))
    } else if lowered.starts_with("mailto:") {
        Some(SafeUrl::Mailto(url.to_string()))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// 表格网格归一化（线上反馈②）：rowspan 占位对齐 + 列宽跨行协调 +
// border 属性网格线 + cellpadding。构建期一次后处理，IR/渲染层消费结果。
// ---------------------------------------------------------------------------

/// `border` 属性未声明颜色时的网格默认色（浏览器遗留表格的惯用灰）。
const TABLE_GRID_BORDER_COLOR: Rgba = [128, 128, 128, 255];

/// 表格级网格参数（`<table border/bordercolor/cellpadding>` 属性解析产物）。
struct TableGrid {
    /// 网格线宽（`border` 属性值，>0 才画线；钳制 0.5–8px 防病理值）。
    border: Option<f32>,
    /// 网格线色（`bordercolor` 属性 → parse_color；未声明取默认灰）。
    border_color: Rgba,
    /// 单元格默认内边距（`cellpadding` 属性，钳制 0–64px；None 不施加）。
    cellpadding: Option<f32>,
}

/// `border` 属性：数值（px）；0 / 非数字 / 非有限 → None（无网格线）。
/// 值钳制到 0.5–8px：遗留邮件惯用 1–2，浏览器对超大值也会收敛渲染。
fn parse_table_border_attr(value: &str) -> Option<f32> {
    let v = value.trim().trim_end_matches("px").trim().parse::<f32>().ok()?;
    (v.is_finite() && v > 0.0).then(|| v.clamp(0.5, 8.0))
}

/// `cellpadding` 属性：数值（px），钳制 0–64px；非数字/非有限 → None。
fn parse_cellpadding_attr(value: &str) -> Option<f32> {
    let v = value.trim().trim_end_matches("px").trim().parse::<f32>().ok()?;
    v.is_finite().then(|| v.clamp(0.0, 64.0))
}

/// 网格归一化的中间产物：一个已定位的单元格。
struct PlacedCell {
    cell: TableCell,
    /// 终止行（不含）：起始行 + rowspan（表格外缘下边框判据）。
    row_end: usize,
    /// 占位单元格归属的 span 起始列（None = 真实单元格）。span 内部占位
    /// 不画上/下边框（浏览器语义：跨行单元格内部无横线）。
    span_col: Option<usize>,
}

/// 表格网格归一化（线上反馈②）：
///
/// 1. **rowspan 占位**：按 (row, col) 占位追踪展开——span 覆盖的格位在后续
///    行生成空占位单元格，行内单元格回到正确列位（此前 rowspan 忽略，
///    span 行少一格整体左移，「续费价格」列显示成日期）；
/// 2. **列宽跨行协调**：任一行声明的列宽（colspan=1）传播到所有行的同列
///    （渲染层逐行独立 flex 布局，无协调则各行列宽漂移、竖线错位）；
/// 3. **border 网格线**：`border` 属性 >0 时按 collapse 语义给单元格补边框
///    ——每格 top/left 总是、right 仅尾列、bottom 仅终止行在末行的格
///    （相邻格不双边，避免 2px 重叠线）；span 内部占位只画 span 左边界；
///    单元格自身声明的边框优先（仅补 None 的边）；
/// 4. **cellpadding**：无自身 padding 声明的单元格施加表级默认内边距。
fn normalize_table_grid(rows: Vec<TableRow>, grid: &TableGrid) -> Vec<TableRow> {
    // (row, col) -> 覆盖该格位的 span（起始列, 终止行）
    let mut covered: std::collections::HashMap<(usize, usize), (usize, usize)> =
        std::collections::HashMap::new();
    let mut placed_rows: Vec<Vec<PlacedCell>> = Vec::with_capacity(rows.len());
    for (r, row) in rows.into_iter().enumerate() {
        let mut placed: Vec<PlacedCell> = Vec::new();
        let mut col = 0usize;
        for cell in row.cells {
            while let Some(&(span_col, _)) = covered.get(&(r, col)) {
                placed.push(PlacedCell::placeholder(col, span_col));
                col += 1;
            }
            let cs = cell.colspan as usize;
            let row_end = r + cell.rowspan as usize;
            for rr in r..row_end {
                for cc in col..col + cs {
                    covered.insert((rr, cc), (col, row_end));
                }
            }
            placed.push(PlacedCell {
                row_end,
                cell,
                span_col: None,
            });
            col += cs;
        }
        // 尾部被上方 span 延伸覆盖的格位同样补占位
        while let Some(&(span_col, _)) = covered.get(&(r, col)) {
            placed.push(PlacedCell::placeholder(col, span_col));
            col += 1;
        }
        placed_rows.push(placed);
    }
    let n_rows = placed_rows.len();
    let n_cols = placed_rows
        .iter()
        .map(|row| row.iter().map(|p| p.cell.colspan as usize).sum::<usize>())
        .max()
        .unwrap_or(0);

    // 列宽协调：首见声明传播到同列所有行（colspan=1 才可逐列定宽）。
    // review M-6：声明百分比总和 >100%（邮件作者常态误差）先按比例归一到
    // 100——浏览器同语义；不归一的话未声明列的自由空间为 0、塌成 ~0 宽
    let mut col_widths: Vec<Option<Width>> = vec![None; n_cols];
    for row in &placed_rows {
        let mut col = 0usize;
        for p in row {
            if p.cell.colspan == 1 && p.cell.width.is_some() && col_widths[col].is_none() {
                col_widths[col] = p.cell.width;
            }
            col += p.cell.colspan as usize;
        }
    }
    let declared_pct_sum: f32 = col_widths
        .iter()
        .filter_map(|w| match w {
            Some(Width::Percent(p)) => Some(*p),
            _ => None,
        })
        .sum();
    if declared_pct_sum > 100.0 {
        let scale = 100.0 / declared_pct_sum;
        for w in &mut col_widths {
            if let Some(Width::Percent(p)) = w {
                *p *= scale;
            }
        }
    }
    // 传播 + 归一改写：所有 colspan=1 格统一落 col_widths[col]（声明格在
    // M-6 归一中也要改写，否则只有未声明列拿到归一值、总和仍超 100）
    for row in &mut placed_rows {
        let mut col = 0usize;
        for p in row {
            if p.cell.colspan == 1 {
                p.cell.width = col_widths[col];
            }
            col += p.cell.colspan as usize;
        }
    }

    // cellpadding 施加（review M-1：先于估权——列权重需含表级默认内边距，
    // 此前估权在施加前跑、比例略偏）。仅无自身 padding 声明的单元格
    if let Some(cp) = grid.cellpadding.filter(|cp| *cp > 0.0) {
        for row in &mut placed_rows {
            for p in row {
                if p.cell.style.padding == [0.0; 4] {
                    p.cell.style.padding = [cp; 4];
                }
            }
        }
    }

    // 内容权重列宽协调（线上反馈②第二波）：渲染层逐行独立 flex，无声明宽度
    // 的列走等分 grow + min-content 钳制——每行按"本行内容最小宽"起算，行间
    // 列宽漂移（表头窄、数据行宽、竖线累积错位）。构建期按各列最大内容宽度
    // 的比例直接分配百分比，行间共线。跳过两种形态：全列已声明（无自由空间
    // 可分）、含跨列格（跨列格的 grow 权重语义保留）。
    let has_colspan_cell = placed_rows
        .iter()
        .flatten()
        .any(|p| p.cell.colspan > 1);
    let all_declared = n_cols > 0 && col_widths.iter().all(Option::is_some);
    if !has_colspan_cell && !all_declared && n_cols > 1 {
        // 各列权重 = 该列所有单元格内容估宽的最大值（占位格空内容走下限）
        const MIN_COL_WEIGHT_PX: f32 = 28.0;
        let mut weights = vec![MIN_COL_WEIGHT_PX; n_cols];
        for row in &placed_rows {
            let mut col = 0usize;
            for p in row {
                if p.cell.colspan == 1 {
                    weights[col] = weights[col].max(estimate_cell_width_px(&p.cell));
                }
                col += p.cell.colspan as usize;
            }
        }
        // 已声明列锁定其实际宽度，未声明列按权重分剩余百分比
        let declared_pct: f32 = col_widths
            .iter()
            .filter_map(|w| match w {
                Some(Width::Percent(p)) => Some(*p),
                _ => None,
            })
            .sum();
        let free_pct = (100.0 - declared_pct).max(0.0);
        let undeclared_weight: f32 = col_widths
            .iter()
            .zip(&weights)
            .filter(|(w, _)| w.is_none())
            .map(|(_, weight)| *weight)
            .sum();
        // review M-6：free_pct == 0（声明列已占满/超出）时不赋 Percent——
        // Percent(0) 会把列锁死在 0 宽（basis 0 + 无自由空间），保持 None
        // 语义更诚实（等分 grow，与浏览器归一后的行为一致）
        if free_pct > 0.0 && undeclared_weight > 0.0 {
            for row in &mut placed_rows {
                let mut col = 0usize;
                for p in row {
                    // 已声明列经传播阶段已赋值（width 非 None），此处只落未声明列
                    if p.cell.colspan == 1 && p.cell.width.is_none() {
                        p.cell.width =
                            Some(Width::Percent(free_pct * weights[col] / undeclared_weight));
                    }
                    col += p.cell.colspan as usize;
                }
            }
        }
    }

    // 网格线（占位单元格按 span 语义：不画 span 内部的横线，
    // 只在 span 边界与表格外缘画线，跨行区域不被横线切开）
    let line = grid
        .border
        .map(|w| crate::types::Border { width: w, color: grid.border_color });
    for (r, row) in placed_rows.iter_mut().enumerate() {
        let mut col = 0usize;
        for p in row {
            let col_end = col + p.cell.colspan as usize;
            if let Some(line) = line {
                let style = &mut p.cell.style;
                let same_span_left = col
                    .checked_sub(1)
                    .and_then(|left| covered.get(&(r, left)))
                    .is_some_and(|&(sc, _)| Some(sc) == p.span_col);
                let span_row_end = covered.get(&(r, col)).map(|&(_, re)| re);
                match p.span_col {
                    None => {
                        // 真实单元格：top/left 总是；right/bottom 仅在表格外缘
                        if style.border_top.is_none() {
                            style.border_top = Some(line);
                        }
                        if style.border_left.is_none() {
                            style.border_left = Some(line);
                        }
                        if style.border_right.is_none() && col_end == n_cols {
                            style.border_right = Some(line);
                        }
                        if style.border_bottom.is_none() && p.row_end == n_rows {
                            style.border_bottom = Some(line);
                        }
                    }
                    Some(_) => {
                        // span 内部占位：left 仅 span 左边界；right/bottom 仅表格外缘
                        if style.border_left.is_none() && !same_span_left {
                            style.border_left = Some(line);
                        }
                        if style.border_right.is_none() && col_end == n_cols {
                            style.border_right = Some(line);
                        }
                        if style.border_bottom.is_none()
                            && span_row_end.is_some_and(|re| re == n_rows)
                        {
                            style.border_bottom = Some(line);
                        }
                    }
                }
            }
            col = col_end;
        }
    }

    placed_rows
        .into_iter()
        .map(|row| TableRow {
            cells: row.into_iter().map(|p| p.cell).collect(),
        })
        .collect()
}

impl PlacedCell {
    /// span 覆盖格位的空占位单元格（默认样式；网格线/列宽阶段按占位语义处理）。
    fn placeholder(_col: usize, span_col: usize) -> Self {
        Self {
            cell: TableCell {
                blocks: Vec::new(),
                colspan: 1,
                rowspan: 1,
                width: None,
                style: BlockStyle::default(),
            },
            row_end: 0,
            span_col: Some(span_col),
        }
    }
}

// ---------------------------------------------------------------------------
// 内容宽度估算（表格列宽内容权重的输入，浏览器 auto 表布局的近似）：
// 只求列间相对比例，绝对精度不敏感。ASCII/半角按 0.6em、其余（CJK/全角等）
// 按 1.0em，em 取 run 字号（兜底 14px——渲染层 BASE_FONT_SIZE 同值）。
// ---------------------------------------------------------------------------

/// 估算单元格内容的显示宽度（px）：内容最宽块 + 左右内边距。
fn estimate_cell_width_px(cell: &TableCell) -> f32 {
    let [_, pr, _, pl] = cell.style.padding;
    blocks_max_width(&cell.blocks) + pr + pl
}

/// 块序列的最宽块（多块垂直堆叠，宽度由最宽者决定）。
fn blocks_max_width(blocks: &[Block]) -> f32 {
    blocks.iter().map(estimate_block_width).fold(0.0, f32::max)
}

/// 单块的内容宽度估算（px）：内容宽 + 左右内边距。
fn estimate_block_width(block: &Block) -> f32 {
    let (content, pad_lr) = match block {
        Block::Paragraph { runs, style, .. } => {
            (runs_max_width(runs), style.padding[1] + style.padding[3])
        }
        Block::Heading { runs, .. } => (runs_max_width(runs), 0.0),
        Block::Container { style, children } => {
            (blocks_max_width(children), style.padding[1] + style.padding[3])
        }
        Block::Quote { children } => (blocks_max_width(children), 0.0),
        Block::List { items, .. } => (
            items
                .iter()
                .map(|item| blocks_max_width(item))
                .fold(0.0, f32::max),
            0.0,
        ),
        // 嵌套表格：首行各列估宽求和（嵌套表自身另有归一化，此处只需量级）
        Block::Table { rows, .. } => (
            rows.first().map_or(0.0, |row| {
                row.cells.iter().map(estimate_cell_width_px).sum()
            }),
            0.0,
        ),
        Block::Image { style, .. } => (image_width_or_default(style), 0.0),
        Block::Hr => (0.0, 0.0),
    };
    content + pad_lr
}

/// 行内 run 序列的宽度估算：Br 分段（折行语义）取最长段。
fn runs_max_width(runs: &[InlineRun]) -> f32 {
    let (mut cur, mut max) = (0.0f32, 0.0f32);
    let bump = |cur: &mut f32, max: &mut f32| {
        *max = max.max(*cur);
        *cur = 0.0;
    };
    for run in runs {
        match run {
            InlineRun::Text { text, style } => {
                let size = style.font_size.unwrap_or(14.0);
                cur += text
                    .chars()
                    .map(|c| if c.is_ascii() { 0.6 } else { 1.0 })
                    .sum::<f32>()
                    * size;
            }
            InlineRun::Image { style, .. } => cur += image_width_or_default(style),
            InlineRun::Link { runs, .. } => cur += runs_max_width(runs),
            InlineRun::Br => bump(&mut cur, &mut max),
        }
    }
    bump(&mut cur, &mut max);
    max
}

/// 图片宽度：声明宽（px）优先，无声明按占位框典型宽 96px。
fn image_width_or_default(style: &ImageStyle) -> f32 {
    match style.width {
        Some(Width::Px(v)) => v,
        _ => 96.0,
    }
}

/// 图片来源终审白名单（spec §4.4）：
///
/// - http/https → [`ImageRef::Remote`]（UI 层决定是否加载，默认拦截）；
/// - `cid:` → [`ImageRef::Cid`]（content_id 小写化）；
/// - `data:image/*;base64,…` → [`ImageRef::DataUri`]（解 base64；容忍邮件换行
///   包装的空白；无 `;base64` 标记或解码失败按 [`ImageRef::Invalid`] 占位）；
/// - 其余（`data:text/html` 等非图片 MIME、其它协议）→ [`ImageRef::Invalid`]。
fn classify_image_src(raw: &str) -> ImageRef {
    let src = raw.trim();
    let lowered = src.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return ImageRef::Remote {
            url: src.to_string(),
        };
    }
    if let Some(rest) = lowered.strip_prefix("cid:") {
        return ImageRef::Cid {
            content_id: rest.trim().to_string(),
        };
    }
    // data: 分支用原文切片——base64 载荷大小写敏感，仅 meta 部分小写化
    if lowered.starts_with("data:") {
        let rest = &src["data:".len()..];
        if let Some((meta, payload)) = rest.split_once(',') {
            let mut meta = meta.to_ascii_lowercase();
            let is_base64 = match meta.strip_suffix(";base64") {
                Some(stripped) => {
                    meta = stripped.to_string();
                    true
                }
                None => false,
            };
            if is_base64
                && meta.starts_with("image/")
                && let Some(bytes) = decode_base64_lenient(payload)
            {
                return ImageRef::DataUri {
                    mime: meta,
                    bytes: bytes.into(),
                };
            }
        }
        return ImageRef::Invalid;
    }
    ImageRef::Invalid
}

/// base64 解码（容忍填充缺失与邮件换行包装的空白；字母表外输入返回 None）。
fn decode_base64_lenient(payload: &str) -> Option<Vec<u8>> {
    let cleaned: String = payload
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let engine = GeneralPurpose::new(
        &BASE64_ALPHABET,
        GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    engine.decode(cleaned).ok()
}

/// 行内元素自身的样式贡献：style 属性解析 + 语义标签的呈现语义
/// （`<b>/<strong>` → 粗体、`<i>/<em>` → 斜体、`<u>` → 下划线、
/// `<s>/<strike>/<del>` → 删除线；ammonia 白名单保留它们正是为了语义）。
/// 标签语义与 style 属性声明按布尔或叠加（`<u style="text-decoration:none">`
/// 的撤销语义不支持，与 [`InlineStyle`] 布尔叠加语义一致，已知限制）。
///
/// `<font color>`（review I-3）：遗留邮件文字着色的头号来源——color 属性
/// 经 [`parse_color`] 白名单终审后作为前景色兜底（style 声明的 color 优先，
/// 浏览器同序）。size/face 缓期（低频且映射面大）。
fn tag_inline_style(local: &str, style_text: &str, attrs: &[Attribute]) -> InlineStyle {
    let mut style = parse_inline_style(style_text);
    match local {
        "b" | "strong" => style.bold = true,
        "i" | "em" => style.italic = true,
        "u" => style.underline = true,
        "s" | "strike" | "del" => style.strikethrough = true,
        "font" => {
            if style.color.is_none() {
                style.color = attr_str(attrs, "color").and_then(parse_color);
            }
        }
        _ => {}
    }
    style
}

/// img 样式：style 属性的 width/height/max-width（T3 映射），`width`/`height`
/// HTML 属性兜底（邮件图片惯用 `<img width="600">`，ammonia 默认属性表放行）。
fn image_style(attrs: &[Attribute]) -> ImageStyle {
    let block = parse_block_style(attr_str(attrs, "style").unwrap_or(""));
    let mut style = ImageStyle {
        width: block.width,
        height: block.height,
        max_width: block.max_width,
    };
    if style.width.is_none() {
        style.width = attr_str(attrs, "width").and_then(parse_dim_attr);
    }
    if style.height.is_none() {
        style.height = attr_str(attrs, "height").and_then(parse_dim_attr);
    }
    style
}

/// img/td 的 width/height 属性值：无单位数值 / px / `%`；非有限/负值拒绝
/// （T5 备案：`f32::parse` 接受 "NaN"/"inf"，不得进入 IR）。上限钳制同
/// 样式路径（review I-2）：px 幅值 ≤ 100_000、百分比 ≤ 1000。
fn parse_dim_attr(value: &str) -> Option<Width> {
    let finite_nonneg = |v: &f32| v.is_finite() && *v >= 0.0;
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f32>()
            .ok()
            .filter(finite_nonneg)
            .map(|p| Width::Percent(p.min(1000.0)));
    }
    let value = value.strip_suffix("px").unwrap_or(value).trim();
    value
        .parse::<f32>()
        .ok()
        .filter(finite_nonneg)
        .map(|v| Width::Px(v.min(100_000.0)))
}

/// colspan 属性：数值钳制到 1–16（types.rs 契约：默认 1、上限 16，防病理宽表）；
/// 0/负数/非数字 → None（调用方回退默认 1）。
fn parse_colspan(value: &str) -> Option<u8> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|n| n.clamp(1, 16) as u8)
}

/// 用行内 run 物化段落（`style` 为段落自身 BlockStyle；聚合内容无自身样式时
/// 由调用方传 [`BlockStyle::default()`，对齐继承父容器）。
fn paragraph(runs: Vec<InlineRun>, align: Align, style: BlockStyle) -> Block {
    Block::Paragraph { style, runs, align }
}

/// 浮动列构建产物 → [`TableCell`]：单个 Container 解包为格（列宽进 cell.width、
/// 内边距/背景落在格样式上，免双层容器）；其余形态（多块产物）整体入格。
/// 空产物（display:none 剪枝）→ None（该列不参与并排）。
fn float_cell_of(built: Vec<Block>) -> Option<TableCell> {
    if built.is_empty() {
        return None;
    }
    let mut blocks = built;
    if blocks.len() == 1
        && matches!(blocks[0], Block::Container { .. })
        && let Block::Container { style, children } = std::mem::take(&mut blocks).remove(0)
    {
        return Some(TableCell {
            blocks: children,
            colspan: 1,
            rowspan: 1,
            width: style.width,
            style,
        });
    }
    Some(TableCell {
        blocks,
        colspan: 1,
        rowspan: 1,
        width: None,
        style: BlockStyle::default(),
    })
}

/// HTML 可折叠空白字符（空格/制表/换行/回车/换页）。`&nbsp;`（U+00A0）是内容
/// 空白——浏览器不折叠也不删除它，邮件排版大量依赖，必须排除在折叠语义外。
fn is_html_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{0C}')
}

/// 仅按 HTML 可折叠空白 trim（[`str::trim`] 系列按 Unicode whitespace 会误删 NBSP）。
fn trim_html_ws(s: &str) -> &str {
    s.trim_matches(is_html_ws)
}

/// 行内内容聚合缓冲：块级边界之间的连续行内内容，块边界处整体物化为段落。
///
/// `Ws::Fold` 下同时承担空白折叠状态机（HTML 块级上下文语义）：
///
/// - 文本内部连续空白折叠为单空格；
/// - 段首、段尾、强制换行边界的空白删除（行首/行尾空白不渲染）；
/// - 行中独立空白文本节点（如 `<b>x</b> <i>y</i>` 之间的空格）先挂起不发射：
///   与后续文本合并为单空格（归属空白处的样式；上一 run 已以可折叠空白结尾时
///   并入其尾随空白，不产双空格——T5 评审 Minor-1），或在链接/图片前物化——
///   否则跨不同样式 run 的空格会丢失；
/// - `Ws::Preserve`（pre）下所有文本原样直入，不触碰状态。
struct InlineBuffer {
    ws: Ws,
    runs: Vec<InlineRun>,
    /// 段首 / Br 之后：后续文本的前导空白删除（保持到首个实质内容出现）。
    line_start: bool,
    /// 已发射文本以可折叠空白结尾（后续文本的前导空白与之合并）。
    trailing_ws: bool,
    /// 已见未发射的行中空白（遇到非文本 run 前挂起，物化为单空格——上一 run
    /// 已以空白结尾时并入之；段尾消失）。
    pending: Option<InlineStyle>,
}

impl InlineBuffer {
    fn new(ws: Ws) -> Self {
        Self {
            ws,
            runs: Vec::new(),
            line_start: true,
            trailing_ws: false,
            pending: None,
        }
    }

    fn push_text(&mut self, text: &str, style: &InlineStyle) {
        if self.ws == Ws::Preserve {
            if !text.is_empty() {
                self.runs.push(InlineRun::Text {
                    text: text.to_string(),
                    style: style.clone(),
                });
            }
            return;
        }
        if self.line_start {
            // 仅 trim 前导：首个文本节点的尾随空白仍是内容空白（交给折叠状态机）
            let body = text.trim_start_matches(is_html_ws);
            if body.is_empty() {
                return; // 行首纯空白消失（保持行首状态直到首个实质内容）
            }
            self.line_start = false;
            self.emit_folded(body, style);
            return;
        }
        if text.chars().all(is_html_ws) {
            // 行中独立空白节点：挂起（归属空白处的样式）
            if self.pending.is_none() {
                self.pending = Some(style.clone());
            }
            self.trailing_ws = true;
            return;
        }
        // 行中文本：挂起空白物化为单空格；上一 run 已以可折叠空白结尾时并入之
        // 不追加（T5 评审 Minor-1：`<b>a </b> <i>b</i>` 不得产出双空格）
        let pending_style = self.pending.take();
        if let Some(ws_style) = &pending_style
            && !matches!(self.runs.last(), Some(InlineRun::Text { text, .. }) if text.ends_with(' '))
        {
            self.runs.push(InlineRun::Text {
                text: " ".to_string(),
                style: ws_style.clone(),
            });
        }
        let body = if pending_style.is_some() || self.trailing_ws {
            // 新文本的前导空白并入已物化/已存在的空格；尾随空白照常保留
            text.trim_start_matches(is_html_ws)
        } else {
            text
        };
        self.emit_folded(body, style);
    }

    /// 折叠文本内部空白并发射 Text run（前导空白语义由调用方处理）。
    fn emit_folded(&mut self, text: &str, style: &InlineStyle) {
        let mut folded = String::with_capacity(text.len());
        let mut last_ws = false;
        for c in text.chars() {
            if is_html_ws(c) {
                if !last_ws {
                    folded.push(' ');
                }
                last_ws = true;
            } else {
                folded.push(c);
                last_ws = false;
            }
        }
        if folded.is_empty() {
            return;
        }
        self.trailing_ws = folded.ends_with(' ');
        self.runs.push(InlineRun::Text {
            text: folded,
            style: style.clone(),
        });
    }

    fn push_br(&mut self) {
        if self.ws == Ws::Preserve {
            self.runs.push(InlineRun::Br);
            return;
        }
        // 强制换行边界：挂起空格消失；已发射文本的尾随空白删除（行尾空白不渲染）
        self.pending = None;
        if self.trailing_ws
            && let Some(InlineRun::Text { text, .. }) = self.runs.last_mut()
        {
            let trimmed = text.trim_end_matches(is_html_ws);
            if trimmed.is_empty() {
                self.runs.pop();
            } else {
                *text = trimmed.to_owned();
            }
        }
        self.runs.push(InlineRun::Br);
        self.line_start = true;
        self.trailing_ws = false;
    }

    /// 发射非文本 run（Link / Image）：挂起的行中空白物化为单空格（上一 run
    /// 已以可折叠空白结尾时并入之，不追加——与 [`Self::push_text`] 同语义）。
    fn push_wrapped(&mut self, run: InlineRun) {
        if self.ws == Ws::Preserve {
            self.runs.push(run);
            return;
        }
        if let Some(ws_style) = self.pending.take()
            && !matches!(self.runs.last(), Some(InlineRun::Text { text, .. }) if text.ends_with(' '))
        {
            self.runs.push(InlineRun::Text {
                text: " ".to_string(),
                style: ws_style,
            });
        }
        self.line_start = false;
        self.trailing_ws = false;
        self.runs.push(run);
    }

    /// 归位为 run 序列：折叠语义下做段落级规整并丢弃无实质内容的序列；
    /// pre 语义下原样返回。调用后缓冲复位（同缓冲跨块边界复用）。
    fn finish(&mut self) -> Vec<InlineRun> {
        let mut runs = std::mem::take(&mut self.runs);
        self.line_start = true;
        self.trailing_ws = false;
        self.pending = None;
        if self.ws == Ws::Fold {
            normalize_runs(&mut runs);
            if !runs_meaningful(&runs) {
                runs.clear();
            }
        }
        runs
    }

    /// 物化为块序列：聚合内容整体物化为单一段落（行内图片保留在 runs 内，
    /// 渲染层 wrap-flow 横向消费）；pre 语义下整段原样物化。聚合内容无自身
    /// 块级元素，段落样式取默认（对齐继承父容器）。
    fn flush_blocks(&mut self, align: Align) -> Vec<Block> {
        let runs = self.finish();
        if runs.is_empty() {
            return Vec::new();
        }
        vec![paragraph(runs, align, BlockStyle::default())]
    }
}

/// 段落级行内规整：同样式相邻 Text run 合并、修剪段首/段尾空白、丢弃空 Text run。
///
/// 样式不同的相邻 Text 不合并（跨 run 样式叠加后的保真前提）；跨 run 的空白折叠
/// 由 [`InlineBuffer`] 状态机在收集期完成，此处只做兜底修剪。
fn normalize_runs(runs: &mut Vec<InlineRun>) {
    let mut merged: Vec<InlineRun> = Vec::with_capacity(runs.len());
    for run in runs.drain(..) {
        match (merged.last_mut(), run) {
            (
                Some(InlineRun::Text {
                    text: prev,
                    style: prev_style,
                }),
                InlineRun::Text {
                    text: next,
                    style: next_style,
                },
            ) if *prev_style == next_style => prev.push_str(&next),
            (_, run) => merged.push(run),
        }
    }
    // 段首仅 trim 前导、段尾仅 trim 尾随——首个/末个 run 另一侧的空白是内容
    if let Some(InlineRun::Text { text, .. }) = merged.first_mut() {
        *text = text.trim_start_matches(is_html_ws).to_owned();
    }
    if let Some(InlineRun::Text { text, .. }) = merged.last_mut() {
        *text = text.trim_end_matches(is_html_ws).to_owned();
    }
    merged.retain(|run| !matches!(run, InlineRun::Text { text, .. } if text.is_empty()));
    *runs = merged;
}

/// 是否存在实质内容（非空白文本、Br 或图片）；纯空白序列不成段。
fn runs_meaningful(runs: &[InlineRun]) -> bool {
    runs.iter().any(|run| match run {
        InlineRun::Br | InlineRun::Image { .. } => true,
        InlineRun::Text { text, .. } => !trim_html_ws(text).is_empty(),
        InlineRun::Link { runs, .. } => runs_meaningful(runs),
    })
}

/// 按局部名取属性值（HTML 局部名大小写不敏感由解析器归一化保证）。
fn attr_str<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|attr| &*attr.name.local == name)
        .map(|attr| &*attr.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::HtmlError;
    use crate::types::{Align, ImageRef, ImageStyle, SafeUrl, Width};

    fn doc(html: &str) -> HtmlDocument {
        build_document(html, &Limits::default()).unwrap()
    }

    /// serde_json 序列化（安全断言用：危险协议不得出现在任何字段）。
    fn json(doc: &HtmlDocument) -> String {
        serde_json::to_string(doc).unwrap()
    }

    /// 拼接 run 文本（Br 记为 `\n` 便于断言）。
    fn run_text(runs: &[InlineRun]) -> String {
        runs.iter()
            .map(|run| match run {
                InlineRun::Text { text, .. } => text.as_str(),
                InlineRun::Br => "\n",
                _ => "",
            })
            .collect()
    }

    /// 段落/标题的文本内容。
    fn text_of(block: &Block) -> String {
        match block {
            Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => run_text(runs),
            _ => String::new(),
        }
    }

    /// 块序列中是否含指定文本的段落（含容器/列表/引用递归）。
    fn contains_text(blocks: &[Block], text: &str) -> bool {
        blocks.iter().any(|block| match block {
            Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => run_text(runs) == text,
            Block::Container { children, .. } | Block::Quote { children } => {
                contains_text(children, text)
            }
            Block::List { items, .. } => items.iter().any(|item| contains_text(item, text)),
            _ => false,
        })
    }

    // ---- brief 测试（逐字采用）----

    #[test]
    fn javascript_link_downgrades_to_text() {
        let doc = doc(r#"<a href="javascript:alert(1)">点我</a>"#);
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        // Link 节点不存在；文字保留为纯 Text run
        assert_eq!(run_text(runs), "点我");
        assert!(runs.iter().all(|r| matches!(r, InlineRun::Text { .. })));
    }

    #[test]
    fn entity_encoded_js_scheme_rejected() {
        // html5ever 已实体解码 → 树构建收到解码后的 javascript: → 同样拒绝
        let doc = build_document(
            r#"<a href="javascript&#58;alert(1)">x</a>"#,
            &Limits::default(),
        )
        .unwrap();
        assert!(!json(&doc).contains("javascript"));
    }

    #[test]
    fn data_text_image_rejected_but_text_kept() {
        let doc = doc(r#"<img src="data:text/html,<b>x</b>" alt="替代文字">"#);
        assert_eq!(doc.blocks.len(), 1);
        // 行内图片保留在段落 runs 内（线上反馈③第二波：块级提升拆散图文混排流）
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("img 应保留为段落内行内 run")
        };
        let [InlineRun::Image { src, alt, .. }] = runs.as_slice() else {
            panic!("应为单一 Image run")
        };
        assert_eq!(*src, ImageRef::Invalid);
        assert_eq!(alt, "替代文字");
    }

    #[test]
    fn cid_and_data_image_preserved() {
        let doc =
            doc(r#"<img src="cid:Image001@ABC"><img src="data:image/png;base64,iVBORw0KGgo=">"#);
        // 两枚行内图并入同一段落（wrap-flow 横向消费），不再逐图成块
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 2);
        let InlineRun::Image {
            src: ImageRef::Cid { content_id },
            ..
        } = &runs[0]
        else {
            panic!("应为 Cid")
        };
        assert_eq!(content_id, "image001@abc"); // content_id 小写化
        let InlineRun::Image {
            src: ImageRef::DataUri { mime, bytes },
            ..
        } = &runs[1]
        else {
            panic!("应为 DataUri")
        };
        assert_eq!(mime, "image/png");
        // iVBORw0KGgo= 解码即 PNG 文件签名
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn mailto_and_https_preserved() {
        let doc = doc(
            r#"<a href="mailto:a@b.example">m</a><a href="https://example.com/x?y=1">w</a><a href="HTTP://EXAMPLE.COM">u</a>"#,
        );
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(
            runs[0],
            InlineRun::Link {
                href: SafeUrl::Mailto("mailto:a@b.example".to_string()),
                runs: vec![InlineRun::Text {
                    text: "m".to_string(),
                    style: InlineStyle::default()
                }]
            }
        );
        assert_eq!(
            runs[1],
            InlineRun::Link {
                href: SafeUrl::Http("https://example.com/x?y=1".to_string()),
                runs: vec![InlineRun::Text {
                    text: "w".to_string(),
                    style: InlineStyle::default()
                }]
            }
        );
        // 协议大小写不敏感、原值保留
        assert_eq!(
            runs[2],
            InlineRun::Link {
                href: SafeUrl::Http("HTTP://EXAMPLE.COM".to_string()),
                runs: vec![InlineRun::Text {
                    text: "u".to_string(),
                    style: InlineStyle::default()
                }]
            }
        );
    }

    // ---- 分类器单元测试 ----

    #[test]
    fn classify_url_whitelist() {
        assert_eq!(
            classify_url("https://e/x"),
            Some(SafeUrl::Http("https://e/x".to_string()))
        );
        // 前导/尾随空白 trim、大小写不敏感
        assert_eq!(
            classify_url("  HTTPS://e/x  "),
            Some(SafeUrl::Http("HTTPS://e/x".to_string()))
        );
        assert_eq!(
            classify_url("\nmailto:a@b\n"),
            Some(SafeUrl::Mailto("mailto:a@b".to_string()))
        );
        // 白名单外一律拒绝（含实体解码后的混淆形态——到达时已解码）
        assert_eq!(classify_url("javascript:alert(1)"), None);
        assert_eq!(classify_url("JAVASCRIPT:alert(1)"), None);
        assert_eq!(classify_url(" javascript:alert(1)"), None);
        assert_eq!(classify_url("data:text/html,<b>x</b>"), None);
        assert_eq!(classify_url("vbscript:x"), None);
        assert_eq!(classify_url("//evil.example"), None);
        assert_eq!(classify_url("relative/path.html"), None);
        assert_eq!(classify_url(""), None);
    }

    #[test]
    fn classify_image_src_whitelist() {
        assert_eq!(
            classify_image_src("https://e/i.png"),
            ImageRef::Remote {
                url: "https://e/i.png".to_string()
            }
        );
        assert_eq!(
            classify_image_src("  HTTP://e/i.PNG "),
            ImageRef::Remote {
                url: "HTTP://e/i.PNG".to_string()
            }
        );
        assert_eq!(
            classify_image_src("cid:AbC@X"),
            ImageRef::Cid {
                content_id: "abc@x".to_string()
            }
        );
        // 非图片 MIME / 无 base64 标记 / 解码失败 / 其它协议 → Invalid
        assert_eq!(
            classify_image_src("data:text/html,<b>x</b>"),
            ImageRef::Invalid
        );
        assert_eq!(
            classify_image_src("data:image/png,iVBOR"),
            ImageRef::Invalid
        );
        assert_eq!(
            classify_image_src("data:image/png;base64,!!!"),
            ImageRef::Invalid
        );
        assert_eq!(classify_image_src("javascript:alert(1)"), ImageRef::Invalid);
        assert_eq!(classify_image_src(""), ImageRef::Invalid);
        // 容忍邮件换行包装的 base64 与缺失填充
        assert!(matches!(
            classify_image_src("data:image/jpeg;base64,iVBO\n\tRWw"),
            ImageRef::DataUri { .. }
        ));
    }

    // ---- 空白折叠（T5 评审①）----

    #[test]
    fn whitespace_folds_in_block_context() {
        // 文本内部连续空白 → 单空格；段首尾 trim
        let doc = doc("<p>  a\n   b  </p>");
        assert_eq!(text_of(&doc.blocks[0]), "a b");
        // 跨行内元素边界的空白折叠为单空格（样式不同也不丢空格）
        let doc = build_document("<div>a  <b>b</b>  <i>c</i></div>", &Limits::default()).unwrap();
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(text_of(&children[0]), "a b c");
        // 行中独立空白文本节点
        let doc =
            build_document("<p><span>x</span> <span>y</span></p>", &Limits::default()).unwrap();
        assert_eq!(text_of(&doc.blocks[0]), "x y");
    }

    #[test]
    fn nbsp_is_content_not_folded() {
        // &nbsp;（U+00A0）是内容空白：不折叠、不删除
        let doc = doc("<p>a&nbsp;&nbsp;b</p>");
        assert_eq!(text_of(&doc.blocks[0]), "a\u{A0}\u{A0}b");
    }

    #[test]
    fn whitespace_around_br_removed() {
        // 强制换行边界的空白删除（行尾/行首空白不渲染，与浏览器一致）
        let doc = doc("<p>a <br> b</p>");
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 3);
        assert_eq!(run_text(runs), "a\nb");
    }

    #[test]
    fn pending_whitespace_merges_into_trailing_ws() {
        // T5 评审 Minor-1：前文尾随空白 + 独立空白节点不产双空格——物化挂起空白
        // 时上一 run 已以可折叠空白结尾则并入之（注释宣称的合并分支）
        let doc = doc("<p><b>a </b> <i>b</i></p>");
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 2);
        assert_eq!(run_text(runs), "a b");
        // 链接前的物化同语义：不追加空格 run（渲染文本 "a l"，非 "a  l"）
        let doc = build_document(
            r#"<p><b>a </b> <a href="https://e">l</a></p>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 2);
        assert!(matches!(runs[0], InlineRun::Text { .. }));
        assert!(matches!(runs[1], InlineRun::Link { .. }));
    }

    #[test]
    fn pre_preserves_whitespace_verbatim() {
        let doc = doc("<pre>  a\n    b  </pre>");
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("pre 应降级为 Container")
        };
        assert_eq!(children.len(), 1);
        assert_eq!(text_of(&children[0]), "  a\n    b  ");
    }

    // ---- 行内扁平化与样式叠加 ----

    #[test]
    fn inline_styles_accumulate_through_nesting() {
        let doc = doc(r#"<span style="color:red"><b>rb</b> red</span>"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { text, style } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(text, "rb");
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        assert!(style.bold);
        let InlineRun::Text { text, style } = &runs[1] else {
            panic!("应为 Text")
        };
        assert_eq!(text, " red");
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        assert!(!style.bold);
    }

    #[test]
    fn same_style_adjacent_text_merges() {
        // 同样式相邻 Text 合并
        let doc = doc("<b>a</b><b>b</b>");
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 1);
        assert_eq!(run_text(runs), "ab");
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert!(style.bold);
        // 样式不同的相邻 Text 不合并
        let doc = build_document(
            r#"<span style="color:red">a</span><span style="color:blue">b</span>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 2);
        assert_eq!(run_text(runs), "ab");
    }

    #[test]
    fn block_color_inherits_to_runs() {
        // p：块级 color 作为 runs base
        let doc = doc(r#"<p style="color:red">x</p>"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        // h1–h6 同通道
        let doc =
            build_document(r#"<h2 style="color:#336699">t</h2>"#, &Limits::default()).unwrap();
        let Block::Heading { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Heading")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([51, 102, 153, 255]));
        // 容器 color 继承到裸文本与行内子元素
        let doc = build_document(
            r#"<div style="color:red">a <b>b</b></div>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { text, style } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(text, "a ");
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        let InlineRun::Text { style, .. } = &runs[1] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        assert!(style.bold);
    }

    #[test]
    fn nested_inline_style_overrides_block_color() {
        // 行内 color 覆盖块级 color（merge_inline：over 有值覆盖）
        let doc = doc(r#"<p style="color:red"><span style="color:green">g</span>s</p>"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([0, 128, 0, 255]));
        let InlineRun::Text { style, .. } = &runs[1] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
    }

    #[test]
    fn link_keeps_nested_styled_runs_and_trims() {
        let doc = doc(r#"<a href="https://e"><b>b</b>old </a>"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Link {
            href: SafeUrl::Http(url),
            runs: inner,
        } = &runs[0]
        else {
            panic!("应为 Link")
        };
        assert_eq!(url, "https://e");
        assert_eq!(run_text(inner), "bold"); // 链接内文本照常 trim
        let InlineRun::Text { style, .. } = &inner[0] else {
            panic!("应为 Text")
        };
        assert!(style.bold);
    }

    // ---- 对齐（T5 评审③）----

    #[test]
    fn align_attribute_style_priority_and_heading() {
        // p 的 align 属性
        let doc = doc(r#"<p align="center">x</p>"#);
        let Block::Paragraph { align, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(*align, Align::Center);
        // style 优先于属性
        let doc = build_document(
            r#"<div align="center" style="text-align:left"><p>a</p></div>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Container {
            style, children, ..
        } = &doc.blocks[0]
        else {
            panic!("应为 Container")
        };
        assert_eq!(style.text_align, Some(Align::Left));
        let Block::Paragraph { align, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(*align, Align::Left);
        // heading 对齐（T5 增补字段）+ 属性兜底
        let doc = build_document(r#"<h1 align="center">h</h1>"#, &Limits::default()).unwrap();
        let Block::Heading { align, .. } = &doc.blocks[0] else {
            panic!("应为 Heading")
        };
        assert_eq!(*align, Align::Center);
        // table 的 align 同样生效（T6 起由 Block::Table 的 style 承载，此前降级 Container）
        let doc = build_document(
            r#"<table align="center"><tr><td>c</td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Table { style, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(style.text_align, Some(Align::Center));
    }

    // ---- 表格构建（T6）----

    #[test]
    fn basic_two_row_table() {
        // table>tr>td*2 → Table{rows:[Row[Cell,Cell], Row[Cell,Cell]]}
        //（html5ever 隐式补的 tbody 在行遍历中透明）
        let doc = doc("<table><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>");
        assert_eq!(doc.blocks.len(), 1);
        let Block::Table { rows, style } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(*style, BlockStyle::default());
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert_eq!(row.cells.len(), 2);
        }
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "a");
        assert_eq!(text_of(&rows[0].cells[1].blocks[0]), "b");
        assert_eq!(text_of(&rows[1].cells[0].blocks[0]), "c");
        assert_eq!(text_of(&rows[1].cells[1].blocks[0]), "d");
    }

    #[test]
    fn colspan_recorded() {
        // <td colspan="3"> → TableCell{colspan:3}
        let doc = doc(r#"<table><tr><td colspan="3">a</td><td>b</td></tr></table>"#);
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].colspan, 3);
        assert_eq!(rows[0].cells[1].colspan, 1);
        // 畸形值回退默认 1；超出上限钳制 16（types.rs 契约：默认 1、上限 16）
        let doc = build_document(
            r#"<table><tr><td colspan="x">a</td><td colspan="0">b</td><td colspan="99">c</td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].colspan, 1);
        assert_eq!(rows[0].cells[1].colspan, 1);
        assert_eq!(rows[0].cells[2].colspan, 16);
    }

    #[test]
    fn nested_table_recurses_into_cell() {
        // td 内 table → cell.blocks 含 Table（嵌套表格走同一条构建路径）
        let doc = doc(
            r#"<table><tr><td><table><tr><td>in</td></tr></table></td><td>out</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cells.len(), 2);
        let Block::Table { rows: inner, .. } = &rows[0].cells[0].blocks[0] else {
            panic!("cell.blocks 应含嵌套 Table")
        };
        assert_eq!(inner.len(), 1);
        assert_eq!(text_of(&inner[0].cells[0].blocks[0]), "in");
        assert_eq!(text_of(&rows[0].cells[1].blocks[0]), "out");
    }

    #[test]
    fn rowspan_places_spanned_cells_in_column() {
        // 线上反馈②：rowspan 归一化——span 覆盖格位生成空占位，后续行单元格
        // 回到正确列位（此前 span 行少一格整体左移，「续费价格」列显示成日期）
        let doc =
            doc(r#"<table><tr><td rowspan="2">a</td><td>b</td></tr><tr><td>c</td></tr></table>"#);
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells.len(), 2);
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "a");
        assert_eq!(rows[0].cells[0].rowspan, 2);
        // 第二行：首格是占位（空 blocks），c 落在第二列
        assert_eq!(rows[1].cells.len(), 2);
        assert!(rows[1].cells[0].blocks.is_empty(), "span 覆盖格位应是空占位");
        assert_eq!(text_of(&rows[1].cells[1].blocks[0]), "c");
    }

    #[test]
    fn rowspan_colspan_mixed_grid() {
        // rowspan=2 × colspan=2 的 span：覆盖格位生成空占位，真实单元格顺延到
        // 正确列位（行 0 = [span, b]，行 1 = [占, 占, d, e]）
        let doc = doc(
            r#"<table><tr><td rowspan="2" colspan="2">span</td><td>b</td></tr><tr><td>d</td><td>e</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells.len(), 2);
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "span");
        assert_eq!(text_of(&rows[0].cells[1].blocks[0]), "b");
        assert_eq!(rows[1].cells.len(), 4);
        assert!(rows[1].cells[0].blocks.is_empty() && rows[1].cells[1].blocks.is_empty());
        assert_eq!(text_of(&rows[1].cells[2].blocks[0]), "d");
        assert_eq!(text_of(&rows[1].cells[3].blocks[0]), "e");
    }

    #[test]
    fn table_border_attr_draws_grid() {
        // border="1"：collapse 语义网格线——每格 top/left、尾列 right、末行 bottom；
        // 占位格不画 span 内部横线；bordercolor 覆盖默认灰
        let bordered = doc(
            r##"<table border="1" bordercolor="#ff0000"><tr><td>a</td><td>b</td></tr><tr><td>c</td><td>d</td></tr></table>"##,
        );
        let Block::Table { rows, .. } = &bordered.blocks[0] else {
            panic!("应为 Table")
        };
        let line = crate::types::Border { width: 1.0, color: [255, 0, 0, 255] };
        // 首行首格：top+left（非尾列无 right、非末行无 bottom）
        assert_eq!(rows[0].cells[0].style.border_top, Some(line));
        assert_eq!(rows[0].cells[0].style.border_left, Some(line));
        assert_eq!(rows[0].cells[0].style.border_right, None);
        assert_eq!(rows[0].cells[0].style.border_bottom, None);
        // 首行尾格：top+left+right
        assert_eq!(rows[0].cells[1].style.border_right, Some(line));
        // 末行格：bottom 兜底
        assert_eq!(rows[1].cells[0].style.border_bottom, Some(line));
        assert_eq!(rows[1].cells[1].style.border_bottom, Some(line));
        // border="0" / 无属性 → 无网格线
        let plain = doc(r#"<table border="0"><tr><td>a</td></tr></table>"#);
        let Block::Table { rows, .. } = &plain.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].style.border_top, None);
    }

    #[test]
    fn table_cellpadding_applies_when_cell_silent() {
        // cellpadding 施加到无 padding 声明的格；自身声明优先
        let doc = doc(
            r#"<table cellpadding="6"><tr><td>a</td><td style="padding:2px">b</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].style.padding, [6.0; 4]);
        assert_eq!(rows[0].cells[1].style.padding, [2.0; 4]);
    }

    #[test]
    fn column_widths_propagate_across_rows() {
        // 列宽协调：表头行声明的列宽传播到数据行同列（colspan=1 逐列；
        // colspan>1 的声明不拆列）。渲染层逐行独立 flex，无协调则竖线错位
        let doc = doc(
            r#"<table><tr><td width="30%">h1</td><td width="70%">h2</td></tr><tr><td>a</td><td>b</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[1].cells[0].width, Some(Width::Percent(30.0)));
        assert_eq!(rows[1].cells[1].width, Some(Width::Percent(70.0)));
    }

    #[test]
    fn cell_height_attr_fallback() {
        // 高度：style 优先、height 属性兜底（单元格定高是遗留邮件行高惯用法）
        let doc =
            doc(r#"<table><tr><td height="40" style="height:20px">a</td><td height="30">b</td></tr></table>"#);
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].style.height, Some(Width::Px(20.0)));
        assert_eq!(rows[0].cells[1].style.height, Some(Width::Px(30.0)));
    }

    #[test]
    fn content_weighted_column_widths_align_rows() {
        // 线上反馈②第二波：无声明宽度的表按各列最大内容宽度比例分配百分比，
        // 表头行与数据行同列同宽（渲染层逐行独立 flex，无协调则竖线错位）
        let doc = doc(
            "<table><tr><td>域名</td><td>赎回开始时间</td></tr>\
             <tr><td>ddyoudan.com</td><td>2026-11-23 10:22:37</td></tr></table>",
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        let pct = |w| match w {
            Some(Width::Percent(p)) => p,
            other => panic!("未声明列应分得百分比宽度，实际 {other:?}"),
        };
        let (h0, h1) = (pct(rows[0].cells[0].width), pct(rows[0].cells[1].width));
        // 行间共线：同列同宽
        assert_eq!(rows[1].cells[0].width, rows[0].cells[0].width);
        assert_eq!(rows[1].cells[1].width, rows[0].cells[1].width);
        // 比例按内容：col0 权重 = max(2×14, 12×0.6×14) = 100.8；
        // col1 = max(6×14, 19×0.6×14) = 159.6 → 38.7% / 61.3%（±0.5）
        assert!((h0 - 38.7).abs() < 0.5, "col0 应约 38.7%，实际 {h0}");
        assert!((h1 - 61.3).abs() < 0.5, "col1 应约 61.3%，实际 {h1}");
        assert!((h0 + h1 - 100.0).abs() < 0.01, "自由空间应全部分配");
    }

    #[test]
    fn content_weights_yield_to_declared_and_colspan() {
        // 已声明列宽锁定（声明值传播，估权不覆盖）
        let declared = doc(
            r#"<table><tr><td width="30%">域名</td><td>日期</td></tr><tr><td width="30%">x.com</td><td>2026-11-23</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &declared.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].width, Some(Width::Percent(30.0)));
        let Some(Width::Percent(free)) = rows[0].cells[1].width else {
            panic!("未声明列应分得剩余百分比")
        };
        assert_eq!(free, 70.0, "已声明 30% 后剩余 70% 全归未声明列");

        // 含跨列格的表跳过估权（跨列格 grow 权重语义保留，未声明列不产百分比）
        let spanned = doc(
            r#"<table><tr><td colspan="2">跨列头</td></tr><tr><td>a</td><td>b</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &spanned.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[1].cells[0].width, None, "含 colspan 的表不做估权分配");
    }

    #[test]
    fn font_color_attr_maps_to_run_color() {
        // review I-3：<font color> 是遗留邮件文字着色的头号来源——color 属性
        // 经 parse_color 终审映射为前景色；style 声明的 color 优先（浏览器同序）
        let red = doc(r#"<font color="red">红字</font>"#);
        let Block::Paragraph { runs, .. } = &red.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        // style 声明覆盖属性
        let blue = doc(r##"<font color="red" style="color:blue">蓝字</font>"##);
        let Block::Paragraph { runs, .. } = &blue.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([0, 0, 255, 255]));
        // 非法色值安全丢弃
        let invalid = doc(r#"<font color="javascript:alert(1)">字</font>"#);
        let Block::Paragraph { runs, .. } = &invalid.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, None);
    }

    #[test]
    fn inline_background_carries_to_runs() {
        // review M-2：行内底色（span/mark 高亮、按钮式菜单项底色）此前在
        // 行内通道整体丢失——style 声明的纯色背景随行内样式叠加进 runs
        let yellow = doc(r#"<span style="background:yellow">高亮</span>尾随"#);
        let Block::Paragraph { runs, .. } = &yellow.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.background, Some([255, 255, 0, 255]));
        let InlineRun::Text { style, .. } = &runs[1] else {
            panic!("应为 Text")
        };
        assert_eq!(style.background, None, "底色不外溢到兄弟 run");
        // url() 资源向量照旧丢弃，仅纯色生效
        let url_case = doc(r#"<span style="background:url(https://e/x.png) aqua">底</span>"#);
        let Block::Paragraph { runs, .. } = &url_case.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.background, Some([0, 255, 255, 255]));  // aqua
    }

    #[test]
    fn text_before_float_keeps_document_order() {
        // review I-4：「文字A float 文字B」此前 A 留在行内缓冲被整体搬到
        // float 表之后（[表][A+B]）；先 flush 保住 [A][表][B] 文档序
        let doc = doc(
            r#"<div>A<div style="float:left;width:30%">F</div>B</div>"#,
        );
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(children.len(), 3, "[段A, float表, 段B]");
        assert_eq!(text_of(&children[0]), "A");
        assert!(matches!(children[1], Block::Table { .. }));
        assert_eq!(text_of(&children[2]), "B");
    }

    #[test]
    fn float_wins_over_inline_display() {
        // review M-3：float 与 display:inline-block 并存时浏览器语义是 float
        // 优先（列宽保留在 cell 上，不再走行内透传丢宽）
        let doc = doc(
            r#"<div><div style="float:left;display:inline-block;width:40%">L</div><div style="float:right;width:40%">R</div></div>"#,
        );
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        let Block::Table { rows, .. } = &children[0] else {
            panic!("float 应优先入列组（单行 Table），实际 {:?}", children[0])
        };
        assert_eq!(rows[0].cells.len(), 2);
        assert_eq!(rows[0].cells[0].width, Some(Width::Percent(40.0)));
    }

    #[test]
    fn overdeclared_percent_widths_normalized() {
        // review M-6：声明百分比总和 >100% 按比例归一（浏览器语义）——
        // 此前未声明列自由空间为 0 塌成 ~0 宽
        let overdeclared = doc(
            r#"<table><tr><td width="60%">a</td><td width="60%">b</td><td>c</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &overdeclared.blocks[0] else {
            panic!("应为 Table")
        };
        let pct = |w: Option<Width>| match w {
            Some(Width::Percent(p)) => p,
            other => panic!("应为百分比宽度: {other:?}"),
        };
        let c0 = pct(rows[0].cells[0].width);
        let c1 = pct(rows[0].cells[1].width);
        assert!((c0 - 50.0).abs() < 0.5, "60/120*100=50，实际 {c0}");
        assert!((c1 - 50.0).abs() < 0.5, "实际 {c1}");
        // 归一后总和恰 100：未声明列 free=0 → 估权不赋 Percent（None 走
        // 等分 grow，不再被 Percent(0) 锁死塌缩）
        assert_eq!(rows[0].cells[2].width, None);
        // 总和 ≤100 的常规表不受影响
        let normal = doc(r#"<table><tr><td width="30%">a</td><td>b</td></tr></table>"#);
        let Block::Table { rows, .. } = &normal.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].width, Some(Width::Percent(30.0)));
    }

    #[test]
    fn floated_columns_merge_into_single_row() {
        // 线上反馈③第三波（banner float 列）：相邻 float:left|right 兄弟水平
        // 并排为单行 Table（列宽保留在 cell.width）；非 float 内容回到纵向流
        let floated = doc(
            r#"<div><div style="float:left;width:20%"><a href="https://e/logo"><img src="https://e/logo.png" alt="logo"></a></div><div style="float:right;width:70%;padding-left:10%"><a href="https://e/a">产品</a> <a href="https://e/b">定价</a></div><p>after</p></div>"#,
        );
        let Block::Container { children, .. } = &floated.blocks[0] else {
            panic!("应为 Container")
        };
        // [float 并排行, after 段落]
        assert_eq!(children.len(), 2, "float 组 + 后续段落");
        let Block::Table { rows, .. } = &children[0] else {
            panic!("float 组应物化为单行 Table，实际 {:?}", children[0])
        };
        assert_eq!(rows.len(), 1);
        let cells = &rows[0].cells;
        assert_eq!(cells.len(), 2, "两个浮动列并排");
        assert_eq!(cells[0].width, Some(Width::Percent(20.0)));
        assert_eq!(cells[1].width, Some(Width::Percent(70.0)));
        // 第二列的 padding 声明保留在格样式上（百分比槽位按已知限制记 0）
        assert_eq!(cells[1].style.padding, [0.0, 0.0, 0.0, 0.0]);
        // float:none 不入组（保持块级）
        let none_float = doc(r#"<div><div style="float:none;width:20%">a</div><p>b</p></div>"#);
        let Block::Container { children, .. } = &none_float.blocks[0] else {
            panic!("应为 Container")
        };
        assert!(matches!(children[0], Block::Container { .. }), "float:none 应保持块级");
    }

    #[test]
    fn inline_images_stay_in_paragraph_runs() {        // 线上反馈③第二波（banner 竖排的真因）：行内图片与文字/链接同段，
        // 不再提升为块级——「链接 + 分隔小图」混排的横向菜单保持单一流
        let doc = doc(
            r#"<div><a href="https://e/a">产品</a><img src="https://e/s.gif" width="5" height="28" alt=""><a href="https://e/b">定价</a></div>"#,
        );
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        let [Block::Paragraph { runs, .. }] = &children[..] else {
            panic!("图文混排应物化为单一段落，实际块数 {}", children.len())
        };
        let kinds: Vec<&str> = runs
            .iter()
            .map(|run| match run {
                InlineRun::Link { .. } => "link",
                InlineRun::Image { .. } => "img",
                _ => "text",
            })
            .collect();
        assert_eq!(kinds, vec!["link", "img", "link"]);
    }

    #[test]
    fn inline_block_div_joins_inline_flow() {
        // 线上反馈③：块级标签 + display:inline-block（邮件横向菜单主流写法）
        // 按行内透传并入父级行内流——同一物化段落、链接保持可点，不再逐项竖排
        let doc = doc(
            r#"<div><div style="display:inline-block"><a href="https://e/a">产品</a></div> <div style="display:inline-block"><a href="https://e/b">定价</a></div></div>"#,
        );
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        // 两个 inline-block 项并入父级同一行内段落（竖排回归的核心断言）
        let [Block::Paragraph { runs, .. }] = &children[..] else {
            panic!("inline-block 项应并入唯一行内段落，实际块数: {}", children.len())
        };
        let flow: Vec<String> = runs
            .iter()
            .map(|run| match run {
                InlineRun::Text { text, .. } => text.clone(),
                InlineRun::Link { runs, .. } => run_text(runs),
                _ => String::new(),
            })
            .collect();
        assert_eq!(flow, vec!["产品".to_string(), " ".to_string(), "定价".to_string()]);
        let links: Vec<_> = runs
            .iter()
            .filter_map(|run| match run {
                InlineRun::Link { href, .. } => Some(href.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(links.len(), 2, "两个 inline-block 菜单项都应保留为链接");
        // 对照：无行内声明的块级 div 仍逐项成块（竖排语义不回归）
        let doc = build_document(
            r#"<div><div style="display:block"><a href="https://e/a">产品</a></div><div><a href="https://e/b">定价</a></div></div>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(children.len(), 2, "块级 div 仍应逐项成块");
    }

    #[test]
    fn inline_block_style_merges_to_runs() {
        // inline-block 透传不是透明丢弃：元素自身的行内样式（色/字号）照常叠加
        let doc = doc(
            r#"<div><span style="display:inline-block;color:red">红项</span>尾随</div>"#,
        );
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
    }

    #[test]
    fn thead_tbody_transparent_and_th_is_cell() {
        // thead/tbody 透明容器（行按文档顺序平铺）；th 与 td 同构为 TableCell
        let doc = doc(
            "<table><thead><tr><th>h</th></tr></thead><tbody><tr><td>a</td></tr></tbody></table>",
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "h");
        assert_eq!(text_of(&rows[1].cells[0].blocks[0]), "a");
    }

    #[test]
    fn cell_width_style_priority_and_attr_fallback() {
        // 单元格宽度：style 优先、width 属性兜底；% 与 px；非有限值拒绝（T5 备案④）
        let doc = doc(
            r#"<table><tr><td width="600" style="width:300px">a</td><td width="50%">b</td><td width="NaN">c</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        let cells = &rows[0].cells;
        assert_eq!(cells[0].width, Some(Width::Px(300.0)));
        assert_eq!(cells[1].width, Some(Width::Percent(50.0)));
        // NaN 属性不入声明集；估权阶段未声明列按内容权重分剩余百分比
        //（50% 已声明 → 剩余 50% 全归该列）
        assert_eq!(cells[2].width, Some(Width::Percent(50.0)));
    }

    #[test]
    fn table_style_and_align_recorded() {
        // 表格自身 style → Table 的 style；align 属性兜底 text_align；
        // 白名单外声明（border-spacing）静默忽略
        let doc = doc(
            r#"<table align="center" style="background-color:#eee;width:600px;border-spacing:2px"><tr><td>c</td></tr></table>"#,
        );
        let Block::Table { style, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(style.background_color, Some([238, 238, 238, 255]));
        assert_eq!(style.width, Some(Width::Px(600.0)));
        assert_eq!(style.text_align, Some(Align::Center));
        assert_eq!(style.border_top, None);
    }

    #[test]
    fn nonempty_paragraph_keeps_own_block_style() {
        // T7 评审发现修复：非空 <p> 的自身 BlockStyle 经 split_image_blocks→
        // paragraph() 保留（此前被换成默认样式，margin/background 丢失）——
        // margin:0 是邮件排版压掉默认段距的高频写法；空段占位行为不变
        let doc = doc(r#"<p style="margin:0">文本</p>"#);
        let Block::Paragraph { style, runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(style.margin, [0.0; 4]);
        assert_eq!(run_text(runs), "文本");
        // 非默认值同样保留：背景与 margin:auto 居中标记不再丢失
        let centered = build_document(
            r#"<p style="background-color:#eeeeee;margin:0 auto">文本</p>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Paragraph { style, .. } = &centered.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(style.background_color, Some([238, 238, 238, 255]));
        assert!(style.margin_auto);
        // 图片混排路径：图文保留在同一段落（wrap-flow 横向消费，线上反馈③
        // 第二波——此前按图片切分为 [段,Image,段]，图文混排被拆散竖排）
        let mixed = build_document(
            r#"<p style="background-color:#eeeeee">a<img src="cid:x" alt="i">b</p>"#,
            &Limits::default(),
        )
        .unwrap();
        assert_eq!(mixed.blocks.len(), 1);
        let Block::Paragraph { style, runs, .. } = &mixed.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(style.background_color, Some([238, 238, 238, 255]));
        assert_eq!(runs.len(), 3, "文本-图片-文本共 3 个 run");
        assert!(matches!(runs[1], InlineRun::Image { .. }));
    }

    #[test]
    fn table_and_cell_width_bgcolor_attr_fallback() {
        // T6 预批准补：table 的 width/bgcolor 属性、td/th 的 bgcolor 属性兜底
        //（style 声明优先；具名色走 parse_color 的基础 16 色表）
        let doc = doc(
            r##"<table width="600" bgcolor="silver"><tr><td bgcolor="#eeeeee">a</td><td style="background-color:teal" bgcolor="red">b</td></tr></table>"##,
        );
        let Block::Table { style, rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(style.width, Some(Width::Px(600.0)));
        assert_eq!(style.background_color, Some([192, 192, 192, 255]));
        assert_eq!(
            rows[0].cells[0].style.background_color,
            Some([238, 238, 238, 255])
        );
        // style 声明优先于属性
        assert_eq!(
            rows[0].cells[1].style.background_color,
            Some([0, 128, 128, 255])
        );
        // 表格 style width 优先于 width 属性
        let styled = build_document(
            r#"<table width="600" style="width:300px"><tr><td>x</td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Table { style, .. } = &styled.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(style.width, Some(Width::Px(300.0)));
    }

    #[test]
    fn block_font_props_become_run_base() {
        // T7 裁定①：块级字体属性族（color/font-size/font-weight/font-style/
        // text-decoration）统一作为后代 runs base——p/div/heading/li 全部块级
        // 一处收口（此前仅 color 单属性 + td/th 双通道）
        let para = doc(
            r#"<p style="font-size:14px;font-weight:bold;font-style:italic;text-decoration:underline">x</p>"#,
        );
        let Block::Paragraph { runs, .. } = &para.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.font_size, Some(14.0));
        assert!(style.bold && style.italic && style.underline);

        // heading 同通道（T4 既有块级 color 行为不变，新增字号等四族）
        let heading = doc(r#"<h3 style="color:#336699;font-size:20px">t</h3>"#);
        let Block::Heading { runs, .. } = &heading.blocks[0] else {
            panic!("应为 Heading")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([51, 102, 153, 255]));
        assert_eq!(style.font_size, Some(20.0));

        // 容器透传到子块；li 自身声明在继承 base 之上叠加
        let nested = doc(
            r#"<div style="color:red;font-size:13px"><p>直</p><ul><li style="font-weight:bold">项</li></ul></div>"#,
        );
        let Block::Container { children, .. } = &nested.blocks[0] else {
            panic!("应为 Container")
        };
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        assert_eq!(style.font_size, Some(13.0));
        let Block::List { items, .. } = &children[1] else {
            panic!("应为 List")
        };
        let Block::Paragraph { runs, .. } = &items[0][0] else {
            panic!("列表项应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([255, 0, 0, 255]));
        assert_eq!(style.font_size, Some(13.0));
        assert!(style.bold);

        // 行内声明照常覆盖 base
        let overridden =
            doc(r#"<div style="font-size:13px"><span style="font-size:16px">x</span></div>"#);
        let Block::Container { children, .. } = &overridden.blocks[0] else {
            panic!("应为 Container")
        };
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.font_size, Some(16.0));
    }

    #[test]
    fn td_style_font_props_become_run_base() {
        // T5 评审捎带②：td/th 的字体属性族（color/font-size/font-weight）作为内容
        // runs 的 base InlineStyle——表格单元格文字样式的主流来源
        let doc = build_document(
            r#"<table><tr><td style="color:#336699;font-size:13px;font-weight:bold">x</td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        let Block::Paragraph { runs, .. } = &rows[0].cells[0].blocks[0] else {
            panic!("cell 内容应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.color, Some([51, 102, 153, 255]));
        assert_eq!(style.font_size, Some(13.0));
        assert!(style.bold);
        // 行内声明经 merge_inline 覆盖 base
        let doc = build_document(
            r#"<table><tr><td style="font-size:13px"><span style="font-size:16px">x</span></td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        let Block::Paragraph { runs, .. } = &rows[0].cells[0].blocks[0] else {
            panic!("cell 内容应为 Paragraph")
        };
        let InlineRun::Text { style, .. } = &runs[0] else {
            panic!("应为 Text")
        };
        assert_eq!(style.font_size, Some(16.0));
    }

    #[test]
    fn hidden_cell_and_row_pruned() {
        // display:none 的单元格/行在树构建期剪枝（types.rs 约定）
        let doc = doc(
            r#"<table><tr><td style="display:none">gone</td><td>kept</td></tr><tr style="display:none"><td>gone</td></tr><tr><td>kept2</td></tr></table>"#,
        );
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cells.len(), 1);
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "kept");
        assert_eq!(text_of(&rows[1].cells[0].blocks[0]), "kept2");
    }

    // ---- 图片（行内保留 / 样式）----

    #[test]
    fn inline_image_stays_in_paragraph() {
        // 线上反馈③第二波：行内 img 不再提升切块——图文同段（wrap-flow 横排）
        let doc = doc(r#"<p>before<img src="https://e/i.png" alt="a">after</p>"#);
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(run_text(runs), "beforeafter");
        let InlineRun::Image { src, alt, .. } = &runs[1] else {
            panic!("runs[1] 应为 Image")
        };
        assert_eq!(
            *src,
            ImageRef::Remote {
                url: "https://e/i.png".to_string()
            }
        );
        assert_eq!(alt, "a");
    }

    #[test]
    fn bare_image_between_paragraphs_flushes_as_flow() {
        // 裸 img 在块级边界间物化为独立段落（runs 内单 Image，wrap-flow 消费）
        let doc = doc(r#"<p>a</p><img src="http://e/i.png"><p>b</p>"#);
        assert_eq!(doc.blocks.len(), 3);
        let Block::Paragraph { runs, .. } = &doc.blocks[1] else {
            panic!("裸 img 应物化为段落")
        };
        assert!(matches!(&runs[0], InlineRun::Image { .. }));
    }

    #[test]
    fn linked_image_stays_in_link_runs() {
        // 链接内的 img 无法越级提升：保留行内形状（渲染层按 Link 内图片处理）
        let doc = doc(r#"<a href="https://e"><img src="https://e/i.png"></a>"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        let InlineRun::Link {
            href: SafeUrl::Http(url),
            runs: inner,
        } = &runs[0]
        else {
            panic!("应为 Link")
        };
        assert_eq!(url, "https://e");
        assert!(matches!(&inner[0], InlineRun::Image { .. }));
    }

    #[test]
    fn img_dimensions_style_priority_and_attr_fallback() {
        // style 优先、width/height 属性兜底（行内 run 形态承载）
        let doc =
            doc(r#"<img src="cid:x" width="600" height="50%" style="width:300px;max-width:100%">"#);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("img 应物化为段落（行内 run 形态）")
        };
        let InlineRun::Image { style, .. } = &runs[0] else {
            panic!("应为 Image run")
        };
        assert_eq!(
            *style,
            ImageStyle {
                width: Some(Width::Px(300.0)),
                height: Some(Width::Percent(50.0)),
                max_width: Some(Width::Percent(100.0)),
            }
        );
        // 无 style 时属性兜底生效
        let fallback = build_document(r#"<img src="cid:x" width="600">"#, &Limits::default()).unwrap();
        let Block::Paragraph { runs, .. } = &fallback.blocks[0] else {
            panic!("img 应物化为段落（行内 run 形态）")
        };
        let InlineRun::Image { style, .. } = &runs[0] else {
            panic!("应为 Image run")
        };
        assert_eq!(style.width, Some(Width::Px(600.0)));
        assert_eq!(style.height, None);
        assert_eq!(style.max_width, None);
    }

    // ---- body 背景（T4 M-4 / T5 评审④）----

    #[test]
    fn body_background_fills_first_container() {
        let doc = doc(r#"<body style="background-color:#f0f0f0"><div><p>a</p></div></body>"#);
        let Block::Container { style, .. } = &doc.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert_eq!(style.background_color, Some([240, 240, 240, 255]));
        // 容器自身背景不被覆盖
        let doc = build_document(
            r#"<body style="background-color:#f0f0f0"><div style="background-color:white"><p>a</p></div></body>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Container { style, .. } = &doc.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert_eq!(style.background_color, Some([255, 255, 255, 255]));
    }

    // ---- T4 既有测试（块级结构回归）----

    #[test]
    fn nested_divs_build_container_tree() {
        let doc = doc(r#"<div style="padding:4px"><div><p>a</p></div></div>"#);
        assert_eq!(doc.blocks.len(), 1);
        let Block::Container { style, children } = &doc.blocks[0] else {
            panic!("顶层应为 Container")
        };
        assert_eq!(style.padding, [4.0; 4]);
        assert_eq!(children.len(), 1);
        let Block::Container { children, .. } = &children[0] else {
            panic!("第二层应为 Container")
        };
        assert_eq!(children.len(), 1);
        let Block::Paragraph { runs, .. } = &children[0] else {
            panic!("最内层应为 Paragraph")
        };
        assert_eq!(run_text(runs), "a");
    }

    #[test]
    fn mixed_content_aggregates_paragraph() {
        // 顺序锁定：P(t1s), P(b), P(t2)——块级子元素切断行内聚合
        let doc = doc("<div>t1<span>s</span><p>b</p>t2</div>");
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(children.len(), 3);
        assert_eq!(text_of(&children[0]), "t1s");
        assert_eq!(text_of(&children[1]), "b");
        assert_eq!(text_of(&children[2]), "t2");
    }

    #[test]
    fn lists_and_headings() {
        let doc = doc("<ul><li>a</li><li>b</li></ul><h2>标题</h2>");
        assert_eq!(doc.blocks.len(), 2);
        let Block::List { ordered, items } = &doc.blocks[0] else {
            panic!("应为 List")
        };
        assert!(!ordered);
        assert_eq!(items.len(), 2);
        assert_eq!(text_of(&items[0][0]), "a");
        assert_eq!(text_of(&items[1][0]), "b");
        let Block::Heading { level, runs, .. } = &doc.blocks[1] else {
            panic!("应为 Heading")
        };
        assert_eq!(*level, 2);
        assert_eq!(run_text(runs), "标题");
    }

    #[test]
    fn display_none_pruned() {
        let html = r#"<div style="display:none"><p>x</p></div>"#;
        assert_eq!(
            build_document(html, &Limits::default()),
            Err(HtmlError::Empty)
        );
    }

    #[test]
    fn unknown_tag_downgrades_to_container() {
        let doc = doc("<mjml-container><p>a</p></mjml-container>");
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("未知标签应降级为 Container")
        };
        assert_eq!(children.len(), 1);
        assert_eq!(text_of(&children[0]), "a");
    }

    #[test]
    fn too_many_nodes_rejected() {
        let html = format!("<div>{}</div>", "<i>x</i>".repeat(20_001));
        let limits = Limits::default();
        assert!(matches!(
            build_document(&html, &limits),
            Err(HtmlError::TooManyNodes(n)) if n > limits.max_nodes
        ));
    }

    #[test]
    fn full_document_reads_body_only() {
        // css-inline 片段补全场景：head 中 style/title 不产生块，正文取自 body
        let html = "<html><head><style>p{color:red}</style><title>t</title>\
                    </head><body><p>正文</p></body></html>";
        let doc = doc(html);
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(text_of(&doc.blocks[0]), "正文");
    }

    #[test]
    fn body_style_script_metadata_skipped() {
        // body 内残留的 script/style/注释整体跳过（连同子树），不进正文
        let doc = doc("<p>a</p><script>alert(1)</script><style>p{}</style><!--c--><p>b</p>");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(text_of(&doc.blocks[0]), "a");
        assert_eq!(text_of(&doc.blocks[1]), "b");
    }

    #[test]
    fn ordered_list_detected() {
        let doc = doc("<ol><li>x</li></ol>");
        let Block::List { ordered, items } = &doc.blocks[0] else {
            panic!("应为 List")
        };
        assert!(*ordered);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn blockquote_builds_quote() {
        let doc = doc("<blockquote><p>quote</p></blockquote>");
        let Block::Quote { children } = &doc.blocks[0] else {
            panic!("应为 Quote")
        };
        assert_eq!(children.len(), 1);
        assert_eq!(text_of(&children[0]), "quote");
    }

    #[test]
    fn table_cell_content_builds_existing_block_tree() {
        // T6 接管：cell 内 blocks 递归走既有块级构建（p/h/div 在单元格内照常成型）
        let doc = doc("<table><tr><td><p>cell</p></td></tr></table>");
        let Block::Table { rows, .. } = &doc.blocks[0] else {
            panic!("应为 Table")
        };
        assert_eq!(rows[0].cells[0].blocks.len(), 1);
        assert_eq!(text_of(&rows[0].cells[0].blocks[0]), "cell");
    }

    #[test]
    fn center_becomes_container_with_center_align() {
        let doc = doc("<center><p>x</p></center>");
        let Block::Container { style, children } = &doc.blocks[0] else {
            panic!("center 应映射 Container")
        };
        assert_eq!(style.text_align, Some(Align::Center));
        assert_eq!(children.len(), 1);
    }

    #[test]
    fn br_becomes_run_in_aggregated_paragraph() {
        // 顶层裸文本 + br 聚合为单段落 [Text, Br, Text]
        let doc = doc("a<br>b");
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(runs.len(), 3);
        assert_eq!(run_text(runs), "a\nb");
    }

    #[test]
    fn hr_becomes_hr_block() {
        let doc = doc("<p>a</p><hr><p>b</p>");
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.blocks[1], Block::Hr);
    }

    #[test]
    fn whitespace_only_document_is_empty() {
        assert_eq!(
            build_document("  \n  ", &Limits::default()),
            Err(HtmlError::Empty)
        );
        assert_eq!(
            build_document("", &Limits::default()),
            Err(HtmlError::Empty)
        );
    }

    #[test]
    fn whitespace_between_blocks_does_not_become_paragraph() {
        // 排版换行产生的纯空白序列不成段
        let doc = doc("<div>\n  <p>a</p>\n  <p>b</p>\n</div>");
        let Block::Container { children, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn nested_display_none_subtree_pruned() {
        // 剪枝作用于任意层级的 display:none 子树（含行内元素）
        let doc = doc("<p>a<span style=\"display:none\">hidden</span>b</p>");
        assert_eq!(doc.blocks.len(), 1);
        let Block::Paragraph { runs, .. } = &doc.blocks[0] else {
            panic!("应为 Paragraph")
        };
        assert_eq!(run_text(runs), "ab");
    }

    #[test]
    fn deep_nesting_is_bounded_not_panicking() {
        // 病理嵌套防护：超出深度上限的子树剪枝（不栈溢出、不产生节点）。
        // debug 构建每层两个未内联栈帧，512 层上限在 libtest 默认栈上余量不足
        // （T4 时已贴近临界，T5 帧内新增空白折叠缓冲后越界）——剪枝语义在显式
        // 大栈线程中验证；release 帧为数百字节量级，生产行为不受影响。
        let html = format!("{}x{}", "<div>".repeat(600), "</div>".repeat(600));
        let doc = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || build_document(&html, &Limits::default()).unwrap())
            .unwrap()
            .join()
            .unwrap();
        assert!(matches!(&doc.blocks[0], Block::Container { .. }));
        assert!(!contains_text(&doc.blocks, "x"));
    }
}
