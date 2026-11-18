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

/// 双击词选 / 三击段选（浏览器语义）+ 多击不触发链接打开。
#[gpui_pre::test]
fn mail_body_double_click_selects_word_not_link(cx: &mut gpui_pre::TestAppContext) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let link_opens = Arc::new(AtomicUsize::new(0));
    struct Probe {
        doc: HtmlDocument,
        selection: HtmlSelection,
        link_opens: Arc<AtomicUsize>,
    }
    impl Render for Probe {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            cx: &mut gpui_kit::Context<Self>,
        ) -> impl IntoElement {
            let opens = Arc::clone(&self.link_opens);
            let el = email_html::render_html_document(
                &self.doc,
                false,
                ImageCtx::default(),
                self.selection.clone(),
                None,
                move |_: &SafeUrl| {
                    opens.fetch_add(1, Ordering::Relaxed);
                },
                cx,
            );
            div().w(px(400.)).h(px(400.)).child(el)
        }
    }
    let selection = HtmlSelection::default();
    let sel = selection.clone();
    let (_root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        Probe {
            doc: link_doc(),
            selection: sel,
            link_opens: Arc::clone(&link_opens),
        }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 双击链接词「MenuLinkText」（组文本 "前缀MenuLinkText"，链接段从
    // 第 7 字节起）：词选中，链接不打开
    let bounds = cx
        .debug_bounds("html-txt-前缀MenuLinkText")
        .expect("链接组探针应存在");
    let word_point = gpui_pre::point(bounds.left() + px(40.), bounds.center().y);
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Left,
        position: word_point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui_pre::MouseUpEvent {
        button: gpui_kit::MouseButton::Left,
        position: word_point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 2,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        selection.selected_text().as_deref(),
        Some("MenuLinkText"),
        "双击应选中链接词（四类词模型：ASCII 词独立于 CJK 前缀）"
    );
    assert_eq!(link_opens.load(Ordering::Relaxed), 0, "双击不得触发链接打开");

    // 三击同组：整组段选
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Left,
        position: word_point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 3,
        first_mouse: false,
    });
    cx.simulate_event(gpui_pre::MouseUpEvent {
        button: gpui_kit::MouseButton::Left,
        position: word_point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 3,
    });
    cx.run_until_parked();
    assert_eq!(
        selection.selected_text().as_deref(),
        Some("前缀MenuLinkText"),
        "三击应选中整组"
    );
    assert_eq!(link_opens.load(Ordering::Relaxed), 0, "三击不得触发链接");
}

/// 头部字段（主题）可选中：双击词选 + 三击整行（P0 续）。
#[gpui_pre::test]
fn mail_header_subject_selectable(cx: &mut gpui_pre::TestAppContext) {
    use gpui_kit::component::ActiveTheme as _;
    struct HeaderProbe {
        selection: HtmlSelection,
    }
    impl Render for HeaderProbe {
        fn render(
            &mut self,
            _: &mut gpui_kit::Window,
            cx: &mut gpui_kit::Context<Self>,
        ) -> impl IntoElement {
            self.selection.begin_frame();
            let subject = cx.theme().foreground;
            let family = cx.theme().font_family.clone();
            div()
                .w(px(400.))
                .h(px(200.))
                .p_3()
                .child(email_html::selectable_header_text(
                    "mail-subject",
                    "季度Report总结v2",
                    family,
                    14.5,
                    gpui_kit::FontWeight::SEMIBOLD,
                    subject,
                    cx.theme().selection,
                    &self.selection,
                ))
        }
    }
    let selection = HtmlSelection::default();
    let sel = selection.clone();
    let (_root, cx) = cx.add_window_view(|_, cx| {
        crate::e2e_tests::setup(cx);
        HeaderProbe { selection: sel }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let bounds = cx
        .debug_bounds("html-txt-季度Report总结v2")
        .expect("主题探针应存在（内容前缀键）");

    // 双击 ASCII 段（Report 起于 2 个 CJK 后 ≈29px）：只选 ASCII 词
    let point = gpui_pre::point(bounds.left() + px(40.), bounds.center().y);
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Left,
        position: point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 2,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let selected = selection.selected_text();
    assert!(
        selected.as_deref().is_some_and(|s| s.contains("Report")),
        "双击主题应选中 Report 词，实际 {selected:?}"
    );

    // 三击整行（混合文本全组）
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Left,
        position: point,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 3,
        first_mouse: false,
    });
    cx.run_until_parked();
    assert_eq!(
        selection.selected_text().as_deref(),
        Some("季度Report总结v2"),
        "三击应选中整行主题"
    );
}

