//! 标记序列化：净化后 IR → TextView html 管线可解析的标记 HTML。
//!
//! MailView 渲染路线（docs/plans/2026-11-16-mailtextview-feasibility.md 实施版）：
//! 每个顶层块序列化为一个**索引占位元素** `<div data-mb="{i}"></div>`——内容不进
//! HTML（图片 URL、富样式、表格网格都不暴露给 TextView 的内建解析），由 gpui-base
//! mailview-ext 补丁的 `html_block_parser` 在解析期认领为自定义块，渲染期经
//! `html_block_renderer` 回到 postium-ui 的块渲染器（复用 email_html 的全部保真
//! 代码：富样式 runs / 图片三源门控 / 探针）。
//!
//! 由此本模块的契约极简：
//! - 输出只含有序占位元素，顺序 = `blocks` 顺序（渲染层按索引回查 IR）；
//! - 空块列表输出空串（调用方走纯文本兜底，不构造 TextView）；
//! - 字节级确定性（快照测试锁定）。
//!
//! 占位元素选 `<div data-mb>`：div 在 TextView html 解析器的块级标签表内（会
//! 进入 parse_node 的元素分派），`data-mb` 是自定义属性不与任何内建语义冲突；
//! 上游认领测试（gpui-kit mailview-ext）已验证该形态可被 html_block_parser 拦截。

use crate::types::Block;

/// 每块的标记模板：`<div data-mb="{i}"></div>`（无内容、无闭合简写歧义）。
fn marker(i: usize) -> String {
    format!("<div data-mb=\"{i}\"></div>")
}

/// 把顶层块序列化为标记 HTML。块与块之间以换行分隔（html5ever 解析不依赖
/// 换行，分隔只为可读性与快照 diff 友好）。
pub fn serialize_marker_html(blocks: &[Block]) -> String {
    blocks
        .iter()
        .enumerate()
        .map(|(i, _)| marker(i))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_blocks_serialize_to_empty_string() {
        assert_eq!(serialize_marker_html(&[]), "");
    }

    #[test]
    fn markers_are_sequential_and_ordered() {
        let blocks = vec![
            Block::Hr,
            Block::Paragraph {
                style: Default::default(),
                runs: vec![],
                align: crate::types::Align::Left,
            },
            Block::Hr,
        ];
        assert_eq!(
            serialize_marker_html(&blocks),
            "<div data-mb=\"0\"></div>\n<div data-mb=\"1\"></div>\n<div data-mb=\"2\"></div>"
        );
    }

    #[test]
    fn block_content_never_leaks_into_markers() {
        // 富样式/图片/链接内容都不得进入序列化输出——内容只经 IR 通道回渲染层。
        let blocks = vec![Block::Paragraph {
            style: Default::default(),
            runs: vec![
                crate::types::InlineRun::Text {
                    text: "机密正文 <script>alert(1)</script>".into(),
                    style: Default::default(),
                },
                crate::types::InlineRun::Image {
                    src: crate::types::ImageRef::Remote {
                        url: "https://track.example.test/pixel.png".into(),
                    },
                    alt: "追踪像素".into(),
                    style: Default::default(),
                },
            ],
            align: crate::types::Align::Left,
        }];
        let out = serialize_marker_html(&blocks);
        assert_eq!(out, "<div data-mb=\"0\"></div>");
        assert!(!out.contains("script"));
        assert!(!out.contains("track.example"));
    }
}
