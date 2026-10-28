//!
//! # 文档树构建——块级结构（spec §3，plan Task 4）
//!
//! [`build_document`] 是渲染管线的最后一环：把内联 + 净化后的 HTML 解析为
//! [`HtmlDocument`](crate::types::HtmlDocument) 中间表示。
//!
//! ## 处理步骤
//!
//! 1. html5ever + rcdom 宽松解析（解析错误逐个恢复，不整体失败）；
//! 2. 内容提取：css-inline 会把片段补全为完整文档，而 html5ever 对裸片段同样
//!    补全 html/head/body 壳——统一从 Document 根遍历即可：head 一族（style /
//!    title / meta / link / script）连同子树跳过，html / body 作为结构壳透明提升；
//! 3. 按块级白名单表递归构建：
//!
//!    | 标签 | 产物 |
//!    |---|---|
//!    | div / center / 未知标签 | [`Block::Container`]（`<center>` 附带居中对齐） |
//!    | p | [`Block::Paragraph`] |
//!    | h1–h6 | [`Block::Heading`] |
//!    | ul / ol + li | [`Block::List`] |
//!    | blockquote | [`Block::Quote`] |
//!    | hr | [`Block::Hr`] |
//!    | br | [`InlineRun::Br`]（并入所在段落） |
//!    | table / tr / td 一族、pre 等 | Task 6 前按未知标签降级为 Container |
//!    | script / style / head / title 等元数据 | 连同子树整体跳过 |
//!
//! 4. 混合内容规则：块级子元素与裸文本混排时，连续行内内容聚合为一个
//!    [`Block::Paragraph`]；
//! 5. `display:none` 子树剪枝（style 属性先经 [`parse_block_style`] 解析）；
//! 6. 节点计数超过 [`Limits::max_nodes`] → [`HtmlError::TooManyNodes`]；
//!    无有效块 → [`HtmlError::Empty`]。
//!
//! ## Task 5 / Task 6 扩展点
//!
//! - [`TreeBuilder::collect_runs_into`] 当前为占位实现：文本节点 →
//!   [`InlineRun::Text`]、行内元素解包取文本；完整的样式叠加与 a → Link /
//!   img → Image 终审白名单分类归 Task 5；
//! - table 一族暂按未知标签降级为 Container，Task 6 接管为 [`Block::Table`]。

use html5ever::tendril::TendrilSink;
use html5ever::{Attribute, ParseOpts, parse_document};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::error::HtmlError;
use crate::style::parse_block_style;
use crate::types::{Align, Block, BlockStyle, HtmlDocument, InlineRun, InlineStyle, Limits};

/// 树遍历深度上限（含块/行内递归）。
///
/// 病理嵌套防护（与浏览器 ~512 层的防护同数量级）：超出上限的子树剪枝，防止
/// 超深文档打爆渲染栈；上限内的正常嵌套邮件（几十层）不受影响。
const MAX_TREE_DEPTH: usize = 512;