/// 真实应用路径（AppShell → 详情视图）：头部主题拖选、游离抬起守卫、
/// 选区跨右键存活（复制菜单不误灰的根因回归）。
#[gpui_pre::test]
fn mail_header_drag_select_in_real_view(cx: &mut gpui_pre::TestAppContext) {
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    crate::e2e_tests::force_e2e_data_dir();
    let (api, sync_rx) = crate::e2e_tests::boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        crate::e2e_tests::setup(cx);
        crate::app::AppShell::new(api.clone(), sync_rx, window, cx)
    });
    crate::e2e_tests::wait_for(cx, |cx| {
        crate::e2e_tests::with_app(cx, |app| {
            (!shell.read(app).accounts.read(app).groups.is_empty()).then_some(())
        })
    });

    // 带主题的草稿（行选中同款接缝）
    let draft_id = {
        let account_id = crate::e2e_tests::with_app(cx, |app| {
            shell.read(app).accounts.read(app).groups[0].account.id
        });
        let req = postium_core::service::mail_draft::SaveDraftRequest {
            draft_id: None,
            account_id,
            to: vec!["e2e.user@postium.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "HeaderDragProbe Subject Words".into(),
            body_html: "<p>HeaderDragBody</p>".into(),
            body_text: "正文".into(),
            attachments: vec![],
        };
        crate::runtime::runtime()
            .block_on(async move { api.save_draft(req).await })
            .expect("草稿应保存成功")
            .draft_id
    };
    crate::e2e_tests::with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(draft_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(draft_id));
                cx.notify();
            });
        });
    });
    crate::e2e_tests::wait_for(cx, |cx| {
        crate::e2e_tests::with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == draft_id).then_some(())
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 主题探针（内容前 16 个字母数字 = HeaderDragProbeSu）
    let bounds = cx
        .debug_bounds("html-txt-HeaderDragProbeS")
        .expect("主题探针应存在（头部字段经 selectable_header_text 注册）");
    let start = gpui_pre::point(bounds.left() + px(6.), bounds.center().y);
    let end = gpui_pre::point(bounds.left() + px(90.), bounds.center().y);

    // 拖选：按下 → 移动 → 抬起
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Left,
        position: start,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(gpui_pre::MouseMoveEvent {
        position: end,
        modifiers: gpui_pre::Modifiers::default(),
        pressed_button: Some(gpui_kit::MouseButton::Left),
    });
    cx.simulate_event(gpui_pre::MouseUpEvent {
        button: gpui_kit::MouseButton::Left,
        position: end,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let header_sel = crate::e2e_tests::with_app(cx, |app| {
        let view = shell.read(app).detail_view.clone().expect("详情视图应存在");
        view.read(app).header_selection.clone()
    });
    let selected = header_sel.selected_text();
    assert!(
        selected.as_deref().is_some_and(|s| s.contains("Header")),
        "拖选主题应产生选区，实际 {selected:?}"
    );

    // 游离抬起守卫：无按下的左键抬起（浮层点击泄漏形态）不得清选区
    cx.simulate_event(gpui_pre::MouseUpEvent {
        button: gpui_kit::MouseButton::Left,
        position: end,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 1,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        header_sel.is_selecting(),
        "游离抬起（无配对按下）不得清选区"
    );

    // 右键命中记录：主题文本（无链接 → 文本菜单，复制项可用）
    cx.simulate_event(gpui_pre::MouseDownEvent {
        button: gpui_kit::MouseButton::Right,
        position: end,
        modifiers: gpui_pre::Modifiers::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        header_sel.is_selecting() && header_sel.right_click_link().is_none(),
        "右键后选区应存活且命中为纯文本（复制菜单可用）"
    );
}

/// 关闭窗口行为设置（线上反馈⑤）：设置页三段选择点击即持久化，
/// ui-state.json 串行域内往返断言。
#[gpui_pre::test]
fn settings_close_behavior_persists(cx: &mut gpui_pre::TestAppContext) {
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    crate::e2e_tests::force_e2e_data_dir();
    let _guard = crate::e2e_tests::ui_state_lock();
    // 前置归零（防止进程内其他测试的残留）
    crate::ui_state::set_close_behavior(crate::ui_state::CloseBehavior::Ask);
    assert_eq!(
        crate::ui_state::close_behavior(),
        crate::ui_state::CloseBehavior::Ask
    );

    let (api, _sync_rx) = crate::e2e_tests::boot(dir.path());
    let api_for_view = api.clone();
    let (settings, cx) = cx.add_window_view(|window, cx| {
        crate::e2e_tests::setup(cx);
        let sync_store = cx.new(|cx| crate::stores::sync::SyncStore::new(cx));
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, sync_store, api_for_view, window, cx)
    });
    crate::e2e_tests::wait_for(cx, |cx| {
        crate::e2e_tests::with_app(cx, |app| {
            (settings.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    // 切到外观分区（关闭行为行与主题/语言行同域渲染）——直驱视图状态
    // （导航 div 无 debug_selector，分区切换非本断言目标）
    crate::e2e_tests::with_app_mut(cx, |app| {
        settings.update(app, |view, cx| {
            view.active_section = crate::views::settings::SettingsSection::Appearance;
            cx.notify();
        });
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    for key in ["close-ask", "close-tray", "close-quit"] {
        assert!(
            cx.debug_bounds(key).is_some(),
            "关闭行为段 {key} 应存在（外观分区）"
        );
    }

    // 点击「最小化到托盘」→ 持久化 + 高亮切换
    let tray_bounds = cx.debug_bounds("close-tray").expect("tray 段应有 bounds");
    cx.simulate_click(
        gpui_pre::point(tray_bounds.center().x, tray_bounds.center().y),
        gpui_pre::Modifiers::default(),
    );
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        crate::ui_state::close_behavior(),
        crate::ui_state::CloseBehavior::ToTray,
        "点击托盘段应持久化关闭行为"
    );

    // 「退出程序」段同理
    let quit_bounds = cx.debug_bounds("close-quit").expect("quit 段应有 bounds");
    cx.simulate_click(
        gpui_pre::point(quit_bounds.center().x, quit_bounds.center().y),
        gpui_pre::Modifiers::default(),
    );
    cx.run_until_parked();
    assert_eq!(
        crate::ui_state::close_behavior(),
        crate::ui_state::CloseBehavior::Quit
    );

    // 收尾归零（Ask 清除记忆，防污染进程内后续测试）
    crate::ui_state::set_close_behavior(crate::ui_state::CloseBehavior::Ask);
}

/// 启动预载种子（线上反馈⑥）：注入预载数据的 AppShell **首帧**即有账号组
/// 与收件箱列表——只 draw 不 park（懒加载的异步任务尚未执行），证明数据
/// 来自种子注入而非异步回填。
#[gpui_pre::test]
fn preloaded_shell_first_frame_has_data(cx: &mut gpui_pre::TestAppContext) {
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    crate::e2e_tests::force_e2e_data_dir();
    let (api, sync_rx) = crate::e2e_tests::boot(dir.path());

    // 预载查询（与 open_main_window 同函数）：e2e 种子账号应取到非空数据
    let preload =
        crate::runtime::runtime().block_on(crate::app::preload_home(&api));
    assert!(!preload.groups.is_empty(), "种子账号应预载出账号组");
    let inbox_len = preload.inbox.as_ref().map(|r| r.emails.len()).unwrap_or(0);
    assert!(inbox_len > 0, "种子 INBOX 应预载出首页邮件");

    let (shell, cx) = cx.add_window_view(|window, cx| {
        crate::e2e_tests::setup(cx);
        crate::app::AppShell::new_with_preload(api, sync_rx, preload, window, cx)
    });
    // 只 draw 不 run_until_parked：懒加载 spawn 尚未跑，store 数据只能来自种子
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let (groups, emails) = cx.update(|_window: &mut gpui_kit::Window, app: &mut gpui_kit::App| {
        let shell_ref = shell.read(app);
        (
            shell_ref.accounts.read(app).groups.len(),
            shell_ref.emails.read(app).emails.len(),
        )
    });
    assert!(groups > 0, "首帧账号组应来自种子（实际 {groups}）");
    assert!(emails > 0, "首帧收件箱列表应来自种子（实际 {emails}）");
    assert!(
        !cx.update(|_w: &mut gpui_kit::Window, app: &mut gpui_kit::App| shell.read(app).emails.read(app).loading),
        "种子路径不应处于 loading 态"
    );
}
