//! MailView（TextView 宿主 + 自定义块接管）e2e（S1）。
//!
//! 与 HtmlRenderProbe（旧路径直驱）相对：本组直接驱动 mail_view 的
//! 构造/渲染入口，断言「标记序列化 → html_block_parser 认领 → 块渲染器
//! → 探针」全链路在 TextView 宿主内走通；app 级路由（EmailDetailView
//! html_body 分流）由 html_email_renders_via_new_renderer 覆盖（纯段落
//! 文档 → MailView 路径）。

use std::sync::Arc;

use gpui_kit::base::TextViewState;
use gpui_kit::{div, App, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, Styled, px};
use postium_html::{
    Align, Block, BlockStyle, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle, SafeUrl,
};

use crate::views::email_html::ImageCtx;
use crate::views::html_selection::HtmlSelection;
use crate::views::mail_view;

/// 宿主探针视图：真实 TextViewState（html_with_extensions 构造，与生产同径）。
struct MailViewProbe {
    state: Entity<TextViewState>,
    doc: HtmlDocument,
}

impl Render for MailViewProbe {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(400.))
            .h(px(600.))
            .overflow_hidden()
            .child(mail_view::render(
                &self.state,
                &self.doc,
                false,
                ImageCtx::default(),
                HtmlSelection::default(),
                None,
                Arc::new(|_: &SafeUrl| {}),
                // Context<Self> 解引用到 &mut App（gpui 标准解引用链）
                cx,
            ))
    }
}

#[gpui_pre::test]
fn mail_view_renders_blocks_inside_text_view_host(cx: &mut gpui_pre::TestAppContext) {
    fn text(t: &str) -> InlineRun {
        InlineRun::Text {
            text: t.into(),
            style: InlineStyle::default(),
        }
    }

    let doc = HtmlDocument {
        blocks: vec![
            Block::Heading {
                level: 1,
                align: Align::Left,
                runs: vec![text("TitleProbeText")],
            },
            // 容器 + 内嵌段落（递归支持集判定）
            Block::Container {
                style: BlockStyle::default(),
                children: vec![Block::Paragraph {
                    style: BlockStyle::default(),
                    runs: vec![text("ContainerProbe")],
                    align: Align::Left,
                }],
            },
            // 列表（items: Vec<Vec<Block>>）
            Block::List {
                ordered: false,
                items: vec![vec![Block::Paragraph {
                    style: BlockStyle::default(),
                    runs: vec![text("ListItemProbe")],
                    align: Align::Left,
                }]],
            },
            // 链接段落（SelectableText 命中链路在旧路径 e2e 覆盖，此处验证渲染存在）
            Block::Paragraph {
                style: BlockStyle::default(),
                runs: vec![
                    text("正文前缀"),
                    InlineRun::Link {
                        href: SafeUrl::Http("https://example.com/mailview".into()),
                        runs: vec![text("LinkProbeText")],
                    },
                ],
                align: Align::Left,
            },
            Block::Hr,
        ],
    };

    // 支持集判定：无 Table/Quote → MailView 路径
    assert!(mail_view::supported(&doc), "无表格/引用的文档应走 MailView");

    let state = cx.update(|app: &mut App| mail_view::build_text_view_state(&doc, app));
    let (root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        MailViewProbe {
            state: state.clone(),
            doc,
        }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let _ = root; // 探针视图仅承载渲染

    // 根锚点 + 各块探针：证明块渲染器在 TextView 宿主内产出内容
    for key in [
        "html-root",
        "html-hr",
        "html-txt-TitleProbeText",
        "html-txt-ContainerProbe",
        "html-txt-ListItemProbe",
        "html-txt-正文前缀LinkProbeTex",
    ] {
        let bounds = cx.debug_bounds(key).unwrap_or_else(|| panic!("{key} 应存在"));
        assert!(
            bounds.size.width > px(0.) || bounds.size.height > px(0.),
            "{key} 应有非零尺寸（实际 {bounds:?}）"
        );
    }

    // 图片块支持集（无 src 的 Invalid 占位形态）——仅判定，不渲染断言（占位框
    // 探针 html-img 由旧路径 e2e 覆盖渲染细节）
    let img_doc = HtmlDocument {
        blocks: vec![Block::Image {
            src: ImageRef::Invalid,
            alt: "占位".into(),
            style: ImageStyle::default(),
        }],
    };
    assert!(mail_view::supported(&img_doc), "块级图片应在支持集内");

    // 回落判定：Table/Quote → 旧路径
    let quote_doc = HtmlDocument {
        blocks: vec![Block::Quote {
            children: vec![Block::Paragraph {
                style: BlockStyle::default(),
                runs: vec![text("引用")],
                align: Align::Left,
            }],
        }],
    };
    assert!(!mail_view::supported(&quote_doc), "引用块应回落旧路径（S2 迁入）");
}
