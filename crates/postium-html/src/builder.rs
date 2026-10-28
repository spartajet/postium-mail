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
//!    | table / tr / td 一族 | Task 6 前按未知标签降级为 Container |
//!    | script / style / head / title 等元数据 | 连同子树整体跳过 |
//!
//! 4. 行内扁平化（Task 5）：行内元素样式经 [`parse_inline_style`] 解析、
//!    [`merge_inline`] 逐层叠加为 run 样式；`<a>` → [`InlineRun::Link`]（href
//!    经 [`classify_url`] 终审白名单，不合格链接整体丢弃、内容降级纯文字）；
//!    `img` → 图片（src 经 [`classify_image_src`] 终审白名单）；段级上下文中
//!    行内 img 提升为 [`Block::Image`]（链接内的 img 保留为 [`InlineRun::Image`]，
//!    无法越级提升）；同样式相邻 Text 合并；
//! 5. 空白折叠（Task 5，HTML 块级上下文语义）：连续空白（空格/制表/换行）折叠
//!    为单空格，段首/段尾与强制换行边界的空白删除；`pre` 例外（内容原样保留）；
//!    `&nbsp;`（U+00A0）属内容空白，不折叠不删除；
//! 6. 继承与对齐：块级 style 的 color 作为后代文本的 base [`InlineStyle`]（行内
//!    样式经 merge 叠加）；对齐 `text-align` 样式优先、`align` 属性兜底；
//! 7. `display:none` 子树剪枝（style 属性先经 [`parse_block_style`] 解析）；
//!    树遍历深度超 [`MAX_TREE_DEPTH`] 的子树静默剪枝；
//! 8. body 的 background-color（style / bgcolor 属性）应用到顶层首个无自身背景
//!    的 Container（T4 M-4 最小合理版本；管线路径 body 壳已被 sanitize 剥除，
//!    仅对 `build_document` 直接调用成立）；
//! 9. 节点计数超过 [`Limits::max_nodes`] → [`HtmlError::TooManyNodes`]；
//!    无有效块 → [`HtmlError::Empty`]。
//!
//! ## Task 6 扩展点
//!
//! table 一族暂按未知标签降级为 Container，Task 6 接管为 [`Block::Table`]。

use html5ever::tendril::TendrilSink;
use html5ever::{Attribute, ParseOpts, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use base64::Engine;
use base64::alphabet::STANDARD as BASE64_ALPHABET;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};

use crate::error::HtmlError;
use crate::style::{merge_inline, parse_align, parse_block_style, parse_color, parse_inline_style};
use crate::types::{
    Align, Block, BlockStyle, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle, Limits,
    Rgba, SafeUrl, Width,
};

