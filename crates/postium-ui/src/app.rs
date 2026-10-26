//! 主窗口壳：侧栏 + 邮件列表/详情主区域。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::*;

use postium_core::api::{CoreApi, SyncEventRx};

use crate::i18n::{self, Lang};
use crate::nav::NavTarget;
use crate::stores::accounts::init_accounts_store;
use crate::stores::emails::EmailsStore;
use crate::views::email_list::{EmailListView, rebuild_list_state};
use crate::views::sidebar::{Sidebar, SidebarEvent};

/// 打开主窗口（应用启动时调用一次）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut App) {
    gpui_kit::open_window(
        WindowOptions::default(),
        cx,
        |window, cx| cx.new(|cx| AppShell::new(api, sync_rx, window, cx)),
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
    pub emails: gpui_kit::Entity<EmailsStore>,
    /// 列表视图（render 时按 list_version 重建）
    email_list: Option<gpui_kit::Entity<EmailListView>>,
    list_build_version: u64,
    /// 顶栏搜索
    search_input: gpui_kit::Entity<InputState>,
    pub nav: NavTarget,
}

impl AppShell {
    fn new(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 默认暗色主题（与旧版默认一致）
        gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);

        let accounts = init_accounts_store(&api, cx);
        let sidebar = cx.new(|cx| Sidebar {
            accounts: accounts.clone(),
            current: NavTarget::AllInbox,
        });

        let emails = cx.new(|cx| {
            let mut store = EmailsStore::new(cx);
            store.navigate(NavTarget::AllInbox, &api, cx);
            store
        });
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n::t("email.search"))
        });

        // 侧栏导航 → 更新壳 + 装载邮件列表
        cx.subscribe(&sidebar, |this: &mut Self, _, event: &SidebarEvent, cx| {
            if let SidebarEvent::Navigate(target) = event {
                this.nav = target.clone();
                this.emails.update(cx, |store, cx| {
                    store.navigate(target.clone(), &this.api, cx);
                });
                this.email_list = None; // 等 Reloaded 重建
                cx.notify();
            }
        })
        .detach();

        // 顶栏搜索
        cx.subscribe(&search_input, |this: &mut Self, input, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let q = input.read(cx).value().to_string();
                this.emails.update(cx, |store, cx| {
                    store.search(&q, &this.api, cx);
                });
                this.email_list = None;
            }
        })
        .detach();

        Self {
            api,
            sync_rx,
            accounts,
            sidebar,
            emails,
            email_list: None,
            list_build_version: 0,
            search_input,
            nav: NavTarget::AllInbox,
        }
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 列表数据版本变化 → 重建虚拟化列表（render 内建 entity，版本守卫确保只建一次）
        let current_version = self.emails.read(cx).list_version;
        if self.list_build_version != current_version {
            let state = rebuild_list_state(&self.emails, &self.api, window, cx);
            self.email_list = Some(cx.new(|_| EmailListView {
                state,
                store: self.emails.clone(),
            }));
            self.list_build_version = current_version;
        }
        let nav_label: SharedString = match &self.nav {
            NavTarget::AllInbox => i18n::t("sidebar.inbox"),
            NavTarget::Category(c) => i18n::t(crate::nav::category_label_key(c)),
            NavTarget::AccountFolder { folder, .. } => folder.clone().into(),
            NavTarget::Label(id) => format!("标签 #{id}").into(),
            NavTarget::Calendar => i18n::t("sidebar.calendar"),
            NavTarget::Workflow => i18n::t("sidebar.workflow"),
            NavTarget::Settings => i18n::t("settings.title"),
        };

        // 主区域：日历/工作流/设置占位，其余渲染邮件列表
        let main: AnyElement = match &self.nav {
            NavTarget::Calendar => placeholder_pane(&i18n::t("sidebar.calendar"), cx).into_any_element(),
            NavTarget::Workflow => placeholder_pane(&i18n::t("sidebar.workflow"), cx).into_any_element(),
            NavTarget::Settings => placeholder_pane(&i18n::t("settings.title"), cx).into_any_element(),
            _ => match &self.email_list {
                Some(list) => list.clone().into_any_element(),
                None => placeholder_pane(&i18n::t("email.loading"), cx).into_any_element(),
            },
        };

        h_flex()
            .size_full()
            .child(self.sidebar.clone())
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .p_2()
                    .gap_2()
                    // 顶栏：搜索 + 当前视图标题 + 语言/主题
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .w(px(360.))
                                    .child(Input::new(&self.search_input)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(nav_label),
                            )
                            .child(
                                h_flex()
                                    .ml_auto()
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
                    .child(main),
            )
    }
}

fn placeholder_pane(text: &SharedString, cx: &gpui_kit::App) -> impl IntoElement {
    let _ = cx;
    v_flex()
        .flex_1()
        .size_full()
        .items_center()
        .justify_center()
        .child(text.clone())
}
