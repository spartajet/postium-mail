//! 主窗口壳：侧栏 + 邮件列表/详情主区域。

use std::sync::Arc;

use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    v_flex,
    ActiveTheme as _, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit_assets::IconName;

use postium_core::api::{CoreApi, SyncEventRx};

use crate::i18n::{self};
use crate::nav::NavTarget;
use crate::stores::accounts::init_accounts_store;
use crate::stores::detail::{DetailEvent, DetailStore};
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
/// 主窗口默认标题。
pub const MAIN_WINDOW_TITLE: &str = "Postium Mail";

/// 由详情状态推导窗口标题：有详情显示「主题 — Postium Mail」，无则默认。
pub(crate) fn window_title_for(
    detail: Option<&postium_core::service::email_service::EmailDetail>,
) -> String {
    let subject = detail
        .and_then(|d| d.email.subject.clone())
        .filter(|s| !s.trim().is_empty());
    match subject {
        Some(s) => format!("{s} — {MAIN_WINDOW_TITLE}"),
        None => MAIN_WINDOW_TITLE.to_string(),
    }
}

/// 启动预载首屏数据（线上反馈⑥）：主窗口打开前取入内存，首帧即有侧栏
/// 账号/统计与收件箱列表——免「窗口空白约两秒等查询回填」（全部查询经
/// 单条 tokio_rusqlite 连接 FIFO，启动期与其他写入竞争时首屏回填更慢）。
///
/// 空结果（无账号/查询失败）等价 [`PreloadedHome::default`]，构造侧回退
/// 懒加载路径，行为与预载前一致。
pub(crate) struct PreloadedHome {
    pub groups: Vec<crate::stores::accounts::AccountNode>,
    pub inbox: Option<postium_core::service::email_service::EmailListResponse>,
}

impl Default for PreloadedHome {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            inbox: None,
        }
    }
}

/// 预载查询：与 AccountsStore::reload / EmailsStore 首次 navigate 完全
/// 同序列（账号 → 每账号统计/标签 → 收件箱首页）。
pub(crate) async fn preload_home(api: &Arc<CoreApi>) -> PreloadedHome {
    use crate::stores::emails::PAGE_LIMIT;
    let accounts = api.list_accounts().await.unwrap_or_default();
    let mut groups = Vec::with_capacity(accounts.len());
    for account in accounts {
        let stats = api
            .get_folder_stats(account.id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(target: "postium::ui", id = account.id, "预载文件夹统计失败: {e}");
                Vec::new()
            });
        let labels = api.list_labels(account.id).await.unwrap_or_else(|e| {
            tracing::warn!(target: "postium::ui", id = account.id, "预载标签失败: {e}");
            Vec::new()
        });
        groups.push(crate::stores::accounts::AccountNode {
            account,
            stats,
            labels,
        });
    }
    let inbox = api
        .list_emails_by_category_for_all_accounts(
            postium_core::service::email_service::EmailCategory::Inbox,
            0,
            PAGE_LIMIT,
            false,
        )
        .await
        .ok();
    PreloadedHome { groups, inbox }
}

/// 打开主窗口。
///
/// `sync_rx` 为主窗口的同步事件接收端；`None` 时（托盘重建已关闭的
/// 主窗口）使用空通道占位——后台调度的消费循环随旧窗口销毁，重建窗口
/// 不再接收调度事件（同步状态以列表轮询刷新为准）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: Option<SyncEventRx>, cx: &mut App) {
    // 启动预载（线上反馈⑥）：窗口出现即带数据。此时尚无窗口，block_on
    // 不冻结任何 UI；预载耗时入日志（后续性能观察锚点）。
    let preload_started = std::time::Instant::now();
    let preload = crate::runtime::runtime().block_on(preload_home(&api));
    tracing::info!(
        target: "postium::ui",
        "启动预载完成：账号组 {} / 收件箱首页 {} 封（{:?}）",
        preload.groups.len(),
        preload.inbox.as_ref().map(|r| r.emails.len()).unwrap_or(0),
        preload_started.elapsed()
    );
    let sync_rx = sync_rx
        .unwrap_or_else(|| crate::stores::sync::sync_event_channel().1);
    let mut options = match crate::window_state::saved_bounds() {
        Some(bounds) => WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        None => WindowOptions::default(),
    };
    // 截图模式：窗口显示但不抢焦点（Craft Anything 检查期间不打扰用户）。
    // 未聚焦窗口默认按 ~1fps 节流，会拖慢 120 帧目标 → 提到 60fps。
    if crate::screenshot::enabled() {
        options.focus = false;
        options.show = true;
        options.inactive_frame_interval = Some(std::time::Duration::from_millis(16));
    }
    // 原生标题栏深浅只随系统外观（Windows 上无法应用内切换），改为透明 +
    // 应用自绘标题栏（views::title_bar，背景跟随主题 title_bar 色）
    options.titlebar = Some(TitlebarOptions {
        appears_transparent: true,
        ..Default::default()
    });
    let (_handle, shell_view) = gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| AppShell::new_with_preload(api, sync_rx, preload, window, cx))
    })
    .expect("打开主窗口失败");
    // 注册全局弱引用（窗口关闭后自动失效）
    cx.set_global(MainShell(shell_view.downgrade()));
    if !crate::screenshot::enabled() {
        cx.activate(true);
    }
}

