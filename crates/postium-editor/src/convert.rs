//! md ⇄ 邮件 HTML 转换器（产品化）。
//!
//! - `md_to_email_html`：pulldown-cmark 渲染 + 邮件友好内联样式后处理；
//!   任务列表勾选框渲染为 ☐/☑ 文本（邮件客户端不执行 `<input>`）。
//! - `email_html_to_md`：scraper（html5ever）解析存量邮件 HTML，
//!   支持常见标签降级为 markdown 子集；不认识的标签按文本处理。

use pulldown_cmark::{html, Options, Parser};
use scraper::{Html, node::Node as ScraperNode};

/// ego-tree 树节点引用（scraper 内部树）
type NodeRef<'a> = ego_tree::NodeRef<'a, ScraperNode>;

// ---------------------------------------------------------------------------
// md → HTML（发送导出）
// ---------------------------------------------------------------------------

/// 导出为可发送的邮件 HTML（内联样式，收件人客户端友好）。
pub fn md_to_email_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TASKLISTS);

    // 任务列表标记在事件流层替换为文本符号，避免依赖 pulldown 的 HTML 输出格式
    let mut events = Vec::new();
    for event in Parser::new_ext(md, opts) {
        if let pulldown_cmark::Event::TaskListMarker(checked) = event {
            events.push(pulldown_cmark::Event::Text(if checked {
                "☑ ".into()
            } else {
                "☐ ".into()
            }));
        } else {
            events.push(event);
        }
    }

    let mut body = String::new();
    html::push_html(&mut body, events.into_iter());
    inline_email_styles(&body)
}

