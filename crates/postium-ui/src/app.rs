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
use crate::stores::detail::DetailStore;
use crate::stores::sync::SyncStore;
use crate::stores::emails::{EmailsEvent, EmailsStore};
use crate::views::email_detail::EmailDetailView;
use crate::views::status_bar::StatusBar;
use crate::views::email_list::{EmailListView, rebuild_list_state};
use crate::views::sidebar::{Sidebar, SidebarEvent};

/// 打开主窗口（应用启动时调用一次）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut App) {
    let options = match crate::window_state::saved_bounds() {
        Some(bounds) => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        None => WindowOptions::default(),
    };
    gpui_kit::open_window(
        options,
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
    pub detail: gpui_kit::Entity<DetailStore>,
    pub sync_store: gpui_kit::Entity<SyncStore>,
    /// 列表视图（render 时按 list_version 重建）
    email_list: Option<gpui_kit::Entity<EmailListView>>,
    list_build_version: u64,
    detail_view: Option<gpui_kit::Entity<EmailDetailView>>,
    detail_build_version: u32,
    /// 顶栏搜索
    search_input: gpui_kit::Entity<InputState>,
    pub nav: NavTarget,
    /// 托盘句柄（保持存活）
    _tray: Option<tray_icon::TrayIcon>,
    calendar_view: Option<gpui_kit::Entity<crate::views::calendar::CalendarView>>,
    workflow_view: Option<gpui_kit::Entity<crate::views::workflow::WorkflowView>>,
    status_bar: Option<gpui_kit::Entity<crate::views::status_bar::StatusBar>>,
}