/// 应用主壳实体。
pub struct AppShell {
    pub api: Arc<CoreApi>,
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
    pub(crate) detail_view: Option<gpui_kit::Entity<EmailDetailView>>,
    detail_build_version: u32,
    /// 邮件详情「联系人」联动传递的待选邮箱：切到联系人页后由渲染期消费
    /// （视图可能尚未懒建，选中须等 store 首装完成）
    pub(crate) contacts_pending_email: Option<String>,
    /// 顶栏搜索
    search_input: gpui_kit::Entity<InputState>,
    /// 搜索防抖代数（每次变更 +1，超时后比对决定是否执行）
    search_gen: u64,
    pub nav: NavTarget,
    /// 系统外观观察订阅（保持存活）
    _appearance_sub: Option<gpui_kit::Subscription>,
    /// 托盘句柄（保持存活）
    _tray: Option<tray_icon::TrayIcon>,
    pub(crate) calendar_view: Option<gpui_kit::Entity<crate::views::calendar::CalendarView>>,
    pub(crate) contacts_view: Option<gpui_kit::Entity<crate::views::contacts::ContactsView>>,
    workflow_view: Option<gpui_kit::Entity<crate::views::workflow::WorkflowView>>,
    /// 主窗口句柄：选中邮件时更新窗口标题用
    pub(crate) window_handle: gpui_kit::AnyWindowHandle,
    /// 关闭选择弹窗（线上反馈⑤）：关闭行为未记忆时，X 按钮弹窗询问
    /// 托盘驻留还是退出。弹窗期间阻断窗口关闭动作本体。
    close_dialog: bool,
    /// 弹窗「记住我的选择」勾选态（每次打开重置）。
    close_dialog_remember: bool,
    /// 同步中节流刷新：上次刷新时刻 / 到期未刷标记
    sync_refresh_at: Option<std::time::Instant>,
    sync_refresh_pending: bool,
    status_bar: Option<gpui_kit::Entity<crate::views::status_bar::StatusBar>>,
}

impl AppShell {
    pub(crate) fn new(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_preload(api, sync_rx, PreloadedHome::default(), window, cx)
    }