/// 给标准 HTML 标签追加邮件内联样式（邮件客户端忽略 <style> 标签）。
fn inline_email_styles(body: &str) -> String {
    body
        // pulldown 输出的标签均无属性，可直接字符串替换
        .replace("<p>", r#"<p style="margin:0 0 8px;">"#)
        .replace(
            "<blockquote>",
            r#"<blockquote style="border-left:3px solid #cbd5e1;margin:8px 0;padding:4px 12px;color:#555;">"#,
        )
        .replace(
            "<pre>",
            r#"<pre style="background:#f6f8fa;padding:12px;border-radius:6px;overflow-x:auto;">"#,
        )
        .replace("<ul>", r#"<ul style="margin:0 0 8px;padding-left:24px;">"#)
        .replace("<ol>", r#"<ol style="margin:0 0 8px;padding-left:24px;">"#)
}

// ---------------------------------------------------------------------------
// HTML → md（导入：回复/草稿编辑）
// ---------------------------------------------------------------------------

/// 常见邮件 HTML → markdown。
///
/// 支持：h1-h6、p、div（叶子）、ul/ol/li（嵌套）、blockquote、pre、br、
/// b/strong/i/em/s/del/u/code/a/img；table 降级为单元格文本行。
pub fn email_html_to_md(html: &str) -> String {
    let document = Html::parse_document(html);
    let tree = document.tree;
    let root = tree.root();
    let mut out = String::new();

    // 从 body（或 html/根）开始遍历块级结构
    let start = root
        .children()
        .find(|n| matches!(n.value(), ScraperNode::Element(el) if el.name() == "html"))
        .and_then(|html_node| {
            html_node
                .children()
                .find(|n| matches!(n.value(), ScraperNode::Element(el) if el.name() == "body"))
        })
        .unwrap_or(root);

    let mut ctx = Ctx { ordered_depth: Vec::new() };
    for child in start.children() {
        emit_block_node(&child, &mut out, &mut ctx, 0);
    }

    // 压缩 3+ 连续空行为 2（保持段落间隔）
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

struct Ctx {
    /// 列表嵌栈：true = ordered（用于 li 前缀与缩进）
    ordered_depth: Vec<bool>,
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li"
            | "blockquote" | "pre" | "table" | "tr" | "section" | "article" | "header"
            | "footer" | "main" | "hr" | "figure"
    )
}

/// 处理一个块级节点（或游离文本）。
fn emit_block_node(node: &NodeRef<'_>, out: &mut String, ctx: &mut Ctx, depth: usize) {
    match node.value() {
        ScraperNode::Text(t) => {
            let text: &str = &t.text;
            let text = text.trim();
            if !text.is_empty() && ctx.ordered_depth.is_empty() {
                out.push_str(text);
                out.push_str("\n\n");
            }
        }
        ScraperNode::Element(el) => {
            let name = el.name();
            match name {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    let level = name[1..].parse::<usize>().unwrap_or(2).min(6);
                    let text = inline_children_md(node);
                    if !text.is_empty() {
                        out.push_str(&"#".repeat(level));
                        out.push(' ');
                        out.push_str(&text);
                        out.push_str("\n\n");
                    }
                }
                "p" | "div" | "section" | "article" | "header" | "footer" | "main" | "figure" => {
                    // 容器内还有块级子元素则递归；否则按段落取行内内容
                    let has_block = node.children().any(|c| {
                        matches!(c.value(), ScraperNode::Element(e) if is_block_tag(e.name()))
                    });
                    if has_block {
                        for child in node.children() {
                            emit_block_node(&child, out, ctx, depth);
                        }
                    } else {
                        let text = inline_children_md(node);
                        if !text.is_empty() {
                            out.push_str(&text);
                            out.push_str("\n\n");
                        }
                    }
                }
                "blockquote" => {
                    let inner = collect_blockquote_md(node, ctx);
                    for line in inner.lines().filter(|l| !l.trim().is_empty()) {
                        out.push_str(&format!("> {line}\n"));
                    }
                    if !inner.is_empty() {
                        out.push('\n');
                    }
                }
                "pre" => {
                    out.push_str("```\n");
                    out.push_str(&descendant_text(node));
                    out.push_str("\n```\n\n");
                }
                "ul" | "ol" => {
                    ctx.ordered_depth.push(name == "ol");
                    for child in node.children() {
                        emit_block_node(&child, out, ctx, depth + 1);
                    }
                    ctx.ordered_depth.pop();
                    if ctx.ordered_depth.is_empty() {
                        out.push('\n');
                    }
                }
                "li" => {
                    // 行内部分（跳过嵌套列表子节点，另行递归）
                    let mut inline_md = String::new();
                    for child in node.children() {
                        if let ScraperNode::Element(e) = child.value()
                            && matches!(e.name(), "ul" | "ol")
                        {
                            continue;
                        }
                        inline_node(&child, &mut inline_md);
                    }
                    let text = inline_md.trim().to_string();
                    let ordered = *ctx.ordered_depth.last().unwrap_or(&false);
                    let pad = "  ".repeat(ctx.ordered_depth.len().saturating_sub(1));
                    if !text.is_empty() {
                        // 任务列表（☐/☑ 前缀已在行内）保持 "- " 前缀
                        if text.starts_with("☐") || text.starts_with("☑") {
                            out.push_str(&format!("{pad}- {text}\n"));
                        } else if ordered {
                            out.push_str(&format!("{pad}1. {text}\n"));
                        } else {
                            out.push_str(&format!("{pad}- {text}\n"));
                        }
                    }
                    // 嵌套列表递归（当前 li 行之后，缩进层级 +1）
                    for child in node.children() {
                        if let ScraperNode::Element(e) = child.value()
                            && matches!(e.name(), "ul" | "ol")
                        {
                            ctx.ordered_depth.push(e.name() == "ol");
                            for sub in child.children() {
                                emit_block_node(&sub, out, ctx, depth + 1);
                            }
                            ctx.ordered_depth.pop();
                        }
                    }
                }
                "table" | "tr" | "tbody" | "thead" => {
                    for child in node.children() {
                        emit_block_node(&child, out, ctx, depth);
                    }
                }
                "td" | "th" => {
                    let text = inline_children_md(node);
                    if !text.is_empty() {
                        out.push_str(&text);
                        out.push_str("  \n");
                    }
                }
                "br" => out.push('\n'),
                "hr" => out.push_str("\n---\n\n"),
                "script" | "style" | "head" | "title" | "meta" | "link" => {}
                _ => {
                    // 未知元素按行内内容并入段落
                    let text = inline_children_md(node);
                    if !text.is_empty() {
                        out.push_str(&text);
                        out.push_str("\n\n");
                    }
                }
            }
        }
        _ => {}
    }
}

/// blockquote 内部按块级递归后整体缩进。
fn collect_blockquote_md(node: &NodeRef<'_>, ctx: &mut Ctx) -> String {
    let mut inner = String::new();
    for child in node.children() {
        emit_block_node(&child, &mut inner, ctx, 0);
    }
    inner.trim().to_string()
}

/// 收集一个节点的行内子树为 markdown（b/i/s/code/a/img/br/文本）。
fn inline_children_md(node: &NodeRef<'_>) -> String {
    let mut out = String::new();
    for child in node.children() {
        inline_node(&child, &mut out);
    }
    // 行内空白折叠
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn inline_node(node: &NodeRef<'_>, out: &mut String) {
    match node.value() {
        ScraperNode::Text(t) => out.push_str(&collapse_ws(&t.text)),
        ScraperNode::Element(el) => {
            let name = el.name();
            match name {
                "b" | "strong" => wrap_inline(node, out, "**"),
                "i" | "em" => wrap_inline(node, out, "*"),
                "s" | "del" | "strike" => wrap_inline(node, out, "~~"),
                "code" => wrap_inline(node, out, "`"),
                "u" | "span" | "font" | "mark" | "small" | "sub" | "sup" => {
                    for child in node.children() {
                        inline_node(&child, out);
                    }
                }
                "a" => {
                    let text = inline_children_md(node);
                    let href = el.attr("href").unwrap_or_default().to_string();
                    if text.is_empty() {
                        return;
                    }
                    if href.is_empty() || href == "#" {
                        out.push_str(&text);
                    } else {
                        out.push_str(&format!("[{text}]({href})"));
                    }
                }
                "img" => {
                    let alt = el.attr("alt").unwrap_or("");
                    let src = el.attr("src").unwrap_or("");
                    if !src.is_empty() {
                        out.push_str(&format!("![{alt}]({src})"));
                    }
                }
                "br" => out.push('\n'),
                _ => {
                    // 块级元素出现在行内上下文：递归取文本
                    for child in node.children() {
                        inline_node(&child, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn wrap_inline(node: &NodeRef<'_>, out: &mut String, mark: &str) {
    let text = inline_children_md(node);
    if !text.is_empty() {
        out.push_str(mark);
        out.push_str(&text);
        out.push_str(mark);
    }
}

fn collapse_ws(s: &str) -> String {
    // &nbsp; 已由 html5ever 解码为 \u{a0}，统一为空格
    s.replace('\u{a0}', " ")
}

/// 收集节点后代全部文本（pre 块用）。
fn descendant_text(node: &NodeRef<'_>) -> String {
    let mut out = String::new();
    for n in node.traverse() {
        if let ego_tree::iter::Edge::Open(n) = n
            && let ScraperNode::Text(t) = n.value()
        {
            out.push_str(&t.text);
        }
    }
    out.trim().to_string()
}
