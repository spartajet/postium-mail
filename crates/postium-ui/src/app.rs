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

/// 主窗口壳的全局注册（弱引用）：跨窗口（如设置页同步完成后）刷新邮件列表用。
pub struct MainShell(pub gpui_kit::WeakEntity<AppShell>);
impl gpui_kit::Global for MainShell {}

/// 托盘是否存活（setup_tray 成功时置 true）——关闭行为据此决定最小化还是退出。
pub static TRAY_ALIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 主题偏好的全局单源（dark | light | system）：主窗口与设置窗口共用，
/// 避免各自持内存副本导致"跟随系统"覆盖显式选择。
#[derive(Clone)]
pub struct ThemePref(pub std::sync::Arc<std::sync::Mutex<String>>);
impl gpui_kit::Global for ThemePref {}

/// 读当前主题偏好（全局单源；未初始化时回落 dark）。
pub fn current_theme_pref(cx: &App) -> String {
    cx.try_global::<ThemePref>()
        .map(|g| g.0.lock().map(|s| s.clone()).unwrap_or_else(|_| "dark".into()))
        .unwrap_or_else(|| "dark".into())
}

/// 写主题偏好（全局单源 + 持久化）。
pub fn set_theme_pref(pref: &str, cx: &mut App) {
    if let Some(g) = cx.try_global::<ThemePref>() {
        if let Ok(mut s) = g.0.lock() {
            *s = pref.to_string();
        }
    }
    let mut ui = crate::ui_state::load();
    ui.theme = Some(pref.to_string());
    crate::ui_state::save(&ui);
}

