//! 正文右键菜单（P0）e2e：文本（复制/全选）、链接（打开/复制链接地址）、
//! 图片（复制图片/另存为）。
//!
//! 沿 email_list 先例（PopupMenu 弹出属渲染层，动作走直测）：菜单项动作
//! 以 helper 直调断言端到端效果（剪贴板/选区）；菜单构建路径经
//! `PopupMenu::build` 直构断言 `is_empty`；右键交互冒烟守卫「不破坏既有
//! 选区 + 链接命中记录」。

use std::sync::Arc;

use gpui_kit::component::menu::PopupMenu;
use gpui_kit::{div, App, AppContext as _, IntoElement, ParentElement, Render, Styled, px};

use postium_html::{Align, Block, BlockStyle, HtmlDocument, InlineRun, InlineStyle, SafeUrl};

use crate::views::email_html::{self, ImageCtx};
use crate::views::html_selection::HtmlSelection;

fn text(t: &str) -> InlineRun {
    InlineRun::Text {
        text: t.into(),
        style: InlineStyle::default(),
    }
}

/// 两段落文档（MailView 支持集内），第二组带链接。
fn link_doc() -> HtmlDocument {
    HtmlDocument {
        blocks: vec![
            Block::Paragraph {
                style: BlockStyle::default(),
                runs: vec![text("MenuProbeFirst")],
                align: Align::Left,
            },
            Block::Paragraph {
                style: BlockStyle::default(),
                runs: vec![
                    text("前缀"),
                    InlineRun::Link {
                        href: SafeUrl::Http("https://example.com/menu".into()),
                        runs: vec![text("MenuLinkText")],
                    },
                ],
                align: Align::Left,
            },
        ],
    }
}

/// 渲染宿主：驱动渲染内核注册文本组（右键命中/全选动作作用于注册表）。
struct RenderProbe {
    doc: HtmlDocument,
    selection: HtmlSelection,
}

impl Render for RenderProbe {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let el = email_html::render_html_document(
            &self.doc,
            false,
            ImageCtx::default(),
            self.selection.clone(),
            None,
            |_: &SafeUrl| {},
            cx,
        );
        div().w(px(400.)).h(px(400.)).child(el)
    }
}

#[gpui_pre::test]
fn mail_menu_copy_link_writes_clipboard(cx: &mut gpui_pre::TestAppContext) {
    // SafeUrl 显示串（菜单「复制链接地址」的剪贴板载荷）
    assert_eq!(
        email_html::safe_url_string(&SafeUrl::Http("https://example.com/menu".into())),
        "https://example.com/menu"
    );
    assert_eq!(
        email_html::safe_url_string(&SafeUrl::Mailto("a@b.test".into())),
        "a@b.test"
    );
    // 剪贴板写入（on_click 同款路径）
    cx.update(|app: &mut App| {
        app.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
            email_html::safe_url_string(&SafeUrl::Http("https://example.com/menu".into())),
        ));
    });
    let read = cx.update(|app: &mut App| app.read_from_clipboard().and_then(|i| i.text()));
    assert_eq!(read.as_deref(), Some("https://example.com/menu"));
}

#[gpui_pre::test]
fn mail_menu_select_all_selects_across_groups(cx: &mut gpui_pre::TestAppContext) {
    let selection = HtmlSelection::default();
    let sel = selection.clone();
    let (_root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        RenderProbe {
            doc: link_doc(),
            selection: sel,
        }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    // 组注册表应经两帧渲染填充（组文本探针可见）
    assert!(
        cx.debug_bounds("html-txt-MenuProbeFirst").is_some(),
        "文本组应已渲染并注册"
    );

    // 全选动作（菜单项 on_click 同款）覆盖全部组
    selection.select_all();
    let selected = selection.selected_text().expect("全选应有选区");
    assert!(
        selected.contains("MenuProbeFirst") && selected.contains("MenuLinkText"),
        "全选应覆盖两组文本，实际 {selected:?}"
    );
    // 复制动作载荷一致
    let text = selection.selected_text().unwrap();
    assert_eq!(text.lines().count(), 2, "跨组拼接应以换行分隔");
}

#[gpui_pre::test]
fn mail_menu_builders_content(cx: &mut gpui_pre::TestAppContext) {
    // 菜单构建路径绿：链接命中态、纯文本态（有/无选区）与图片菜单均产出
    // 非空菜单（经 PopupMenu::build 直构，与挂接路径同工厂）。
    let (_root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        struct Host;
        impl Render for Host {
            fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                div().w(px(100.)).h(px(100.))
            }
        }
        Host
    });
    let selection = HtmlSelection::default();

    // 链接命中态
    selection.on_right_down(Some(SafeUrl::Http("https://example.com/menu".into())));
    let builder = email_html::build_text_menu(selection.clone(), Arc::new(|_: &SafeUrl| {}));
    let menu = cx.update(|window: &mut gpui_kit::Window, app: &mut App| {
        PopupMenu::build(window, app, |menu, w, c| builder(menu, w, c))
    });
    cx.update(|_, cx| {
        assert!(!menu.read(cx).is_empty(), "链接菜单应有菜单项");
    });

    // 纯文本态（无选区 → 复制项禁用但存在，菜单非空）
    selection.on_right_down(None);
    let builder = email_html::build_text_menu(selection.clone(), Arc::new(|_: &SafeUrl| {}));
    let menu = cx.update(|window: &mut gpui_kit::Window, app: &mut App| {
        PopupMenu::build(window, app, |menu, w, c| builder(menu, w, c))
    });
    cx.update(|_, cx| {
        assert!(!menu.read(cx).is_empty(), "文本菜单应有菜单项");
    });

    // 图片菜单（Rendered 载荷）
    let rendered = Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])),
    )]));
    let payload = email_html::image_payload_for(&postium_html::ImageRef::Invalid, &rendered);
    let menu = cx.update(|window: &mut gpui_kit::Window, app: &mut App| {
        PopupMenu::build(window, app, |menu, _w, _c| {
            email_html::build_image_menu(menu, payload.clone(), "alt 文本".into())
        })
    });
    cx.update(|_, cx| {
        assert!(!menu.read(cx).is_empty(), "图片菜单应有菜单项");
    });
}