    /// [`Self::new`] 的预载形态（线上反馈⑥）：主窗口打开前已取好的首屏
    /// 数据直接注入 store，首帧即有内容；空预载回退懒加载（e2e 同路径）。
    pub(crate) fn new_with_preload(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        preload: PreloadedHome,
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
        // 正文划选复制（线上反馈①）：Ctrl+C 在 EmailHtmlBody 焦点上下文内派发
        cx.bind_keys([
            gpui_kit::KeyBinding::new(
                "ctrl-c",
                crate::views::email_detail::CopyHtmlSelection,
                Some("EmailHtmlBody"),
            ),
            gpui_kit::KeyBinding::new(
                "escape",
                crate::views::email_detail::ClearHtmlSelection,
                Some("EmailHtmlBody"),
            ),
        ]);
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

        // 启动预载注入（线上反馈⑥）：有种子 → 首帧即有数据（免 reload/
        // navigate 异步回填的空白窗口）；空种子 → 懒加载原路径。
        let accounts = if preload.groups.is_empty() {
            init_accounts_store(&api, cx)
        } else {
            cx.new(|_cx| crate::stores::accounts::AccountsStore::with_preloaded(preload.groups))
        };
        let sidebar = cx.new(|_cx| Sidebar {
            accounts: accounts.clone(),
            current: NavTarget::AllInbox,
        });

        let emails = match preload.inbox {
            Some(resp) if !resp.emails.is_empty() => {
                cx.new(|_cx| EmailsStore::seeded_inbox(resp))
            }
            _ => cx.new(|cx| {
                let mut store = EmailsStore::new(cx);
                store.navigate(NavTarget::AllInbox, &api, cx);
                store
            }),
        };
        let detail = cx.new(|cx| DetailStore::new(cx));

        // 同步事件消费（core 返回的接收端）+ 后台调度器
        let (sched_tx, sched_rx) = crate::stores::sync::sync_event_channel();
        let sync_store = cx.new(|cx| {
            let mut store = SyncStore::new(cx);
            let weak = cx.entity().downgrade();
            SyncStore::spawn_consumer(weak.clone(), sched_rx, cx);
            // 引擎默认事件出口（CoreApi.sync_tx 的 rx）：IDLE EXISTS 触发与
            // 手动/定时 SyncNow 的进度事件都发到这里，并入同一消费端驱动
            // SyncEvent::Progress → 节流刷新（此前该 rx 无消费者，事件被丢弃）
            SyncStore::spawn_consumer(weak, sync_rx, cx);
            store.progress.clear();
            store
        });
        // e2e 离线模式跳过后台调度（避免真实网络连接破坏测试确定性）
        if !api.is_e2e_offline() {
            let db = api.db.clone();
            let mut scheduler = postium_core::domain::sync::SyncScheduler::new(
                db,
                api.engine.clone(),
                Some(sched_tx),
            );
            scheduler = scheduler.with_interval(5);
            scheduler.start();
            std::mem::forget(scheduler); // 调度器随进程存活（Drop 会停止）
        }

        // 同步进行中：节流刷新邮件列表与侧栏徽章（边同步边更新 UI）
        cx.subscribe(&sync_store, |this: &mut Self, _, event: &crate::stores::sync::SyncEvent, cx| {
            if matches!(event, crate::stores::sync::SyncEvent::Progress) {
                this.refresh_lists_throttled(cx);
            }
        })
        .detach();

        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(i18n::t("email.search"))
        });

        // 侧栏导航 → 更新壳 + 装载邮件列表
        cx.subscribe(&sidebar, |this: &mut Self, _, event: &SidebarEvent, cx| {
            match event {
                SidebarEvent::Navigate(target) => {
                    this.nav = target.clone();
                    // 浏览具体文件夹 → 通知该账号引擎 IDLE watch（最近浏览 LRU，
                    // 新邮件秒级推送；fire-and-forget，失败不影响导航）
                    if let NavTarget::AccountFolder { account_id, folder } = &target {
                        let api = this.api.clone();
                        let (account_id, folder) = (*account_id, folder.clone());
                        crate::runtime::spawn_tokio(async move {
                            if let Err(e) = api.watch_folder(account_id, folder).await {
                                tracing::debug!(target: "postium::ui", "WatchFolder 发送失败: {e}");
                            }
                        });
                    }
                    this.emails.update(cx, |store, cx| {
                        store.navigate(target.clone(), &this.api, cx);
                    });
                    // 新视图无选中：详情面板清空，避免显示旧文件夹邮件
                    this.detail.update(cx, |store, cx| {
                        store.clear(cx);
                    });
                    this.update_window_title(cx);
                    this.detail_view = None;
                    this.email_list = None; // 等 Reloaded 重建
                    cx.notify();
                }
                SidebarEvent::Settings => {
                    let api = this.api.clone();
                    let accounts = this.accounts.clone();
                    let sync = this.sync_store.clone();
                    cx.defer(move |cx| {
                        crate::views::settings::open_settings_window(&accounts, &sync, &api, cx);
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
            match event {
                EmailsEvent::SelectionChanged(id) => {
                    this.detail.update(cx, |store, cx| {
                        store.load(*id, &this.api, cx);
                    });
                    this.detail_view = Some(cx.new(|cx| EmailDetailView {
                        store: this.detail.clone(),
                        api: this.api.clone(),
                        emails: this.emails.downgrade(),
                        recipients_expanded: false,
                        html_selection: Default::default(),
                        header_selection: Default::default(),
                        body_focus: cx.focus_handle(),
                    }));
                    this.detail_build_version = this.emails.read(cx).selected_version;
                    cx.notify();
                }
                // 标已读/未读后刷新文件夹统计（侧栏未读徽章）
                EmailsEvent::UnreadChanged => {
                    this.accounts.update(cx, |store, cx| {
                        store.reload(&this.api, cx);
                    });
                }
                EmailsEvent::Reloaded => {}
            }
        })
        .detach();

        // 详情装载完成 → 窗口标题显示邮件主题（无详情时恢复默认）
        cx.subscribe(&detail, |this: &mut Self, _, event: &DetailEvent, cx| {
            if matches!(event, DetailEvent::Loaded) {
                this.update_window_title(cx);
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

        let shell = Self {
            api,
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
            contacts_pending_email: None,
            search_input,
            search_gen: 0,
            nav: NavTarget::AllInbox,
            _appearance_sub: appearance_sub,
            _tray,
            calendar_view: None,
            contacts_view: None,
            workflow_view: None,
            status_bar: None,
            close_dialog: false,
            close_dialog_remember: false,
            window_handle: window.window_handle(),
            sync_refresh_at: None,
            sync_refresh_pending: false,
        };

        // 启动即回填：把存量邮件的往来地址聚合进联系人表（fire-and-forget，
        // 不阻塞首帧）。幂等且对 local/dav 条目只推进往来时间，不覆盖用户数据。
        let backfill_api = shell.api.clone();
        let _ = crate::runtime::spawn_tokio(async move {
            let _ = backfill_api.backfill_contacts().await;
        });

        shell
    }

    /// 同步进行中的节流刷新（SyncEvent::Progress 驱动）：
    /// 距上次刷新 ≥2s 立即刷；否则标记 pending、1.5s 后补刷一次，
    /// 避免高频事件拖垮列表重绘，同时保证收尾必有最后一次刷新。
    fn refresh_lists_throttled(&mut self, cx: &mut Context<Self>) {
        const MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
        const DEFER: std::time::Duration = std::time::Duration::from_millis(1500);
        let now = std::time::Instant::now();
        let due = self
            .sync_refresh_at
            .map_or(true, |t| now.duration_since(t) >= MIN_INTERVAL);
        if due {
            self.sync_refresh_at = Some(now);
            self.sync_refresh_pending = false;
            self.refresh_lists_now(cx);
            return;
        }
        if self.sync_refresh_pending {
            return;
        }
        self.sync_refresh_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEFER).await;
            this.update(cx, |this, cx| {
                if !this.sync_refresh_pending {
                    return;
                }
                this.sync_refresh_pending = false;
                this.sync_refresh_at = Some(std::time::Instant::now());
                this.refresh_lists_now(cx);
            })
            .ok();
        })
        .detach();
    }

    /// 立即刷新邮件列表与侧栏徽章（文件夹统计）。
    fn refresh_lists_now(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.emails.update(cx, |store, cx| {
            store.refresh_now(&api, cx);
        });
        self.accounts.update(cx, |store, cx| {
            store.reload(&api, cx);
        });
        cx.notify();
    }

    /// 更新窗口标题：选中邮件时显示「邮件主题 — Postium Mail」，
    /// 无选中（详情空）时恢复默认标题。
    pub(crate) fn update_window_title(&mut self, cx: &mut Context<Self>) {
        // 自绘标题栏的文字由 render 读取 detail 推导（window_title_for），
        // 这里 notify 触发重绘；系统窗口标题（任务栏显示）同步更新。
        cx.notify();
        let title = window_title_for(self.detail.read(cx).detail.as_ref());
        let _ = self.window_handle.update(cx, |_, window, _| {
            window.set_window_title(&title);
        });
    }

    /// 深浅主题互切（e2e 主题持久化测试与状态栏按钮共用逻辑入口）。
    pub(crate) fn toggle_theme(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        crate::theme::toggle_theme_mode(cx);
    }

    // -----------------------------------------------------------------
    // 关闭主窗口行为（线上反馈⑤）：托盘驻留 / 退出 / 每次询问弹窗
    // -----------------------------------------------------------------

    /// 标题栏 X：按持久化行为分流。
    pub(crate) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match crate::ui_state::close_behavior() {
            crate::ui_state::CloseBehavior::Quit => self.close_quit(window, cx),
            crate::ui_state::CloseBehavior::ToTray => self.close_to_tray(window, cx),
            crate::ui_state::CloseBehavior::Ask => {
                self.close_dialog = true;
                self.close_dialog_remember = false;
                cx.notify();
            }
        }
    }

    /// 最小化到托盘：保存窗口状态后销毁窗口——应用驻留（QuitMode::
    /// Explicit 下无窗口不退出），托盘「显示窗口」重建主窗口。销毁需
    /// 异步（事件分发栈上同步移除自身窗口不安全）。
    fn close_to_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::window_state::save(window);
        let handle = window.window_handle();
        cx.spawn(async move |_, cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        })
        .detach();
    }

    /// 退出程序：保存窗口状态后显式退出（托盘菜单退出同款路径）。
    fn close_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::window_state::save(window);
        cx.quit();
    }

    /// 关闭选择弹窗——「最小化到托盘」。
    pub(crate) fn dialog_close_to_tray(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_dialog_remember {
            crate::ui_state::set_close_behavior(crate::ui_state::CloseBehavior::ToTray);
        }
        self.close_dialog = false;
        self.close_to_tray(window, cx);
    }

    /// 关闭选择弹窗——「退出程序」。
    pub(crate) fn dialog_close_quit(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_dialog_remember {
            crate::ui_state::set_close_behavior(crate::ui_state::CloseBehavior::Quit);
        }
        self.close_dialog = false;
        self.close_quit(window, cx);
    }

    /// 关闭选择弹窗——「取消」。
    pub(crate) fn dialog_close_cancel(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.close_dialog = false;
        cx.notify();
    }

    /// 关闭选择弹窗——勾选「记住我的选择」。
    pub(crate) fn dialog_toggle_remember(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.close_dialog_remember = !self.close_dialog_remember;
        cx.notify();
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // --screenshot 模式：每帧计数 + 自驱动画下一帧（窗口未聚焦时
        // gpui 默认按事件驱动，不 notify 就停在第一帧）；截图线程完成后请求退出
        if crate::screenshot::enabled() {
            crate::screenshot::on_frame_tick();
            if crate::screenshot::quit_requested() {
                tracing::info!("screenshot 完成，退出");
                cx.quit();
            } else {
                cx.notify();
            }
        }
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
            NavTarget::AccountFolder { folder, .. } => {
                // 角色文件夹的类别键（"sent" 等）显示标准标签，原始名原样显示
                crate::nav::folder_label_key(folder)
                    .map(i18n::t)
                    .unwrap_or_else(|| folder.clone().into())
            }
            NavTarget::Label(id) => format!("标签 #{id}").into(),
            NavTarget::Calendar => i18n::t("sidebar.calendar"),
            NavTarget::Workflow => i18n::t("sidebar.workflow"),
            NavTarget::Contacts => i18n::t("sidebar.contacts"),
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
            NavTarget::Contacts => {
                if self.contacts_view.is_none() {
                    let api = self.api.clone();
                    self.contacts_view =
                        Some(cx.new(|cx| crate::views::contacts::ContactsView::new(api, window, cx)));
                }
                // 邮件详情联动传递的待选邮箱：视图就绪后定向选中
                // （store 清过滤全量重载，装载完成按邮箱命中即选中）
                if let Some(email) = self.contacts_pending_email.take() {
                    let api = self.api.clone();
                    if let Some(view) = self.contacts_view.clone() {
                        view.update(cx, |view, cx| {
                            view.store.update(cx, |store, cx| {
                                store.select_by_email(&email, &api, cx);
                            });
                        });
                    }
                }
                self.contacts_view.clone().unwrap().into_any_element()
            }
            NavTarget::Settings => placeholder_pane(&i18n::t("settings.title"), cx).into_any_element(),
            _ => h_resizable("mail-columns")
                .child(
                    resizable_panel()
                        .size(px(420.))
                        .size_range(px(280.)..px(720.))
                        .child(match &self.email_list {
                            Some(list) => list.clone().into_any_element(),
                            None => placeholder_pane(&i18n::t("email.loading"), cx).into_any_element(),
                        }),
                )
                .child(
                    resizable_panel().child(
                        match self.detail_view.clone() {
                            Some(view) => view.into_any_element(),
                            None => {
                                // 首帧兜底：建一次并缓存（后续由 SelectionChanged 重建）
                                let view = cx.new(|cx| EmailDetailView {
                                    store: self.detail.clone(),
                                    api: self.api.clone(),
                                    emails: self.emails.downgrade(),
                                    recipients_expanded: false,
                                    html_selection: Default::default(),
                                    header_selection: Default::default(),
                                    body_focus: cx.focus_handle(),
                                });
                                self.detail_view = Some(view.clone());
                                view.into_any_element()
                            }
                        },
                    ),
                )
                .into_any_element(),
        };

        // 根布局：自定义标题栏（跟随主题深浅）+ 侧栏/内容列（拖把手可调宽）
        let root = v_flex()
            .size_full()
            .child(crate::views::title_bar::render(self, window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        h_resizable("root-columns")
            .child(
                resizable_panel()
                    .size(px(220.))
                    .size_range(px(170.)..px(420.))
                    .fixed()
                    .child(self.sidebar.clone()),
            )
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
                    .min_h_0()
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
                    })
                    .into_any_element(),
            ),
                    ),
            );

        // 关闭选择弹窗（线上反馈⑤）：全屏遮罩 + 居中卡片（Z 序在内容之上）
        div()
            .relative()
            .size_full()
            .child(root)
            .when(self.close_dialog, |d| {
                let theme = cx.theme().clone();
                let remember = self.close_dialog_remember;
                d.child(
                    div()
                        .id("close-dialog-mask")
                        .absolute()
                        .size_full()
                        .top_0()
                        .left_0()
                        .bg(gpui_kit::black().opacity(0.45))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            v_flex()
                                .id("close-dialog")
                                .w(px(380.))
                                .p_5()
                                .gap_3()
                                .rounded(px(12.))
                                .bg(theme.background)
                                .border_1()
                                .border_color(theme.border)
                                .shadow_lg()
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme.foreground)
                                        .child(i18n::t("app.closeDialogTitle")),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(theme.muted_foreground)
                                        .child(i18n::t("app.closeDialogText")),
                                )
                                // 记住我的选择（勾选框）
                                .child(
                                    h_flex()
                                        .id("close-dialog-remember")
                                        .gap_2()
                                        .items_center()
                                        .cursor_pointer()
                                        .on_click(cx.listener(Self::dialog_toggle_remember))
                                        .child(
                                            div()
                                                .w(px(14.))
                                                .h(px(14.))
                                                .rounded(px(3.))
                                                .border_1()
                                                .border_color(theme.border)
                                                .when(remember, |d| {
                                                    d.bg(theme.accent)
                                                        .border_color(theme.accent)
                                                })
                                                .when(remember, |d| {
                                                    d.child(
                                                        gpui_kit::component::Icon::new(
                                                            IconName::Check,
                                                        )
                                                        .size_3()
                                                        .text_color(theme.accent_foreground),
                                                    )
                                                }),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.foreground)
                                                .child(i18n::t("app.closeRemember")),
                                        ),
                                )
                                // 动作行：取消 / 最小化到托盘 / 退出程序
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .justify_end()
                                        .child(
                                            Button::new("close-dialog-cancel")
                                                .ghost()
                                                .small()
                                                .label(i18n::t("app.closeCancel"))
                                                .on_click(cx.listener(Self::dialog_close_cancel)),
                                        )
                                        .child(
                                            Button::new("close-dialog-tray")
                                                .primary()
                                                .small()
                                                .label(i18n::t("app.closeToTray"))
                                                .on_click(cx.listener(Self::dialog_close_to_tray)),
                                        )
                                        .child(
                                            Button::new("close-dialog-quit")
                                                .ghost()
                                                .small()
                                                .label(i18n::t("app.closeQuit"))
                                                .on_click(cx.listener(Self::dialog_close_quit)),
                                        ),
                                ),
                        ),
                )
            })
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