/// 解析 HTML 为中间表示（管线最后一环）。
///
/// 节点计数超限 → [`HtmlError::TooManyNodes`]（携带实际计数）；净化/解析后无有效块
/// → [`HtmlError::Empty`]（调用方降级纯文本）。
pub fn build_document(html: &str, limits: &Limits) -> Result<HtmlDocument, HtmlError> {
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(html.to_owned());
    let mut builder = TreeBuilder { limits, visited: 0 };
    // 解析器总是补全 html/head/body 壳（css-inline 的完整文档与裸片段两种输入同构），
    // 从 Document 根遍历即可：head 一族被跳过，html/body 作为结构壳透明提升。
    let blocks = builder.child_blocks(&dom.document, 0, Align::Left)?;
    if blocks.is_empty() {
        return Err(HtmlError::Empty);
    }
    Ok(HtmlDocument { blocks })
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
    /// 常见行内元素：T4 占位解包取文本，T5 做样式叠加与 Link/Image 分类。
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
        "hr" => Kind::Block(BlockKind::Hr),
        "div" => Kind::Block(BlockKind::Container { center: false }),
        "center" => Kind::Block(BlockKind::Container { center: true }),
        // 常见行内元素（含 img——T5 提升为块级图片；T4 解包为空文本，自然消失）
        "span" | "a" | "b" | "strong" | "i" | "em" | "u" | "s" | "strike" | "del" | "ins"
        | "small" | "big" | "sub" | "sup" | "font" | "code" | "tt" | "q" | "cite" | "abbr"
        | "acronym" | "label" | "mark" | "img" => Kind::Inline,
        // 其余（table 一族 / pre / mjml-* 等模板标签）→ 未知标签宽容降级
        _ => Kind::Block(BlockKind::Container { center: false }),
    }
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
    /// `inherited` 为父容器解析后的水平对齐（text-align 继承语义）。
    fn child_blocks(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
    ) -> Result<Vec<Block>, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }
        let mut blocks = Vec::new();
        let mut inline = InlineBuffer::default();
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => inline.push_text(&contents.borrow()),
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Pass => {
                            // html/body 结构壳：子块直接提升，无自身样式
                            blocks.extend(self.child_blocks(child, depth + 1, inherited)?);
                        }
                        Kind::Inline => {
                            self.collect_runs_into(child, &mut inline.runs, depth + 1)?;
                        }
                        Kind::Br => inline.push_br(),
                        Kind::Block(kind) => {
                            if let Some(runs) = inline.flush() {
                                blocks.push(paragraph(runs, inherited));
                            }
                            let attrs = attrs.borrow();
                            if let Some(block) = self.build_element_block(
                                child,
                                kind,
                                attrs.as_slice(),
                                depth,
                                inherited,
                            )? {
                                blocks.push(block);
                            }
                        }
                    }
                }
                _ => {} // 注释 / DOCTYPE / PI：跳过
            }
        }
        if let Some(runs) = inline.flush() {
            blocks.push(paragraph(runs, inherited));
        }
        Ok(blocks)
    }

    /// 构建单个块级元素；`display:none` 子树返回 None 剪枝。
    fn build_element_block(
        &mut self,
        node: &Handle,
        kind: BlockKind,
        attrs: &[Attribute],
        depth: usize,
        inherited: Align,
    ) -> Result<Option<Block>, HtmlError> {
        let style = parse_block_style(attr_str(attrs, "style").unwrap_or(""));
        if style.hidden {
            return Ok(None); // display:none 子树整体剪枝
        }
        // 自身解析后的对齐（无声明则继承父级）；<center> 补 Center 语义
        let center = matches!(kind, BlockKind::Container { center: true });
        let own_align = style
            .text_align
            .or_else(|| center.then_some(Align::Center))
            .unwrap_or(inherited);
        let block = match kind {
            BlockKind::Container { center } => {
                let mut style = style;
                // <center> 语义物化进自身样式（GPUI 渲染层消费 text_align）
                if center && style.text_align.is_none() {
                    style.text_align = Some(Align::Center);
                }
                Block::Container {
                    style,
                    children: self.child_blocks(node, depth + 1, own_align)?,
                }
            }
            BlockKind::Paragraph => Block::Paragraph {
                runs: self.collect_runs_normalized(node, depth + 1)?,
                align: own_align,
                style,
            },
            BlockKind::Heading(level) => Block::Heading {
                level,
                runs: self.collect_runs_normalized(node, depth + 1)?,
            },
            BlockKind::List { ordered } => Block::List {
                ordered,
                items: self.build_list_items(node, depth + 1, own_align)?,
            },
            BlockKind::Quote => Block::Quote {
                children: self.child_blocks(node, depth + 1, own_align)?,
            },
            BlockKind::Hr => Block::Hr,
        };
        Ok(Some(block))
    }

    /// 列表项构建：li → 子块序列；裸行内内容聚合为独立列表项（宽容处理）。
    fn build_list_items(
        &mut self,
        node: &Handle,
        depth: usize,
        inherited: Align,
    ) -> Result<Vec<Vec<Block>>, HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(Vec::new());
        }
        let mut items: Vec<Vec<Block>> = Vec::new();
        let mut inline = InlineBuffer::default();
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => inline.push_text(&contents.borrow()),
                NodeData::Element { name, attrs, .. } => {
                    let local = name.local.as_ref();
                    if local == "li" {
                        let attrs = attrs.borrow();
                        let style =
                            parse_block_style(attr_str(attrs.as_slice(), "style").unwrap_or(""));
                        if style.hidden {
                            continue;
                        }
                        if let Some(runs) = inline.flush() {
                            items.push(vec![paragraph(runs, inherited)]);
                        }
                        let align = style.text_align.unwrap_or(inherited);
                        items.push(self.child_blocks(child, depth + 1, align)?);
                        continue;
                    }
                    match classify(local) {
                        Kind::Skip => {}
                        Kind::Inline => {
                            self.collect_runs_into(child, &mut inline.runs, depth + 1)?;
                        }
                        Kind::Br => inline.push_br(),
                        Kind::Pass => {
                            if let Some(runs) = inline.flush() {
                                items.push(vec![paragraph(runs, inherited)]);
                            }
                            items.push(self.child_blocks(child, depth + 1, inherited)?);
                        }
                        Kind::Block(kind) => {
                            if let Some(runs) = inline.flush() {
                                items.push(vec![paragraph(runs, inherited)]);
                            }
                            let attrs = attrs.borrow();
                            if let Some(block) = self.build_element_block(
                                child,
                                kind,
                                attrs.as_slice(),
                                depth,
                                inherited,
                            )? {
                                items.push(vec![block]); // 非 li 的块级子元素宽容为独立列表项
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(runs) = inline.flush() {
            items.push(vec![paragraph(runs, inherited)]);
        }
        Ok(items)
    }

    /// 收集块级元素（p/h1–h6）内容为行内 run 序列并做段落级规整。
    fn collect_runs_normalized(
        &mut self,
        node: &Handle,
        depth: usize,
    ) -> Result<Vec<InlineRun>, HtmlError> {
        let mut runs = Vec::new();
        self.collect_runs_into(node, &mut runs, depth)?;
        normalize_runs(&mut runs);
        Ok(runs)
    }

    /// T5 前占位：收集行内内容为文本 run——文本节点 → [`InlineRun::Text`]、
    /// 行内元素解包取文本、br → [`InlineRun::Br`]；`display:none` 行内子树剪枝，
    /// 元数据子树跳过。完整的样式叠加与 Link/Image 分类归 Task 5。
    fn collect_runs_into(
        &mut self,
        node: &Handle,
        runs: &mut Vec<InlineRun>,
        depth: usize,
    ) -> Result<(), HtmlError> {
        if depth > MAX_TREE_DEPTH {
            return Ok(());
        }
        for child in node.children.borrow().iter() {
            self.check_node()?;
            match &child.data {
                NodeData::Text { contents } => runs.push(InlineRun::Text {
                    text: contents.borrow().to_string(),
                    style: InlineStyle::default(),
                }),
                NodeData::Element { name, attrs, .. } => {
                    match classify(name.local.as_ref()) {
                        Kind::Skip => {}
                        Kind::Br => runs.push(InlineRun::Br),
                        Kind::Inline => {
                            let hidden = attr_str(attrs.borrow().as_slice(), "style")
                                .map(parse_block_style)
                                .is_some_and(|style| style.hidden);
                            if !hidden {
                                self.collect_runs_into(child, runs, depth + 1)?;
                            }
                        }
                        // 宽容：正常解析下 p/h 内不含块级子元素（解析器先行闭合），
                        // 手工构造的畸形树按混合内容规则解包取文本，不丢内容
                        Kind::Block(_) | Kind::Pass => {
                            self.collect_runs_into(child, runs, depth + 1)?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// 用聚合的行内 run 物化段落（聚合段落无自身样式，对齐继承父容器）。
fn paragraph(runs: Vec<InlineRun>, align: Align) -> Block {
    Block::Paragraph {
        style: BlockStyle::default(),
        runs,
        align,
    }
}

/// 行内内容聚合缓冲：块级边界之间的连续行内内容，块边界处整体物化为段落。
#[derive(Default)]
struct InlineBuffer {
    runs: Vec<InlineRun>,
}

impl InlineBuffer {
    fn push_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.runs.push(InlineRun::Text {
                text: text.to_string(),
                style: InlineStyle::default(),
            });
        }
    }

    fn push_br(&mut self) {
        self.runs.push(InlineRun::Br);
    }

    /// 物化为段落行内序列；纯空白序列（无文本无 Br）返回 None 不成段。
    fn flush(&mut self) -> Option<Vec<InlineRun>> {
        normalize_runs(&mut self.runs);
        if !runs_meaningful(&self.runs) {
            self.runs.clear();
            return None;
        }
        Some(std::mem::take(&mut self.runs))
    }
}

/// 段落级行内规整：合并相邻 Text run、修剪段首/段尾空白、丢弃空 Text run。
///
/// 浏览器块边界空白折叠的近似；run 内部空白不折叠（完整文本规整归 Task 5）。
fn normalize_runs(runs: &mut Vec<InlineRun>) {
    let mut merged: Vec<InlineRun> = Vec::with_capacity(runs.len());
    for run in runs.drain(..) {
        match (merged.last_mut(), run) {
            (Some(InlineRun::Text { text: prev, .. }), InlineRun::Text { text: next, .. }) => {
                prev.push_str(&next)
            }
            (_, run) => merged.push(run),
        }
    }
    if let Some(InlineRun::Text { text, .. }) = merged.first_mut() {
        *text = text.trim_start().to_owned();
    }
    if let Some(InlineRun::Text { text, .. }) = merged.last_mut() {
        *text = text.trim_end().to_owned();
    }
    merged.retain(|run| !matches!(run, InlineRun::Text { text, .. } if text.is_empty()));
    *runs = merged;
}

/// 是否存在实质内容（非空文本或 Br）；纯空白序列不成段。
fn runs_meaningful(runs: &[InlineRun]) -> bool {
    runs.iter().any(|run| match run {
        InlineRun::Br => true,
        InlineRun::Text { text, .. } => !text.trim().is_empty(),
        _ => false,
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
    use crate::types::Align;

    fn doc(html: &str) -> HtmlDocument {
        build_document(html, &Limits::default()).unwrap()
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
        let Block::Heading { level, runs } = &doc.blocks[1] else {
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

    // ---- 补充测试：body 提取 / 降级路径 / 边界 ----

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
        // 病理嵌套防护：超出深度上限的子树剪枝（不栈溢出、不产生节点）
        let html = format!("{}x{}", "<div>".repeat(600), "</div>".repeat(600));
        let doc = doc(&html);
        assert!(matches!(&doc.blocks[0], Block::Container { .. }));
        assert!(!contains_text(&doc.blocks, "x"));
    }
}