/// 树遍历深度上限（含块/行内递归）。
///
/// 病理嵌套防护（与浏览器 ~512 层的防护同数量级）：超出上限的子树剪枝，防止
/// 超深文档打爆渲染栈；上限内的正常嵌套邮件（几十层）不受影响。
///
/// 栈占用说明：每层嵌套消耗 child_blocks + build_element_blocks 两个栈帧，
/// release 构建每帧数百字节量级；debug 构建无栈槽复用、帧显著膨胀，512 层在
/// 默认 2MB 测试线程栈上余量不足——深度剪枝语义的测试在显式大栈线程中执行
/// （见 `deep_nesting_is_bounded_not_panicking`），生产行为以 release 为准。
const MAX_TREE_DEPTH: usize = 512;

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
        "div" => Kind::Block(BlockKind::Container { center: false }),
        "center" => Kind::Block(BlockKind::Container { center: true }),
        // 常见行内元素（img 在行内收集期分类为图片：段级提升为块级、链接内保留行内）
        "span" | "a" | "b" | "strong" | "i" | "em" | "u" | "s" | "strike" | "del" | "ins"
        | "small" | "big" | "sub" | "sup" | "font" | "code" | "tt" | "q" | "cite" | "abbr"
        | "acronym" | "label" | "mark" | "img" => Kind::Inline,
        // 其余（table 一族 / mjml-* 等模板标签）→ 未知标签宽容降级
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
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => inline.push_text(&contents.borrow(), text_base),
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Pass => {
                            // html/body 结构壳：子块直接提升，无自身样式
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
                            blocks.extend(inline.flush_blocks(inherited));
                            let attrs = attrs.borrow();
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
                _ => {} // 注释 / DOCTYPE / PI：跳过
            }
        }
        blocks.extend(inline.flush_blocks(inherited));
        Ok(blocks)
    }

    /// 构建单个块级元素为块序列；`display:none` 子树返回空序列剪枝。
    /// 段落含行内图片时切分为多块（见 [`split_image_blocks`]）。
    /// 构建单个块级元素为块序列；`display:none` 子树返回空序列剪枝。
    /// 段落含行内图片时切分为多块（见 [`split_image_blocks`]）。
    ///
    fn build_element_blocks(
        &mut self,
        node: &Handle,
        kind: BlockKind,
        attrs: &[Attribute],
        depth: usize,
        inherited: Align,
        text_base: &InlineStyle,
    ) -> Result<Vec<Block>, HtmlError> {
        let mut style = parse_block_style(attr_str(attrs, "style").unwrap_or(""));
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
        // 块级 color → 后代文本 base 行内样式（T5 评审②）；其余块级字段不映射行内
        let child_base = merge_inline(
            text_base,
            &InlineStyle {
                color: style.color,
                ..InlineStyle::default()
            },
        );
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
                if runs.is_empty() {
                    // 空 p 保留（邮件中以空段落做间距的惯用法）
                    vec![Block::Paragraph {
                        runs,
                        align: own_align,
                        style,
                    }]
                } else {
                    split_image_blocks(runs, own_align)
                }
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
                        let style =
                            parse_block_style(attr_str(attrs.as_slice(), "style").unwrap_or(""));
                        if style.hidden {
                            continue;
                        }
                        let flushed = inline.flush_blocks(inherited);
                        if !flushed.is_empty() {
                            items.push(flushed);
                        }
                        let align = style.text_align.unwrap_or(inherited);
                        let li_base = merge_inline(
                            text_base,
                            &InlineStyle {
                                color: style.color,
                                ..InlineStyle::default()
                            },
                        );
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
        if parse_block_style(style_text).hidden {
            return Ok(()); // display:none 行内子树剪枝
        }
        let merged = merge_inline(base, &tag_inline_style(local, style_text));
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
fn tag_inline_style(local: &str, style_text: &str) -> InlineStyle {
    let mut style = parse_inline_style(style_text);
    match local {
        "b" | "strong" => style.bold = true,
        "i" | "em" => style.italic = true,
        "u" => style.underline = true,
        "s" | "strike" | "del" => style.strikethrough = true,
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

/// img 的 width/height 属性值：无单位数值 / px / `%`。
fn parse_dim_attr(value: &str) -> Option<Width> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse::<f32>().ok().map(Width::Percent);
    }
    let value = value.strip_suffix("px").unwrap_or(value).trim();
    value.parse::<f32>().ok().map(Width::Px)
}

/// 用聚合的行内 run 物化段落（聚合段落无自身样式，对齐继承父容器）。
fn paragraph(runs: Vec<InlineRun>, align: Align) -> Block {
    Block::Paragraph {
        style: BlockStyle::default(),
        runs,
        align,
    }
}

/// 行内 run 序列按图片切分为 `[段落, Image, 段落, …]`——行内 img 一律提升为
/// 块级图片（plan Task 5：简化布局）；非空 run 物化为段落，顺序保持。
fn split_image_blocks(runs: Vec<InlineRun>, align: Align) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut current: Vec<InlineRun> = Vec::new();
    for run in runs {
        match run {
            InlineRun::Image { src, alt, style } => {
                if !current.is_empty() {
                    blocks.push(paragraph(std::mem::take(&mut current), align));
                }
                blocks.push(Block::Image { src, alt, style });
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        blocks.push(paragraph(current, align));
    }
    blocks
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
///   与后续文本合并为单空格（归属空白处的样式），或在链接/图片前物化——
///   否则跨不同样式 run 的空格会丢失；
/// - `Ws::Preserve`（pre）下所有文本原样直入，不触碰状态。
struct InlineBuffer {
    ws: Ws,
    runs: Vec<InlineRun>,
    /// 段首 / Br 之后：后续文本的前导空白删除（保持到首个实质内容出现）。
    line_start: bool,
    /// 已发射文本以可折叠空白结尾（后续文本的前导空白与之合并）。
    trailing_ws: bool,
    /// 已见未发射的行中空白（遇到非文本 run 前挂起，物化为单空格；段尾消失）。
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
        // 行中文本：挂起空白先物化为单空格，或与上一 run 的尾随空白合并
        let pending_style = self.pending.take();
        if let Some(ws_style) = &pending_style {
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

    /// 发射非文本 run（Link / Image）：挂起的行中空白先物化为单空格。
    fn push_wrapped(&mut self, run: InlineRun) {
        if self.ws == Ws::Preserve {
            self.runs.push(run);
            return;
        }
        if let Some(ws_style) = self.pending.take() {
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

    /// 物化为块序列：折叠语义下按图片切分为 `[段落, Image, 段落, …]`（行内 img
    /// 提升为块级，简化布局）；pre 语义下整段原样物化。
    fn flush_blocks(&mut self, align: Align) -> Vec<Block> {
        let runs = self.finish();
        if runs.is_empty() {
            return Vec::new();
        }
        if self.ws == Ws::Preserve {
            return vec![paragraph(runs, align)];
        }
        split_image_blocks(runs, align)
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
        let Block::Image { src, alt, .. } = &doc.blocks[0] else {
            panic!("img 应提升为块级图片")
        };
        assert_eq!(*src, ImageRef::Invalid);
        assert_eq!(alt, "替代文字");
    }

    #[test]
    fn cid_and_data_image_preserved() {
        let doc =
            doc(r#"<img src="cid:Image001@ABC"><img src="data:image/png;base64,iVBORw0KGgo=">"#);
        assert_eq!(doc.blocks.len(), 2);
        let Block::Image {
            src: ImageRef::Cid { content_id },
            ..
        } = &doc.blocks[0]
        else {
            panic!("应为 Cid")
        };
        assert_eq!(content_id, "image001@abc"); // content_id 小写化
        let Block::Image {
            src: ImageRef::DataUri { mime, bytes },
            ..
        } = &doc.blocks[1]
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
        // table（T6 前降级 Container）的 align 同样生效
        let doc = build_document(
            r#"<table align="center"><tr><td>c</td></tr></table>"#,
            &Limits::default(),
        )
        .unwrap();
        let Block::Container { style, .. } = &doc.blocks[0] else {
            panic!("应为 Container")
        };
        assert_eq!(style.text_align, Some(Align::Center));
    }

    // ---- 图片（提升为块级 / 样式）----

    #[test]
    fn inline_image_promoted_to_block() {
        // 行内 img 提升为块级：段落被图片切分，顺序保持
        let doc = doc(r#"<p>before<img src="https://e/i.png" alt="a">after</p>"#);
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(text_of(&doc.blocks[0]), "before");
        let Block::Image { src, alt, .. } = &doc.blocks[1] else {
            panic!("img 应提升为块级")
        };
        assert_eq!(
            *src,
            ImageRef::Remote {
                url: "https://e/i.png".to_string()
            }
        );
        assert_eq!(alt, "a");
        assert_eq!(text_of(&doc.blocks[2]), "after");
    }

    #[test]
    fn bare_image_between_paragraphs_promoted() {
        let doc = doc(r#"<p>a</p><img src="http://e/i.png"><p>b</p>"#);
        assert_eq!(doc.blocks.len(), 3);
        assert!(matches!(&doc.blocks[1], Block::Image { .. }));
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
        // style 优先、width/height 属性兜底
        let doc =
            doc(r#"<img src="cid:x" width="600" height="50%" style="width:300px;max-width:100%">"#);
        let Block::Image { style, .. } = &doc.blocks[0] else {
            panic!("应为 Image")
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
        let doc = build_document(r#"<img src="cid:x" width="600">"#, &Limits::default()).unwrap();
        let Block::Image { style, .. } = &doc.blocks[0] else {
            panic!("应为 Image")
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
    fn table_downgrades_to_container_before_task6() {
        // T6 前 table 一族按未知标签降级为 Container（解析器隐式补的 tbody 同样降级）
        let doc = doc("<table><tr><td><p>cell</p></td></tr></table>");
        assert!(matches!(&doc.blocks[0], Block::Container { .. }));
        assert!(contains_text(&doc.blocks, "cell"));
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
