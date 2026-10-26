//! md ⇄ HTML 转换器往返测试（无 GPUI 运行时依赖）。

use postium_editor::{email_html_to_md, md_to_email_html};

#[test]
fn roundtrip_heading_bold_list_link() {
    let md = "# 标题\n\n**加粗** 与 [链接](https://a.b) 以及：\n\n- 甲\n- 乙\n";
    let html = md_to_email_html(md);
    let back = email_html_to_md(&html);
    assert!(back.contains("# 标题"), "标题往返: {back}");
    assert!(back.contains("**加粗**"), "加粗往返: {back}");
    assert!(back.contains("[链接](https://a.b)"), "链接往返: {back}");
    assert!(back.contains("- 甲"), "列表往返: {back}");
}

#[test]
fn chinese_inline_content_survives() {
    let html = r#"<p>你好，<b>世界</b>，<i>斜体</i>与<code>代码</code></p><ul><li>中文列表项</li></ul>"#;
    let md = email_html_to_md(html);
    assert!(md.contains("**世界**"), "加粗: {md}");
    assert!(md.contains("*斜体*"), "斜体: {md}");
    assert!(md.contains("`代码`"), "行内代码: {md}");
    assert!(md.contains("中文列表项"), "列表: {md}");
}

#[test]
fn export_contains_email_inline_styles() {
    let html = md_to_email_html("段落\n\n> 引用\n\n    code");
    assert!(html.contains("<p style="), "段落内联样式: {html}");
    assert!(html.contains("blockquote style="), "引用内联样式: {html}");
}

#[test]
fn blockquote_and_nested_list_import() {
    let html = r#"<blockquote><p>引用一行</p></blockquote><ul><li>外层<ul><li>内层</li></ul></li></ul>"#;
    let md = email_html_to_md(html);
    assert!(md.contains("> 引用一行"), "引用: {md}");
    assert!(md.contains("- 外层"), "外层列表: {md}");
    assert!(md.contains("  - 内层"), "嵌套缩进: {md}");
}

#[test]
fn images_become_markdown() {
    let html = r#"<p><img src="https://x.test/a.png" alt="图">结束</p>"#;
    let md = email_html_to_md(html);
    assert!(md.contains("![图](https://x.test/a.png)"), "图片: {md}");
}

#[test]
fn script_and_style_are_dropped() {
    let html = r#"<html><head><style>p{color:red}</style></head><body><p>正文</p><script>alert(1)</script></body></html>"#;
    let md = email_html_to_md(html);
    assert!(md.contains("正文"), "正文保留: {md}");
    assert!(!md.contains("alert"), "脚本剔除: {md}");
    assert!(!md.contains("color"), "样式剔除: {md}");
}