impl AppShell {
    pub(crate) fn new(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 恢复持久化偏好（默认暗色，与旧版一致）
        let ui = crate::ui_state::load();
        let theme_mode = match ui.theme.as_deref() {
            Some("light") => gpui_kit::component::ThemeMode::Light,
            _ => gpui_kit::component::ThemeMode::Dark,
        };
        gpui_kit::component::Theme::change(theme_mode, None, cx);
        if ui.lang.as_deref() == Some("en") {
            crate::i18n::set_lang(crate::i18n::Lang::En, cx);
        }

        // Toast 总线 + 关闭主窗口 → 隐藏到托盘（退出走托盘菜单）
        crate::notify::setup(window, cx);
        // gpui-pre 0.3.9 无窗口 hide API：点关闭 = 保存状态后最小化（退出走托盘菜单）
        window.on_window_should_close(cx, |window, _| {
            crate::window_state::save(window);
            window.minimize_window();
            false
        });

        let accounts = init_accounts_store(&api, cx);
        let sidebar = cx.new(|_cx| Sidebar {
            accounts: accounts.clone(),
            current: NavTarget::AllInbox,
        });

        let emails = cx.new(|cx| {
            let mut store = EmailsStore::new(cx);
            store.navigate(NavTarget::AllInbox, &api, cx);
            store
        });
        let detail = cx.new(|cx| DetailStore::new(cx));

        // 同步事件消费（core 返回的接收端）+ 后台调度器
        let (sched_tx, sched_rx) = crate::stores::sync::sync_event_channel();
        let sync_store = cx.new(|cx| {
            let mut store = SyncStore::new(cx);
            let weak = cx.entity().downgrade();
            SyncStore::spawn_consumer(weak, sched_rx, cx);
            store.progress.clear();
            store
        });
        {
            let db = api.db.clone();
            let auth = api.auth.clone();
            let mut scheduler = postium_core::domain::sync::SyncScheduler::new(db, auth, Some(sched_tx));
            scheduler = scheduler.with_interval(5);
            scheduler.start();
            std::mem::forget(scheduler); // 调度器随进程存活（Drop 会停止）
        }
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n::t("email.search"))
        });

        // 侧栏导航 → 更新壳 + 装载邮件列表
        cx.subscribe(&sidebar, |this: &mut Self, _, event: &SidebarEvent, cx| {
            match event {
                SidebarEvent::Navigate(target) => {
                    this.nav = target.clone();
                    this.emails.update(cx, |store, cx| {
                        store.navigate(target.clone(), &this.api, cx);
                    });
                    this.email_list = None; // 等 Reloaded 重建
                    cx.notify();
                }
                SidebarEvent::Settings => {
                    let api = this.api.clone();
                    let accounts = this.accounts.clone();
                    cx.defer(move |cx| {
                        crate::views::settings::open_settings_window(&accounts, &api, cx);
                    });
                }
                SidebarEvent::Compose => {
                    let api = this.api.clone();
                    cx.defer(move |cx| {
                        crate::views::compose::open_compose_window(
                            crate::views::compose::ComposeMode::New,
                            &api,
                            cx,
                        );
                    });
                }
            }
        })
        .detach();

        // 选中邮件 → 装载详情（视图按选中版本重建，保住展开态/滚动）
        cx.subscribe(&emails, |this: &mut Self, _, event: &EmailsEvent, cx| {
            if let EmailsEvent::SelectionChanged(id) = event {
                this.detail.update(cx, |store, cx| {
                    store.load(*id, &this.api, cx);
                });
                this.detail_view = Some(cx.new(|_| EmailDetailView {
                    store: this.detail.clone(),
                    api: this.api.clone(),
                    recipients_expanded: false,
                }));
                this.detail_build_version = this.emails.read(cx).selected_version;
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

        let _tray = crate::tray::setup_tray(api.clone(), sync_store.clone(), cx);

        Self {
            api,
            sync_rx,
            accounts,
            sidebar,
            emails,
            detail,
            sync_store,
            email_list: None,
            list_build_version: 0,
            detail_view: None,
            detail_build_version: 0,
            search_input,
            nav: NavTarget::AllInbox,
            _tray,
            calendar_view: None,
            workflow_view: None,
            status_bar: None,
        }
    }

    fn toggle_lang(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<i18n::I18nGlobal>().lang;
        let next = if current == Lang::Zh { Lang::En } else { Lang::Zh };
        i18n::set_lang(next, cx);
        let mut ui = crate::ui_state::load();
        ui.lang = Some(match next { Lang::Zh => "zh".into(), Lang::En => "en".into() });
        crate::ui_state::save(&ui);
    }

    pub(crate) fn toggle_theme(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let dark = cx.theme().mode == gpui_kit::component::ThemeMode::Dark;
        let mode = if dark {
            gpui_kit::component::ThemeMode::Light
        } else {
            gpui_kit::component::ThemeMode::Dark
        };
        gpui_kit::component::Theme::change(mode, None, cx);
        let mut ui = crate::ui_state::load();
        ui.theme = Some(if dark { "light".into() } else { "dark".into() });
        crate::ui_state::save(&ui);
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 列表数据版本变化 → 重建虚拟化列表（render 内建 entity，版本守卫确保只建一次）
        let current_version = self.emails.read(cx).list_version;
        if self.list_build_version != current_version {
            let state = rebuild_list_state(&self.emails, &self.api, window, cx);
            let api = self.api.clone();
            self.email_list = Some(cx.new(|_| EmailListView {
                state,
                store: self.emails.clone(),
                api,
                older_syncing: false,
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
            NavTarget::Calendar => {
                if self.calendar_view.is_none() {
                    let api = self.api.clone();
                    self.calendar_view = Some(cx.new(|cx| crate::views::calendar::CalendarView::new(api, cx)));
                }
                self.calendar_view.clone().unwrap().into_any_element()
            }
            NavTarget::Workflow => {
                if self.workflow_view.is_none() {
                    let api = self.api.clone();
                    self.workflow_view = Some(cx.new(|cx| crate::views::workflow::WorkflowView::new(api, cx)));
                }
                self.workflow_view.clone().unwrap().into_any_element()
            }
            NavTarget::Settings => placeholder_pane(&i18n::t("settings.title"), cx).into_any_element(),
            _ => h_flex()
                .flex_1()
                .size_full()
                .overflow_hidden()
                .child(match &self.email_list {
                    Some(list) => div().flex_1().size_full().child(list.clone()).into_any_element(),
                    None => placeholder_pane(&i18n::t("email.loading"), cx).into_any_element(),
                })
                .child(
                    match self.detail_view.clone() {
                        Some(view) => view.into_any_element(),
                        None => {
                            // 首帧兜底：建一次并缓存（后续由 SelectionChanged 重建）
                            let view = cx.new(|_| EmailDetailView {
                                store: self.detail.clone(),
                                api: self.api.clone(),
                                recipients_expanded: false,
                            });
                            self.detail_view = Some(view.clone());
                            view.into_any_element()
                        }
                    },
                )
                .into_any_element(),
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
                                    .w(px(420.))
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
                    .child(div().flex_1().overflow_hidden().child(main))
                    .child(match self.status_bar.clone() {
                        Some(bar) => bar.into_any_element(),
                        None => {
                            let bar = cx.new(|_| StatusBar {
                                sync: self.sync_store.clone(),
                                api: self.api.clone(),
                                accounts: self.accounts.clone(),
                            });
                            self.status_bar = Some(bar.clone());
                            bar.into_any_element()
                        }
                    }),
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
