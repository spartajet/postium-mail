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
use crate::nav::NavTarget;
use crate::stores::accounts::init_accounts_store;
use crate::views::sidebar::{Sidebar, SidebarEvent};

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

/// 应用主壳实体。
pub struct AppShell {
    pub api: Arc<CoreApi>,
    #[allow(dead_code)] // Task 7 起消费
    pub sync_rx: SyncEventRx,
    pub accounts: gpui_kit::Entity<crate::stores::accounts::AccountsStore>,
    pub sidebar: gpui_kit::Entity<Sidebar>,
    pub nav: NavTarget,
}

impl AppShell {
    fn new(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut Context<Self>) -> Self {
        // 默认暗色主题（与旧版默认一致）
        gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);

        let accounts = init_accounts_store(&api, cx);
        let sidebar = cx.new(|cx| Sidebar {
            accounts: accounts.clone(),
            current: NavTarget::AllInbox,
        });

        // 侧栏导航 → 主壳状态
        cx.subscribe(&sidebar, |this: &mut Self, _, event: &SidebarEvent, cx| {
            if let SidebarEvent::Navigate(target) = event {
                this.nav = target.clone();
                cx.notify();
            }
        })
        .detach();

        Self { api, sync_rx, accounts, sidebar, nav: NavTarget::AllInbox }
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
        let nav_label: SharedString = match &self.nav {
            NavTarget::AllInbox => i18n::t("sidebar.inbox"),
            NavTarget::Category(c) => i18n::t(crate::nav::category_label_key(c)),
            NavTarget::AccountFolder { folder, .. } => folder.clone().into(),
            NavTarget::Label(id) => format!("标签 #{id}").into(),
            NavTarget::Calendar => i18n::t("sidebar.calendar"),
            NavTarget::Workflow => i18n::t("sidebar.workflow"),
            NavTarget::Settings => i18n::t("settings.title"),
        };

        h_flex()
            .size_full()
            // 侧栏
            .child(self.sidebar.clone())
            // 主区域（Task 4 起按 nav 渲染邮件列表）
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .p_3()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(nav_label),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(cx.theme().muted_foreground)
                            .child("邮件列表（Task 4 实装）"),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("lang")
                                .ghost()
                                .label("中/EN")
                                .on_click(cx.listener(Self::toggle_lang)),
                        ),
                    ),
            )
    }
}