/// 打开主窗口（应用启动时调用一次）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut App) {
    let options = match crate::window_state::saved_bounds() {
        Some(bounds) => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        None => WindowOptions::default(),
    };
    let (_handle, shell_view) = gpui_kit::open_window(
        options,
        cx,
        |window, cx| cx.new(|cx| AppShell::new(api, sync_rx, window, cx)),
    )
    .expect("打开主窗口失败");
    // 注册全局弱引用（窗口关闭后自动失效）
    cx.set_global(MainShell(shell_view.downgrade()));
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
    /// 列表视图（导航/搜索上下文切换时整体重建）
    email_list: Option<gpui_kit::Entity<EmailListView>>,
    /// 已重建到的上下文版本（nav_version）
    list_nav_version: u64,
    /// 已同步到的数据版本（list_version；原位更新行，不重置滚动）
    list_data_version: u64,
    detail_view: Option<gpui_kit::Entity<EmailDetailView>>,
    detail_build_version: u32,
    /// 顶栏搜索
    search_input: gpui_kit::Entity<InputState>,
    /// 搜索防抖代数（每次变更 +1，超时后比对决定是否执行）
    search_gen: u64,
    pub nav: NavTarget,
    /// 主题偏好（dark | light | system），system 跟随系统外观
    pub(crate) theme_pref: String,
    /// 系统外观观察订阅（保持存活）
    _appearance_sub: Option<gpui_kit::Subscription>,
    /// 托盘句柄（保持存活）
    _tray: Option<tray_icon::TrayIcon>,
    pub(crate) calendar_view: Option<gpui_kit::Entity<crate::views::calendar::CalendarView>>,
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
        // 恢复持久化偏好（默认暗色，与旧版一致；system 跟随系统外观）
        let ui = crate::ui_state::load();
        let theme_pref = ui.theme.clone().unwrap_or_else(|| "dark".into());
        // 主题偏好全局单源（仅首个主窗口初始化；设置窗口复用）
        if cx.try_global::<ThemePref>().is_none() {
            cx.set_global(ThemePref(std::sync::Arc::new(std::sync::Mutex::new(
                theme_pref.clone(),
            ))));
        }
        let theme_mode = crate::theme::resolve_mode(&theme_pref, window.appearance());
        gpui_kit::component::Theme::change(theme_mode, None, cx);
        crate::theme::apply_brand(cx);
        // 系统外观变化时，"跟随系统"偏好自动重解析
        let appearance_sub = {
            let weak = cx.entity().downgrade();
            let sub = window.observe_window_appearance(move |window, cx| {
                let pref = current_theme_pref(cx);
                let _ = weak.update(cx, |_shell: &mut Self, cx| {
                    if pref == "system" {
                        let mode = crate::theme::resolve_mode("system", window.appearance());
                        crate::theme::apply_mode(mode, cx);
                        cx.notify();
                    }
                });
            });
            Some(sub)
        };
        if ui.lang.as_deref() == Some("en") {
            crate::i18n::set_lang(crate::i18n::Lang::En, cx);
        }

        // Toast 总线 + 关闭主窗口 → 隐藏到托盘（退出走托盘菜单）
        crate::notify::setup(window, cx);
        // gpui-pre 0.3.9 无窗口 hide API：点关闭 = 保存状态后最小化（退出走托盘菜单）
        // 关闭行为：托盘可用 → 保存状态并最小化（退出走托盘菜单）；
        // 托盘不可用（如无 appindicator 的环境）→ 真关闭，否则应用无法退出
        window.on_window_should_close(cx, |window, _| {
            crate::window_state::save(window);
            let tray_alive = TRAY_ALIVE.load(std::sync::atomic::Ordering::Relaxed);
            if tray_alive {
                window.minimize_window();
                false
            } else {
                true
            }
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
        // e2e 离线模式跳过后台调度（避免真实网络连接破坏测试确定性）
        if !api.is_e2e_offline() {
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
                    // 新视图无选中：详情面板清空，避免显示旧文件夹邮件
                    this.detail.update(cx, |store, cx| {
                        store.clear(cx);
                    });
                    this.detail_view = None;
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
                    emails: this.emails.downgrade(),
                    recipients_expanded: false,
                }));
                this.detail_build_version = this.emails.read(cx).selected_version;
                cx.notify();
            }
        })
        .detach();

        // 顶栏搜索：回车立即搜；输入变更 300ms 防抖自动搜（对齐旧版）
        cx.subscribe(&search_input, |this: &mut Self, input, event: &InputEvent, cx| {
            match event {
                InputEvent::PressEnter { .. } => {
                    this.search_gen += 1;
                    let q = input.read(cx).value().to_string();
                    let api = this.api.clone();
                    this.emails.update(cx, |store, cx| {
                        store.search(&q, &api, cx);
                    });
                    this.email_list = None;
                }
                InputEvent::Change => {
                    this.search_gen += 1;
                    let debounce_gen = this.search_gen;
                    let input = input.clone();
                    let api = this.api.clone();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(300))
                            .await;
                        this.update(cx, |shell, cx| {
                            if shell.search_gen != debounce_gen {
                                return; // 后续输入已接管
                            }
                            let q = input.read(cx).value().to_string();
                            let api = api;
                            shell.emails.update(cx, |store, cx| {
                                if q.trim().is_empty() {
                                    // 清空搜索 → 恢复当前导航视图
                                    store.navigate(shell.nav.clone(), &api, cx);
                                } else {
                                    store.search(&q, &api, cx);
                                }
                            });
                            shell.email_list = None;
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                }
                _ => {}
            }
        })
        .detach();

        let _tray = crate::tray::setup_tray(api.clone(), sync_store.clone(), cx);
        if _tray.is_some() {
            TRAY_ALIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        Self {
            api,
            sync_rx,
            accounts,
            sidebar,
            emails,
            detail,
            sync_store,
            email_list: None,
            list_nav_version: 0,
            list_data_version: 0,
            detail_view: None,
            detail_build_version: 0,
            search_input,
            search_gen: 0,
            nav: NavTarget::AllInbox,
            theme_pref,
            _appearance_sub: appearance_sub,
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
        let (mode, pref) = if dark {
            (gpui_kit::component::ThemeMode::Light, "light")
        } else {
            (gpui_kit::component::ThemeMode::Dark, "dark")
        };
        crate::theme::apply_mode(mode, cx);
        self.theme_pref = pref.to_string();
        let mut ui = crate::ui_state::load();
        ui.theme = Some(pref.to_string());
        crate::ui_state::save(&ui);
    }

}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 上下文切换（导航/搜索）→ 重建虚拟化列表；
        // 仅数据变化（翻页/操作后刷新）→ 原位更新行，保持滚动与选中
        let (data_v, nav_v) = {
            let store = self.emails.read(cx);
            (store.list_version, store.nav_version)
        };
        if self.list_nav_version != nav_v {
            let state = rebuild_list_state(&self.emails, &self.api, window, cx);
            let api = self.api.clone();
            self.email_list = Some(cx.new(|_| EmailListView {
                state,
                store: self.emails.clone(),
                api,
                older_syncing: false,
            }));
            self.list_nav_version = nav_v;
            self.list_data_version = data_v;
        } else if self.list_data_version != data_v {
            if let Some(view) = self.email_list.clone() {
                let rows: Vec<std::rc::Rc<postium_core::service::email_service::EmailDto>> =
                    self.emails.read(cx).emails.iter().cloned().map(std::rc::Rc::new).collect();
                view.update(cx, |_, cx| {
                    cx.notify();
                });
                let state = view.read(cx).state.clone();
                state.update(cx, |st, cx| {
                    let delegate = st.delegate_mut();
                    delegate.set_rows(rows);
                    cx.notify();
                });
            }
            self.list_data_version = data_v;
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
                                emails: self.emails.downgrade(),
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
                    .relative()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .bg(crate::theme::brand_gradient(cx))
                    // 品牌光球装饰层（最底，内容覆盖其上）
                    .child(crate::theme::orbs(cx))
                    .child(
                v_flex()
                    .relative()
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
                    )
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
