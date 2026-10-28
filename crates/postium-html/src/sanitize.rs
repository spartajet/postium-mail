//!
//! # 净化与预处理
//!
//! spec §4 的前两步（字符串级预处理，发生在 css-inline 之前）：
//!
//! 1. [`strip_mso_comments`]：删除 MSO 条件注释（Outlook VML，非标准 HTML）：
//!    - downlevel-hidden：`<!--[if …]>…<![endif]-->`（完整 HTML 注释，整体删除）；
//!    - downlevel-revealed：`<![if …]>…<![endif]>`（非注释形式，含条件内容整体删除）。
//! 2. [`sanitize_lenient`]：ammonia 宽松净化——结构白名单 + 保留 `class/id/style`
//!    （css-inline 选择器匹配依赖它们），剔除 script/iframe/form 等，事件属性不进白名单。

use std::collections::HashSet;

use ammonia::UrlRelative;

/// downlevel-revealed 形式的起始标记：`<![if …]>`。
const REVEALED_OPEN: &str = "<![if";
/// downlevel-revealed 形式的结束标记：`<![endif]>`。
const REVEALED_END: &str = "<![endif]>";
/// downlevel-hidden 形式的完整结束标记（含条件内容末尾的 endif 注释收口）。
const HIDDEN_END: &str = "<![endif]-->";

/// 删除 MSO 条件注释（spec §4.1，字符串级整体删除）。
///
/// 处理两种形态：
/// - `<!--[if …]>…<![endif]-->`：downlevel-hidden，整个注释区间删除；
///   优先匹配 `<![endif]-->` 收口，退化时按普通注释首个 `-->` 收口。
/// - `<![if …]>…<![endif]>`：downlevel-revealed，从 `<![if` 起到 `<![endif]>`
///   止连同内部 VML 内容整体删除；无收口时仅删起始标签本身（保留后续内容）。
///
/// 普通注释（`<!--` 后非 `[if`/`[endif]`）原样保留，交由 [`sanitize_lenient`] 统一删除。
pub fn strip_mso_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        // 两种起始标记中更靠前的先处理；下标均来自 ASCII 标记的 find 结果，必为字符边界。
        let hidden = rest.find("<!--");
        let revealed = rest.find(REVEALED_OPEN);
        let (start, is_hidden) = match (hidden, revealed) {
            (None, None) => break,
            (Some(h), None) => (h, true),
            (None, Some(r)) => (r, false),
            (Some(h), Some(r)) => (h.min(r), h < r),
        };
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if is_hidden {
            if is_conditional_open(after) {
                // 条件注释整体删除：优先 `<![endif]-->` 收口，其次首个 `-->`，未闭合删到串尾。
                let end = after
                    .find(HIDDEN_END)
                    .map(|p| p + HIDDEN_END.len())
                    .or_else(|| after.find("-->").map(|p| p + "-->".len()))
                    .unwrap_or(after.len());
                rest = &after[end..];
            } else {
                // 普通注释原样保留到收口（未闭合则保留到串尾）。
                let end = after
                    .find("-->")
                    .map(|p| p + "-->".len())
                    .unwrap_or(after.len());
                out.push_str(&after[..end]);
                rest = &after[end..];
            }
        } else {
            // downlevel-revealed：含内部内容整体删除；无 `<![endif]>` 收口时只删起始标签。
            let end = after
                .find(REVEALED_END)
                .map(|p| p + REVEALED_END.len())
                .unwrap_or_else(|| {
                    after
                        .find('>')
                        .map(|p| p + ">".len())
                        .unwrap_or(after.len())
                });
            rest = &after[end..];
        }
    }
    out.push_str(rest);
    out
}

/// 判断 `<!--` 开头的注释是否为 MSO 条件注释（`<!--` 后允许空白，随后是 `[if` / `[endif`）。
fn is_conditional_open(s: &str) -> bool {
    s.strip_prefix("<!--").is_some_and(|body| {
        let t = body.trim_start();
        t.starts_with("[if") || t.starts_with("[endif")
    })
}

/// 宽松净化（spec §4.2）：ammonia 白名单 + 保留 `class/id/style`。
///
/// - 标签白名单 = 邮件常见结构集（spec §4.2 逐项列出）；
/// - `class/id/style` 对所有白名单标签放行（css-inline 选择器匹配依赖）；
/// - `align` 放行：遗留呈现属性（T5：p/div/h1–h6/table 对齐兜底，style 优先），
///   无 URL/脚本向量；
/// - `href`（a）/ `src`（img）沿用 ammonia 默认属性表；
/// - URL 相对路径原样透传（后续渲染按原始文档上下文处理，不做改写）；
/// - clean_content_tags 在 ammonia 默认（script/style）之上追加 title/noscript/template：
///   ammonia 以 fragment 解析、剥 html/head/body 壳但把 head 内容重挂到顶层，
///   `<title>` 若不在白名单会被解包保留文本（评审 I-1：邮件标题泄漏为正文首段），
///   内容型剔除连文本一并移除；noscript/template 同理整体删除。
pub fn sanitize_lenient(html: &str) -> String {
    let tags = HashSet::from([
        "a",
        "b",
        "blockquote",
        "br",
        "center",
        "div",
        "em",
        "font",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "i",
        "img",
        "li",
        "ol",
        "p",
        "pre",
        "small",
        "span",
        "strong",
        "sub",
        "sup",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "tr",
        "u",
        "ul",
    ]);
    ammonia::Builder::default()
        .tags(tags)
        .generic_attributes(HashSet::from(["class", "id", "style", "align"]))
        .clean_content_tags(HashSet::from([
            "script", "style", "title", "noscript", "template",
        ]))
        .url_relative(UrlRelative::PassThrough)
        .clean(html)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_mso_conditional_comments() {
        let html = r#"<div>A<!--[if mso]><v:rect></v:rect><![endif]-->B<![if !mso]><span>C</span><![endif]></div>"#;
        let out = strip_mso_comments(html);
        assert!(!out.contains("mso") && !out.contains("v:rect"));
        assert!(out.contains("A") && out.contains("B"));
    }

    #[test]
    fn sanitize_keeps_style_attrs_drops_scripts() {
        let html = r#"<div class="a" style="color:red" onclick="x()"><script>alert(1)</script><p data-x="1">t</p></div>"#;
        let out = sanitize_lenient(html);
        assert!(out.contains(r#"class="a""#) && out.contains("color:red"));
        assert!(!out.contains("script") && !out.contains("onclick"));
    }

    #[test]
    fn sanitize_keeps_align_attribute() {
        // T5：align 属性放行（builder 的对齐属性兜底依赖它到达树构建）
        let out = sanitize_lenient(r#"<p align="center" valign="middle">x</p>"#);
        assert!(out.contains(r#"align="center""#));
        assert!(!out.contains("valign"), "白名单外属性仍应剔除");
    }
}
