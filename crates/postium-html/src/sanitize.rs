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

use std::borrow::Cow;
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
/// - `s/strike/del` 放行（T14 语料审阅补）：删除线语义标签——builder 的
///   `tag_inline_style` 以"ammonia 白名单保留它们"为前提映射呈现语义
///   （builder.rs 文档注释明示该契约），白名单缺位使 `<s>` 在净化期被剥除、
///   语义永远到不了树构建；纯格式标签，无属性/URL 向量；
/// - `align` 放行：遗留呈现属性（T5：p/div/h1–h6/table 对齐兜底，style 优先），
///   无 URL/脚本向量；
/// - `width` 放行（td/th，T6；table，T8 管线实证补）：遗留呈现属性
///   `<td width="300">` / `<table width="600">` 是单元格/表格宽度的主流来源
///   （builder 的 Width 解析依赖它到达树构建）；纯数值/百分比取值，
///   无 URL/脚本向量；
/// - `bgcolor` 放行（table/td/th，T6 预批准补）：遗留背景色属性
///   `<table bgcolor="silver">`，取值经 builder 的 `parse_color` 通道终审
///   （基础 16 色表 + 十六进制/rgb 函数），白名单外取值解析为 None 自然丢弃；
///   无 URL/脚本向量；
/// - `colspan` 在 ammonia 默认属性表（td/th），照常放行；
/// - `href`（a）/ `src`（img）沿用 ammonia 默认属性表；
/// - 本地/内容 scheme 精细放行（[`local_content_scheme_filter`]，T5 评审裁定）：
///   `cid:`（内嵌附件引用）与 `data:image/*` 的 **img src** 穿透净化到达树构建
///   终审（spec §4.4 的 ImageRef 分类因此端到端可达——内嵌附件图片是邮件图片
///   显示的主流机制）；其余元素与其余 data: 取值一律维持剥除；
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
        "del",
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
        "s",
        "small",
        "span",
        "strike",
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
        // td/th 的 width 属性放行（T6）+ 三者的 bgcolor（T6 预批准补）：
        // 宽度/背景色的遗留呈现属性主流来源，builder 依赖它们到达树构建；
        // table 的 width 一并放行（T8 管线 fixture 实证补齐，与 bgcolor 同批）
        .add_tag_attributes("table", &["width", "bgcolor"])
        .add_tag_attributes("td", &["width", "bgcolor"])
        .add_tag_attributes("th", &["width", "bgcolor"])
        // cid:/data: 先过 ammonia 的 scheme 校验（其 url_schemes 是全局白名单、
        // 无 per-tag 配置）；attribute_filter 在其后执行、只能收紧——不合规形态
        // （a href="data:…"、img 的 data:text 等）在净化期即被剥除
        .add_url_schemes(&["cid", "data"])
        .attribute_filter(local_content_scheme_filter)
        .clean_content_tags(HashSet::from([
            "script", "style", "title", "noscript", "template",
        ]))
        .url_relative(UrlRelative::PassThrough)
        .clean(html)
        .to_string()
}

/// 本地/内容 URL scheme 的精细放行（attribute_filter 回调，ammonia 在 scheme
/// 校验之后调用，返回 None 即剥除该属性）：
///
/// - `cid:` 仅 **img src** 放行（内嵌附件引用，纯本地）；
/// - `data:` 仅 **img src** 且值为 `data:image/` 前缀放行（大小写不敏感、前导
///   空白 trim）；MIME 合法性与 base64 解码由树构建期终审
///   （`classify_image_src`：非 image/* 或解码失败 → `ImageRef::Invalid` 占位）；
/// - 其余一切 cid:/data: 形态（`a href="data:text/html"`、img 的
///   `data:text/...`、任意元素上的 cid: 等）一律剥除——攻击面与 ammonia 默认
///   行为一致；
/// - 其余属性原样透传。
fn local_content_scheme_filter<'u>(
    element: &str,
    attribute: &str,
    value: &'u str,
) -> Option<Cow<'u, str>> {
    let lowered = value.trim().to_ascii_lowercase();
    if !lowered.starts_with("cid:") && !lowered.starts_with("data:") {
        // 非本地/内容 scheme：原样透传（scheme 白名单校验由 ammonia 照常执行）
        return Some(Cow::Borrowed(value));
    }
    let allowed = element.eq_ignore_ascii_case("img")
        && attribute.eq_ignore_ascii_case("src")
        && (lowered.starts_with("cid:") || lowered.starts_with("data:image/"));
    allowed.then_some(Cow::Borrowed(value))
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

    #[test]
    fn sanitize_keeps_td_width_and_colspan() {
        // T6：td/th 的 width 放行（单元格宽度属性主流来源）；colspan 在默认表
        let out = sanitize_lenient(r#"<table><tr><td colspan="3" width="300">x</td></tr></table>"#);
        assert!(out.contains(r#"colspan="3""#) && out.contains(r#"width="300""#));
        // th 同通道；非单元格元素的 width 不放行（如 p 的 width 仍剔除）
        let out = sanitize_lenient(r#"<p width="300">x</p>"#);
        assert!(!out.contains("width"));
    }

    #[test]
    fn sanitize_keeps_table_width_and_bgcolor() {
        // T6 预批准补：table 的 width/bgcolor、td/th 的 bgcolor 放行（遗留呈现
        // 属性，builder 的宽度/parse_color 通道消费；无 URL/脚本向量）
        let out = sanitize_lenient(
            r##"<table width="600" bgcolor="#f9f9f9"><tr><td bgcolor="silver">x</td></tr></table>"##,
        );
        assert!(
            out.contains(r##"width="600""##)
                && out.contains(r##"bgcolor="#f9f9f9""##)
                && out.contains(r##"bgcolor="silver""##)
        );
        // th 同通道（th 须在表格上下文内，fragment 解析才保留）；白名单外
        // 元素的 bgcolor 仍剔除
        let out = sanitize_lenient(
            r#"<table><tr><th bgcolor="teal">h</th></tr></table><p bgcolor="red">x</p>"#,
        );
        assert!(out.contains(r#"bgcolor="teal""#));
        assert!(!out.contains(r#"bgcolor="red""#));
    }

    #[test]
    fn sanitize_passes_cid_and_data_image_src_on_img() {
        // T5 评审裁定：cid:/data:image 的 img src 放行（内嵌/数据图片端到端可达）
        let out = sanitize_lenient(
            r#"<img src="cid:image001@ABC" alt="c"><img src="data:image/png;base64,iVBOR" alt="d">"#,
        );
        assert!(out.contains(r#"src="cid:image001@ABC""#));
        assert!(out.contains(r#"src="data:image/png;base64,iVBOR""#));
        // 大小写不敏感 + 前导空白 trim（filter 裁定）
        let out = sanitize_lenient(r#"<img src=" DATA:IMAGE/PNG;BASE64,iVBOR ">"#);
        assert!(out.contains("DATA:IMAGE/PNG;BASE64,iVBOR"));
    }

    #[test]
    fn sanitize_strips_data_and_cid_outside_img_src() {
        // img 上的非图片 data:（data:text 攻击面不变）
        let out = sanitize_lenient(r#"<img src="data:text/html,<b>x</b>" alt="x">"#);
        assert!(!out.contains("data:"));
        // a href 的 cid:/data: 不放行（scheme 白名单仅对 img src 精细放行）
        let out =
            sanitize_lenient(r#"<a href="cid:image001">c</a><a href="data:text/html,evil">d</a>"#);
        assert!(!out.contains("cid:") && !out.contains("data:"));
        assert!(out.contains("c") && out.contains("d"));
    }
}