#[gpui_pre::test]
fn mail_menu_right_click_preserves_selection(cx: &mut gpui_pre::TestAppContext) {
    let selection = HtmlSelection::default();
    let sel = selection.clone();
    let (_root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        RenderProbe {
            doc: link_doc(),
            selection: sel,
        }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 建立选区（全选语义；拖选路径由既有 e2e 覆盖）
    selection.select_all();
    assert!(selection.is_selecting());

    // 右键打在链接文本 bounds 上：不 panic、不破坏选区、记录链接命中
    let bounds = cx
        .debug_bounds("html-txt-前缀MenuLinkText")
        .expect("链接文本组探针应存在");
    // 组 div 拉满宽（w_full）而文本仅占左段：取左端 40px（前缀 CJK 约
    // 28px 之后即 MenuLinkText 链接区间）。
    let link_point = gpui_pre::point(bounds.left() + px(40.), bounds.center().y);
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Right,
        position: link_point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        selection.is_selecting(),
        "右键不得破坏既有选区（浏览器语义）"
    );
    assert_eq!(
        selection.right_click_link(),
        Some(SafeUrl::Http("https://example.com/menu".into())),
        "右键在链接文本上应记录链接命中"
    );
}

/// PNG 重编码回路（复制图片/另存为的字节通道）+ 文件名净化。
#[gpui_pre::test]
fn mail_menu_image_bytes_and_file_name(cx: &mut gpui_pre::TestAppContext) {
    cx.update(crate::e2e_tests::setup);
    let (w, h) = (12u32, 8u32);
    // 管线存储恒为 BGRA（decode_render_image 期交换）：合成图按同口径预交换
    let mut buffer = image::RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]));
    for p in buffer.pixels_mut() {
        p.0.swap(0, 2);
    }
    let rendered = Arc::new(gpui_kit::RenderImage::new(vec![image::Frame::new(buffer)]));

    cx.update(|_app: &mut App| {
        let png = email_html::render_image_png_bytes(&rendered).expect("PNG 编码应成功");
        let decoded = image::load_from_memory(&png).expect("PNG 应可解回");
        assert_eq!((decoded.width(), decoded.height()), (w, h));
        // 首像素 RGB 保真（BGRA→RGBA 交换正确性）
        let rgba = decoded.to_rgba8();
        let p = rgba.get_pixel(0, 0);
        assert_eq!((p[0], p[1], p[2]), (10, 20, 30));
    });

    // 文件名净化：控制字符/路径分隔符替换、空 alt 兜底、扩展名按载荷
    let rendered_payload =
        email_html::image_payload_for(&postium_html::ImageRef::Invalid, &rendered);
    let name = email_html::image_save_file_name("lo go<x>:q", &rendered_payload);
    assert!(name.ends_with(".png"), "重编码载荷恒 .png：{name}");
    assert!(!name.contains('<') && !name.contains(':') && !name.contains('/'));
    assert_eq!(
        email_html::image_save_file_name("", &rendered_payload),
        "image.png"
    );
    // DataUri 直通载荷按 mime 定扩展名
    let raw = postium_html::ImageRef::DataUri {
        mime: "image/jpeg".into(),
        bytes: Arc::from(vec![1u8, 2, 3]),
    };
    let raw_payload = email_html::image_payload_for(&raw, &rendered);
    assert_eq!(email_html::image_save_file_name("photo", &raw_payload), "photo.jpg");
}
