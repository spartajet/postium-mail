//! 主窗口壳：侧栏 + 主区域（各视图由后续任务充实）。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::*;

use postium_core::api::{CoreApi, SyncEventRx};

use crate::i18n::{self, Lang};

/// 打开主窗口（应用启动时调用一次）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut App) {
    gpui_kit::open_window(
        WindowOptions::default(),
        cx,
        |_, cx| cx.new(|cx| AppShell::new(api, sync_rx, cx)),
    )
    .expect("打开主窗口失败");
    cx.activate(true);
}

/// 应用主壳实体：持有核心服务与全局状态，后续任务逐步充实。
pub struct AppShell {
    #[allow(dead_code)] // Task 3+ 起消费
    api: Arc<CoreApi>,
    #[allow(dead_code)] // Task 7 起消费
    sync_rx: SyncEventRx,
}

impl AppShell {
    fn new(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut Context<Self>) -> Self {
        // 默认暗色主题（与旧版默认一致）
        gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
        Self { api, sync_rx }
    }

    fn toggle_lang(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<i18n::I18nGlobal>().lang;
        let next = if current == Lang::Zh { Lang::En } else { Lang::Zh };
        i18n::set_lang(next, cx);
    }

    fn toggle_theme(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let dark = cx.theme().mode == gpui_kit::component::ThemeMode::Dark;
        gpui_kit::component::Theme::change(
            if dark {
                gpui_kit::component::ThemeMode::Light
            } else {
                gpui_kit::component::ThemeMode::Dark
            },
            None,
            cx,
        );
    }
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            // 侧栏（Task 3 实装）
            .child(
                v_flex()
                    .w(px(220.))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child(i18n::t("app.name")),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(cx.theme().muted_foreground)
                            .child(i18n::t("email.search")),
                    ),
            )
            // 主区域（Task 4 实装邮件列表）
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(cx.theme().muted_foreground)
                            .child("邮件列表（Task 4 实装）"),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("lang")
                                    .ghost()
                                    .label("中/EN")
                                    .on_click(cx.listener(Self::toggle_lang)),
                            )
                            .child(
                                Button::new("theme")
                                    .ghost()
                                    .label(i18n::t("settings.theme"))
                                    .on_click(cx.listener(Self::toggle_theme)),
                            ),
                    ),
            )
    }
}
