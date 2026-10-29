//! GPUI 版 E2E 功能测试（迁移自 e2e/test/specs/*.e2e.js）。
//!
//! 模式：真实全栈（CoreApi + SQLite + e2e seed 固定数据 + 全部视图/状态），
//! headless 窗口；用户操作以 on_click 同款 handler 驱动 +
//! simulate_keystrokes 输入框键入（TestAppContext 无元素定位 API），
//! 异步 settle 用 run_until_parked + 轮询。

use std::sync::Arc;

use chrono::Datelike as _;
use gpui_kit::AppContext as _;
use gpui_pre::{Focusable as _, TestAppContext, VisualTestContext};
use postium_core::api::CoreApi;

use crate::app::AppShell;
use crate::nav::NavTarget;

fn boot(dir: &std::path::Path) -> (Arc<CoreApi>, postium_core::api::SyncEventRx) {
    crate::runtime::runtime()
        .block_on(async { CoreApi::init(dir, true, false).await })
        .expect("核心服务初始化失败")
}

/// 进程级一次初始化的 E2E 数据目录（POSTIUM_E2E=1 + POSTIUM_DATA_DIR）。
///
/// ui-state 等持久化路径经 `resolve_data_dir` 读进程全局环境变量——并行测试
/// 各自 set_var 会在毫秒窗口内互相翻转（写与读落在不同 tempdir，断言偶发击穿，
/// 线上反馈①的放行记忆断言实测踩中）。改为首个调用者一次设定、全进程共享
/// 同一目录：各测试的 DB 仍用独立 tempdir，ui-state.json 字段互不相干
/// （theme/lang 与 remote_image_senders 不重叠，读改写交错最坏丢一次对方
/// 字段写入，不破坏各自断言）。目录句柄泄漏持有——进程存活期间不得回收。
static E2E_DATA_DIR: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn force_e2e_data_dir() {
    E2E_DATA_DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("E2E 数据目录创建失败");
        let path = dir.path().to_path_buf();
        std::mem::forget(dir);
        unsafe {
            std::env::set_var("POSTIUM_E2E", "1");
            std::env::set_var("POSTIUM_DATA_DIR", path.to_str().unwrap());
        }
    });
}

/// 共享 ui-state.json 的测试间串行化（theme 持久化 × 放行记忆）：读改写
/// 整份文件的两组写者并行交错会丢对方字段（OnceLock 只稳定目录，不序列化
/// 文件写）。锁只在这两个测试体内持有，测试结束随 guard 释放。
static UI_STATE_FILE_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn with_app<R>(cx: &mut VisualTestContext, f: impl FnOnce(&gpui_pre::App) -> R) -> R {
    cx.update(|_, app| f(app))
}

/// 在 &mut App 上执行（实体更新需要可变上下文）。
fn with_app_mut<R>(cx: &mut VisualTestContext, f: impl FnOnce(&mut gpui_pre::App) -> R) -> R {
    cx.update(|_, app| f(app))
}

/// 同时拿 window 与 &mut App（焦点设置等需要 window）。
fn with_window_mut<R>(
    cx: &mut VisualTestContext,
    f: impl FnOnce(&mut gpui_pre::Window, &mut gpui_pre::App) -> R,
) -> R {
    cx.update(|window, app| f(window, app))
}

fn wait_for<T>(
    cx: &mut VisualTestContext,
    mut f: impl FnMut(&mut VisualTestContext) -> Option<T>,
) -> T {
    for _ in 0..200 {
        cx.run_until_parked();
        if let Some(v) = f(cx) {
            return v;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    panic!("等待条件超时");
}

fn setup(cx: &mut gpui_pre::App) {
    gpui_kit::init(cx);
    postium_editor::init(cx);
    cx.set_global(crate::i18n::I18nGlobal {
        lang: crate::i18n::Lang::Zh,
    });
}

// ---------------------------------------------------------------------------
// smoke.e2e.js → 启动应用并显示主账号 inbox seed 数据
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn boots_with_inbox_seed(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });

    let count = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let n = shell.read(app).accounts.read(app).groups.len();
            (n >= 2).then_some(n)
        })
    });
    assert_eq!(count, 2, "seed 应含两个账号");

    let ok = with_app(cx, |app| {
        let groups = &shell.read(app).accounts.read(app).groups;
        groups.iter().any(|g| g.account.email.contains("primary"))
            && groups.iter().any(|g| g.account.email.contains("secondary"))
    });
    assert!(ok, "seed 账号应含 primary 与 secondary");

    let emails = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).emails.read(app);
            (!store.emails.is_empty()).then(|| store.emails.clone())
        })
    });
    assert!(
        emails.iter().any(|e| e
            .subject
            .as_deref()
            .unwrap_or("")
            .contains("Primary Inbox Message")),
        "inbox 应包含 seed 邮件，首封: {:?}",
        emails.first().and_then(|e| e.subject.clone())
    );
}

// ---------------------------------------------------------------------------
// account-switching.e2e.js → 主/次账号切换并刷新列表
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn switches_between_accounts(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });

    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let n = shell.read(app).accounts.read(app).groups.len();
            (n >= 2).then_some(n)
        })
    });

    let second_id = with_app(cx, |app| {
        shell
            .read(app)
            .accounts
            .read(app)
            .groups
            .iter()
            .find(|g| g.account.email.contains("secondary"))
            .map(|g| g.account.id)
            .expect("次账号")
    });

    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::AccountFolder {
                account_id: second_id,
                folder: "INBOX".into(),
            };
            shell.emails.update(cx, |store, cx| {
                store.navigate(shell.nav.clone(), &api, cx);
            });
        });
    });

    let emails = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).emails.read(app);
            store
                .emails
                .iter()
                .any(|e| e.subject.as_deref().unwrap_or("").contains("Secondary"))
                .then(|| store.emails.clone())
        })
    });
    assert!(
        emails.iter().all(|e| e.account_id == second_id),
        "次账号列表不应混入其他账号邮件"
    );

    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::AllInbox;
            shell.emails.update(cx, |store, cx| {
                store.navigate(NavTarget::AllInbox, &api, cx);
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .iter()
                .any(|e| {
                    e.subject
                        .as_deref()
                        .unwrap_or("")
                        .contains("Primary Inbox Message")
                })
                .then_some(())
        })
    });
}

// ---------------------------------------------------------------------------
// navigation.e2e.js → Sent / Starred / Inbox
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn navigates_sent_starred_inbox(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let n = shell.read(app).accounts.read(app).groups.len();
            (n >= 1).then_some(n)
        })
    });

    fn navigate(shell: &gpui_kit::Entity<AppShell>, cx: &mut VisualTestContext, target: NavTarget) {
        with_app_mut(cx, |app| {
            let api = shell.read(app).api.clone();
            shell.update(app, |shell, cx| {
                shell.nav = target.clone();
                shell.emails.update(cx, |store, cx| {
                    store.navigate(target, &api, cx);
                });
            });
        });
    }

    navigate(
        &shell,
        cx,
        NavTarget::Category(postium_core::service::email_service::EmailCategory::Sent),
    );
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .iter()
                .any(|e| {
                    e.subject
                        .as_deref()
                        .unwrap_or("")
                        .contains("Sent Confirmation")
                })
                .then_some(())
        })
    });

    navigate(
        &shell,
        cx,
        NavTarget::Category(postium_core::service::email_service::EmailCategory::Starred),
    );
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .iter()
                .any(|e| {
                    e.subject
                        .as_deref()
                        .unwrap_or("")
                        .contains("Starred Reference")
                })
                .then_some(())
        })
    });

    navigate(&shell, cx, NavTarget::AllInbox);
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .iter()
                .any(|e| {
                    e.subject
                        .as_deref()
                        .unwrap_or("")
                        .contains("Primary Inbox Message")
                })
                .then_some(())
        })
    });
}

// ---------------------------------------------------------------------------
// email-list.e2e.js → 搜索 Quarterly Planning
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn searches_quarterly_planning(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).emails.read(app).emails.is_empty()).then_some(())
        })
    });

    let before = with_app_mut(cx, |app| {
        let before = shell.read(app).emails.read(app).list_version;
        let api = shell.read(app).api.clone();
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.search("Quarterly Planning", &api, cx);
            });
        });
        before
    });

    let results = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).emails.read(app);
            (store.list_version > before && !store.emails.is_empty()).then(|| store.emails.clone())
        })
    });
    assert!(results.len() >= 2, "Quarterly 应至少命中 Alpha/Beta");
    assert!(
        results.iter().all(|e| e
            .subject
            .as_deref()
            .unwrap_or("")
            .contains("Quarterly Planning")),
        "搜索结果应全部匹配"
    );
}

// ---------------------------------------------------------------------------
// email-list.e2e.js → 详情页切换星标并删除当前邮件
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn stars_and_deletes_selected(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });

    let target_id = wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .iter()
                .find(|e| {
                    !e.is_starred && e.subject.as_deref().unwrap_or("").contains("Primary Inbox")
                })
                .map(|e| e.id)
        })
    });

    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(target_id);
                store.selected_version += 1;
                cx.notify();
            });
        });
    });

    // 星标切换（数据库验证）
    let rx = crate::runtime::spawn_tokio({
        let api = with_app(cx, |app| shell.read(app).api.clone());
        async move { api.toggle_star(target_id).await }
    });
    cx.run_until_parked();
    assert!(rx.recv_blocking().unwrap().unwrap(), "星标切换应置位");

    // 删除 → 刷新后列表不含该邮件
    let rx = crate::runtime::spawn_tokio({
        let api = with_app(cx, |app| shell.read(app).api.clone());
        async move { api.delete_emails(vec![target_id]).await }
    });
    cx.run_until_parked();
    assert_eq!(rx.recv_blocking().unwrap().unwrap(), 1);

    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.refresh_now(&api, cx);
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).emails.read(app);
            (!store.emails.iter().any(|e| e.id == target_id)).then_some(())
        })
    });
}

// ---------------------------------------------------------------------------
// compose.e2e.js → 写信字段填写 / 草稿 / 重开为空
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn compose_fills_fields_and_draft(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());
    let _ = sync_rx;

    let (compose, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::compose::ComposeView::new(
            crate::views::compose::ComposeMode::New,
            api,
            window,
            cx,
        )
    });

    // 发件账号默认选中
    wait_for(cx, |cx| {
        with_app(cx, |app| compose.read(app).selected_account.clone())
    });

    // 键入收件人（焦点 + 键盘事件）
    with_window_mut(cx, |window, app| {
        let handle = compose.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&handle, app);
    });
    cx.simulate_keystrokes("peer@example.com");
    let to_value = with_app(cx, |app| {
        compose
            .read(app)
            .to_field
            .input
            .read(app)
            .value()
            .to_string()
    });
    assert_eq!(to_value, "peer@example.com", "收件人键入应生效");

    // 空主题 → 发送被拦截
    with_window_mut(cx, |window, app| {
        compose.update(app, |view, cx| {
            view.send(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    let status = with_app(cx, |app| compose.read(app).status.to_string());
    assert!(status.contains("主题"), "空主题应被拦截，实际: {status}");

    // 填主题 → 存草稿
    with_window_mut(cx, |window, app| {
        let handle = compose.read(app).subject.read(app).focus_handle(app);
        window.focus(&handle, app);
    });
    cx.simulate_keystrokes("E2E Draft Subject");
    with_window_mut(cx, |_window, app| {
        compose.update(app, |view, cx| {
            view.persist_draft(cx);
        });
    });
    let draft_id = wait_for(cx, |cx| {
        with_app(cx, |app| compose.read(app).draft_email_id)
    });
    assert!(draft_id > 0, "草稿应保存并返回 id");
}

#[gpui_pre::test]
fn compose_reopens_empty(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());
    let _ = sync_rx;

    let (first, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::compose::ComposeView::new(
            crate::views::compose::ComposeMode::New,
            api.clone(),
            window,
            cx,
        )
    });
    with_window_mut(cx, |window, app| {
        let handle = first.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&handle, app);
    });
    cx.simulate_keystrokes("someone@x.test");

    // 重新打开：新实例字段为空（对齐旧用例）
    let api2 = with_app(cx, |app| first.read(app).api.clone());
    let (second, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::compose::ComposeView::new(
            crate::views::compose::ComposeMode::New,
            api2,
            window,
            cx,
        )
    });
    let (to, subject) = with_app(cx, |app| {
        (
            second
                .read(app)
                .to_field
                .input
                .read(app)
                .value()
                .to_string(),
            second.read(app).subject.read(app).value().to_string(),
        )
    });
    assert!(to.is_empty(), "重开收件人应为空，实际: {to}");
    assert!(subject.is_empty(), "重开主题应为空，实际: {subject}");
}

// ---------------------------------------------------------------------------
// theme.e2e.js → 深浅主题切换 + 持久化
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn toggles_theme_and_persists(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    // ui-state 持久化路径走 resolve_data_dir：强制进程级共享 E2E 目录
    //（此前各测试自行 set_var，并行下毫秒窗口内互相翻转——见 force_e2e_data_dir）。
    // 与放行记忆测试共享同一 ui-state.json，串行化防读改写交错丢字段
    force_e2e_data_dir();
    let _guard = UI_STATE_FILE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let ui_state_path =
        std::path::PathBuf::from(std::env::var("POSTIUM_DATA_DIR").unwrap()).join("ui-state.json");

    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });

    use gpui_kit::component::ActiveTheme as _;
    let mode = |cx: &mut VisualTestContext| -> bool {
        with_app(cx, |app| {
            app.theme().mode == gpui_kit::component::ThemeMode::Dark
        })
    };

    let initial_dark = mode(cx);
    with_window_mut(cx, |window, app| {
        shell.update(app, |shell, cx| {
            shell.toggle_theme(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    assert_ne!(mode(cx), initial_dark, "第一次切换应翻转主题");

    let saved = std::fs::read_to_string(&ui_state_path).unwrap_or_default();
    assert!(saved.contains("theme"), "主题应写入 ui-state.json: {saved}");

    with_window_mut(cx, |window, app| {
        shell.update(app, |shell, cx| {
            shell.toggle_theme(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    assert_eq!(mode(cx), initial_dark, "第二次切换应回到原主题");
    // 环境变量不回收：OnceLock 语义下进程内首个设定全程稳定，其他测试
    //（放行记忆等 ui-state 写读）依赖它定位共享目录
}

// ---------------------------------------------------------------------------
// calendar.e2e → 今日锚定夹具：当日条目、选日/取消、翻月、今天
// ---------------------------------------------------------------------------
/// 导航到日历视图并等待视图创建 + 今日夹具装载完成，返回日历实体。
fn open_calendar(
    shell: &gpui_kit::Entity<AppShell>,
    cx: &mut VisualTestContext,
) -> gpui_kit::Entity<crate::views::calendar::CalendarView> {
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Calendar;
            cx.notify();
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            shell.read(app).calendar_view.clone().filter(|view| {
                let v = view.read(app);
                v.loaded
                    && v.emails.iter().any(|e| {
                        e.subject
                            .as_deref()
                            .unwrap_or("")
                            .contains("Calendar Today Alpha")
                    })
            })
        })
    })
}

#[gpui_pre::test]
fn calendar_shows_today_and_pick_day(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    let calendar = open_calendar(&shell, cx);
    let today = chrono::Local::now().date_naive();

    // 今天应有 2 封夹具（Alpha 未读 + Beta 星标已读）
    let subjects = with_app(cx, |app| {
        calendar
            .read(app)
            .emails_of(today.day())
            .iter()
            .map(|e| e.subject.clone().unwrap_or_default())
            .collect::<Vec<_>>()
    });
    assert!(
        subjects.iter().any(|s| s.contains("Calendar Today Alpha"))
            && subjects
                .iter()
                .any(|s| s.contains("Calendar Today Beta Starred")),
        "今天应含 2 封日历夹具，实际: {subjects:?}"
    );
    let unread = with_app(cx, |app| {
        calendar
            .read(app)
            .emails_of(today.day())
            .iter()
            .filter(|e| !e.is_read)
            .count()
    });
    assert_eq!(unread, 1, "今天夹具应恰有 1 封未读（Alpha）");

    // 选中今天 → 取消选择（toggle）
    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.pick_day(today.day(), cx));
    });
    assert_eq!(
        with_app(cx, |app| calendar.read(app).selected_day),
        Some(today.day()),
        "pick_day 应选中今天"
    );
    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.pick_day(today.day(), cx));
    });
    assert_eq!(
        with_app(cx, |app| calendar.read(app).selected_day),
        None,
        "再次点击已选日期应取消选择"
    );
}

#[gpui_pre::test]
fn calendar_navigates_months_and_today(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    let calendar = open_calendar(&shell, cx);
    let today = chrono::Local::now().date_naive();
    let next_ym = |y: i32, m: u32| if m == 12 { (y + 1, 1u32) } else { (y, m + 1) };
    let prev_ym = |y: i32, m: u32| if m == 1 { (y - 1, 12u32) } else { (y, m - 1) };

    let (y0, m0) = with_app(cx, |app| {
        let v = calendar.read(app);
        (v.year, v.month)
    });
    assert_eq!((y0, m0), (today.year(), today.month()), "初始应停在当月");

    // 先选一天，翻月后应清空选中
    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.pick_day(15, cx));
    });

    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.shift_month(1, cx));
    });
    assert_eq!(
        with_app(cx, |app| {
            let v = calendar.read(app);
            (v.year, v.month)
        }),
        next_ym(y0, m0),
        "下月切换应正确跨年"
    );
    assert_eq!(
        with_app(cx, |app| calendar.read(app).selected_day),
        None,
        "翻月应清空选中日"
    );

    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.shift_month(-1, cx));
    });
    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.shift_month(-1, cx));
    });
    assert_eq!(
        with_app(cx, |app| {
            let v = calendar.read(app);
            (v.year, v.month)
        }),
        prev_ym(y0, m0),
        "连翻两个月应回到上月"
    );

    with_app_mut(cx, |app| {
        calendar.update(app, |view, cx| view.jump_to_today(cx));
    });
    assert_eq!(
        with_app(cx, |app| {
            let v = calendar.read(app);
            (v.year, v.month, v.selected_day)
        }),
        (today.year(), today.month(), Some(today.day())),
        "今天按钮应回到当月并选中今天"
    );
}

// ---------------------------------------------------------------------------
// add-account.e2e → 四步向导：服务商/手动 → 凭据 → 同步范围
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn adds_account_via_password_wizard(cx: &mut TestAppContext) {
    // 向导全程真实 I/O（keyring/DB 多轮往返），允许混合非确定性异步
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let api_for_view = api.clone();
    let (settings, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        let sync_store = cx.new(|cx| crate::stores::sync::SyncStore::new(cx));
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, sync_store.clone(), api_for_view.clone(), window, cx)
    });

    // 等初始账号列表装载完成（settle 后台任务，保证确定性）
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (settings.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // 打开向导 → 服务商列表装载
    with_window_mut(cx, |window, app| {
        settings.update(app, |view, cx| {
            view.open_add(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    let provider_count = wait_for(cx, |cx| {
        with_app(cx, |app| {
            settings
                .read(app)
                .add
                .as_ref()
                .filter(|a| !a.loading_providers && !a.providers.is_empty())
                .map(|a| a.providers.len())
        })
    });
    assert!(
        provider_count >= 2,
        "服务商列表应非空，实际 {provider_count}"
    );

    // 手动配置路径（对齐旧版"其他"卡片）
    with_app_mut(cx, |app| {
        settings.update(app, |view, cx| {
            view.select_manual(cx);
        });
    });

    // 填表单：邮箱 / 显示名 / 密码 / IMAP / SMTP
    let type_into = |cx: &mut VisualTestContext, which: usize, text: &str| {
        with_window_mut(cx, |window, app| {
            let handle = settings.read(app).add.as_ref().map(|a| match which {
                0 => a.email.clone(),
                1 => a.display_name.clone(),
                2 => a.password.clone(),
                3 => a.imap_host.clone(),
                4 => a.smtp_host.clone(),
                _ => a.email.clone(),
            });
            if let Some(h) = handle {
                let fh = h.read(app).focus_handle(app);
                window.focus(&fh, app);
            }
        });
        cx.simulate_keystrokes(text);
    };
    // 键入内容避免空格被解析为分隔：simulate_keystrokes 按空格分键，用连续字符串
    type_into(cx, 0, "wizard@local.test");
    type_into(cx, 1, "Wizard");
    type_into(cx, 2, "secretpassword");
    type_into(cx, 3, "imap.local.test");
    type_into(cx, 4, "smtp.local.test");

    // 提交 → 进入同步范围步骤
    with_window_mut(cx, |window, app| {
        settings.update(app, |view, cx| {
            view.submit_credentials(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    let pending = wait_for(cx, |cx| {
        with_app(cx, |app| {
            settings.read(app).add.as_ref().and_then(|a| a.pending_sync)
        })
    });
    assert!(pending > 0, "创建账号应返回正 id");
    let step = with_app(cx, |app| settings.read(app).add.as_ref().map(|a| a.step));
    assert_eq!(
        step,
        Some(crate::views::settings::AddStep::SyncScope),
        "提交后应进入同步范围步骤"
    );

    // 账号确实入库（按邮箱定位）
    let created = crate::runtime::runtime()
        .block_on(async { api.list_accounts().await.unwrap_or_default() })
        .into_iter()
        .find(|a| a.email == "wizard@local.test");
    assert!(created.is_some(), "账号应已写入数据库");

    // 选择范围并开始同步（begin_sync 不关窗；关窗路径由实机验证覆盖）
    with_app_mut(cx, |app| {
        settings.update(app, |view, cx| {
            view.set_sync_range(postium_core::domain::sync::InitialSyncRange::Week, cx);
        });
    });
    with_app_mut(cx, |app| {
        settings.update(app, |view, cx| {
            view.begin_sync(cx);
        });
    });
    // 等同步触发的账号列表 reload 收尾（含新账号），保证无未决通道
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            settings
                .read(app)
                .accounts
                .read(app)
                .groups
                .iter()
                .any(|g| g.account.email == "wizard@local.test")
                .then_some(())
        })
    });
    // 关闭向导（窗口仍在，可安全走 window 上下文）
    with_window_mut(cx, |window, app| {
        settings.update(app, |view, cx| {
            view.close_add(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    let wizard_open = with_app(cx, |app| settings.read(app).add.is_some());
    assert!(!wizard_open, "close_add 后向导应关闭");
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// gap-list.e2e → 差距清单补齐：右键菜单行为 / 主题跟随系统 / 搜索防抖 /
// 收件人 Chips / ESC 关闭向导
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn theme_follows_system_when_pref_is_system(cx: &mut TestAppContext) {
    // 纯映射断言（持久化往返由 toggles_theme_and_persists 覆盖；
    // 并行测试共享进程 env，这里不触碰 ui-state 路径）
    let _ = cx;
    assert_eq!(
        crate::theme::resolve_mode("system", gpui_kit::WindowAppearance::Light),
        gpui_kit::component::ThemeMode::Light
    );
    assert_eq!(
        crate::theme::resolve_mode("system", gpui_kit::WindowAppearance::Dark),
        gpui_kit::component::ThemeMode::Dark
    );
    assert_eq!(
        crate::theme::resolve_mode("light", gpui_kit::WindowAppearance::Dark),
        gpui_kit::component::ThemeMode::Light
    );
    assert_eq!(
        crate::theme::resolve_mode("dark", gpui_kit::WindowAppearance::Light),
        gpui_kit::component::ThemeMode::Dark
    );
    // 未知值回落暗色（与旧默认一致）
    assert_eq!(
        crate::theme::resolve_mode("bogus", gpui_kit::WindowAppearance::Light),
        gpui_kit::component::ThemeMode::Dark
    );
}

#[gpui_pre::test]
fn compose_recipient_field_chips_commit_and_remove(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let (compose, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::compose::ComposeView::new(
            crate::views::compose::ComposeMode::New,
            api.clone(),
            window,
            cx,
        )
    });

    // 输入两个地址（空格分隔）→ 回车提交 chips
    with_window_mut(cx, |window, app| {
        let h = compose.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&h, app);
    });
    cx.simulate_input("a@postium.test, b@postium.test");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let emails = with_app(cx, |app| compose.read(app).to_field.emails(app));
    assert_eq!(
        emails,
        vec!["a@postium.test".to_string(), "b@postium.test".to_string()],
        "回车应把输入解析为 chips: {emails:?}"
    );
    // chips 化后输入框清空
    let remaining = with_app(cx, |app| {
        compose
            .read(app)
            .to_field
            .input
            .read(app)
            .value()
            .to_string()
    });
    assert!(remaining.is_empty(), "提交后输入框应清空: {remaining}");

    // 重复地址合并（再次输入同一地址回车）
    with_window_mut(cx, |window, app| {
        let h = compose.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&h, app);
    });
    cx.simulate_keystrokes("a@postium.test");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let emails = with_app(cx, |app| compose.read(app).to_field.emails(app));
    assert_eq!(emails.len(), 2, "重复地址应合并去重: {emails:?}");

    // chip 删除
    with_app_mut(cx, |app| {
        compose.update(app, |view, cx| {
            view.to_field.remove_by_email("a@postium.test");
            cx.notify();
        });
    });
    let emails = with_app(cx, |app| compose.read(app).to_field.emails(app));
    assert_eq!(
        emails,
        vec!["b@postium.test".to_string()],
        "删除 chip 应生效"
    );
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e.js（补全子集）→ 写信收件人自动补全：输入触发候选、Enter 取第一项
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn compose_autocomplete_from_contacts(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let (compose, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::compose::ComposeView::new(
            crate::views::compose::ComposeMode::New,
            api.clone(),
            window,
            cx,
        )
    });

    // to 输入 "postium"（命中 seed.e2e@postium.test 与 sender.*@postium.test）
    with_window_mut(cx, |window, app| {
        let h = compose.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&h, app);
    });
    cx.simulate_input("postium");

    // 防抖 300ms 走模拟时钟：advance_clock 推进后再由 run_until_parked 驱动
    // 防抖定时 → tokio 查询 → 候选回填（真实 I/O 跨线程唤醒）
    cx.dispatcher
        .advance_clock(std::time::Duration::from_millis(400));

    // 300ms 防抖 + 异步查询 → 候选非空且含 seed 联系人（星标置顶 → 第一项）
    let sugg = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let list = compose.read(app).to_field.suggestions.clone();
            (!list.is_empty()).then_some(list)
        })
    });
    let first = sugg.first().map(|c| c.email.clone()).unwrap_or_default();
    assert_eq!(
        first,
        "seed.e2e@postium.test",
        "星标 seed 联系人应排第一，实际: {:?}",
        sugg.iter().map(|c| c.email.clone()).collect::<Vec<_>>()
    );
    assert!(
        with_app(cx, |app| compose.read(app).to_field.sugg_open),
        "候选非空时浮层应开启"
    );

    // Enter 取第一项 → chips 化（输入框清空）
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let emails = with_app(cx, |app| compose.read(app).to_field.emails(app));
    assert_eq!(
        emails,
        vec!["seed.e2e@postium.test".to_string()],
        "Enter 应提交第一项候选为 chip: {emails:?}"
    );
    let open = with_app(cx, |app| compose.read(app).to_field.sugg_open);
    assert!(!open, "提交后浮层应关闭");
}

#[gpui_pre::test]
fn add_account_modal_closes_on_escape(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let api_for_view = api.clone();
    let (settings, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        let sync_store = cx.new(|cx| crate::stores::sync::SyncStore::new(cx));
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, sync_store.clone(), api_for_view.clone(), window, cx)
    });

    // 打开向导并等服务商装载（焦点已落在弹窗）
    with_window_mut(cx, |window, app| {
        settings.update(app, |view, cx| {
            view.open_add(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            settings
                .read(app)
                .add
                .as_ref()
                .filter(|a| !a.loading_providers && !a.providers.is_empty())
                .map(|_| ())
        })
    });

    // ESC → 向导关闭
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let open = with_app(cx, |app| settings.read(app).add.is_some());
    assert!(!open, "ESC 应关闭添加账号向导");
    cx.run_until_parked();
}

#[gpui_pre::test]
fn email_row_context_menu_actions_apply(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    // 等账号装载 settle 后再等列表（对齐 boots 测试次序）
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).emails.read(app).emails.is_empty()).then_some(())
        })
    });
    let api2 = with_app(cx, |app| shell.read(app).api.clone());

    // 直接调右键菜单的行级动作路径（PopupMenu 弹出本身属渲染层，
    // 动作执行链路 act_on_email 为核心逻辑）
    let id = {
        let email = with_app(cx, |app| {
            shell
                .read(app)
                .emails
                .read(app)
                .emails
                .first()
                .unwrap()
                .clone()
        });
        email.id
    };

    // 标未读 → 已读翻转验证（种子首封已读 → 标未读 → DB 应为未读）
    with_app_mut(cx, |app| {
        crate::views::email_list::test_hooks::act_on_email_for_test(
            api2.clone(),
            id,
            crate::views::email_list::test_hooks::RowActionForTest::MarkRead(false),
            app,
        );
    });
    // 后台任务为真实异步：轮询直至落库（最多 ~3s）
    let mut unread = true;
    for _ in 0..30 {
        cx.run_until_parked();
        unread = crate::runtime::runtime()
            .block_on(async { api2.get_email(id).await.unwrap().email.is_read });
        if !unread {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(!unread, "右键菜单标未读应落库");
}

// ---------------------------------------------------------------------------
// contacts.e2e → 种子联系人装载 + 启动回填共存 + 星标过滤
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn contacts_seed_loaded(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());
    let api_check = api.clone();

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    let _ = shell; // AppShell::new 的构造副作用（启动回填）已被下方断言覆盖

    // 种子联系人在 CoreApi::init 已落库（source='local'，回填不覆盖频次）
    let primary = wait_for(cx, |_| {
        crate::runtime::runtime()
            .block_on(async { api_check.list_contacts(None, None, false, 0, 50).await.ok() })
            .and_then(|r| {
                r.contacts
                    .into_iter()
                    .find(|c| c.email == "seed.e2e@postium.test")
            })
    });
    assert_eq!(primary.name.as_deref(), Some("Primary E2E"));
    assert_eq!(primary.frequency, 5);
    assert!(primary.is_starred);
    assert_eq!(primary.source, "local");

    // 星标过滤：种子中仅 Primary E2E 被星标
    let starred = wait_for(cx, |_| {
        let r = crate::runtime::runtime()
            .block_on(async { api_check.list_contacts(None, None, true, 0, 50).await })
            .expect("星标过滤查询失败");
        (r.total == 1).then_some(r)
    });
    assert_eq!(
        starred.contacts[0].email, "seed.e2e@postium.test",
        "星标过滤应仅含 Primary E2E"
    );

    // AppShell::new 的 fire-and-forget 回填已跑完：夹具邮件的收件人以
    // collected 条目出现（仅能来自启动回填，种子不产生该地址）
    wait_for(cx, |_| {
        crate::runtime::runtime()
            .block_on(async {
                api_check
                    .get_contact_by_email("e2e.user@postium.test".into())
                    .await
                    .ok()
            })
            .flatten()
            .filter(|c| c.source == "collected")
    });

    // 种子的 local 发件人与回填共存：频次保持种子值（10），仅推进往来时间
    let sender = wait_for(cx, |_| {
        crate::runtime::runtime()
            .block_on(async {
                api_check
                    .get_contact_by_email("sender.primary@postium.test".into())
                    .await
                    .ok()
            })
            .flatten()
    });
    assert_eq!(sender.source, "local");
    assert_eq!(sender.frequency, 10, "回填不得覆盖 local 条目频次");
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e → 导航入口：nav=Contacts 切换主区（占位渲染不 panic）
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn navigate_to_contacts_shows_title(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // 侧栏点击同款路径：nav=Contacts → 主区渲染占位（run_until_parked 隐式覆盖不 panic）
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Contacts;
            cx.notify();
        });
    });
    cx.run_until_parked();
    assert_eq!(
        with_app(cx, |app| shell.read(app).nav.clone()),
        NavTarget::Contacts,
        "nav 应切换到联系人"
    );
}

// ---------------------------------------------------------------------------
// contacts.e2e → ContactsView 三栏交互：导航懒建/装载/星标反转/刷新/搜索过滤
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn contacts_view_interactions(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // 导航 Contacts → 视图懒建 + store 首次装载
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Contacts;
            cx.notify();
        });
    });
    let view = wait_for(cx, |cx| {
        with_app(cx, |app| shell.read(app).contacts_view.clone())
    });
    let contacts = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.list_version > 0 && !store.loading).then(|| store.contacts.clone())
        })
    });

    // 种子联系人应已在 store（含 Primary E2E，初始为星标）
    let primary = contacts
        .iter()
        .find(|c| c.email == "seed.e2e@postium.test")
        .expect("store 联系人应含种子 Primary E2E");
    assert_eq!(primary.name.as_deref(), Some("Primary E2E"));
    assert!(primary.is_starred, "种子 Primary E2E 初始应为星标");
    let primary_id = primary.id;

    // 直驱星标路径（列表行 ★ 点击同款 store 方法）：乐观反转
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store
                .update(cx, |store, cx| store.toggle_starred(primary_id, &api, cx));
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            let flipped = store
                .contacts
                .iter()
                .find(|c| c.id == primary_id)
                .filter(|c| !c.is_starred)
                .is_some();
            flipped.then_some(())
        })
    });

    // 落库确认（异步写入：轮询直至持久化）
    wait_for(cx, |_| {
        let c = crate::runtime::runtime()
            .block_on(async { api.get_contact(primary_id).await })
            .expect("联系人查询失败")
            .expect("联系人应存在");
        (!c.is_starred).then_some(())
    });

    // store 刷新：装载完成后 DB 状态保持（仍为非星标）
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store.update(cx, |store, cx| store.load(&api, cx));
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.list_version >= 2 && !store.loading)
                .then(|| store.contacts.iter().find(|c| c.id == primary_id).cloned())
                .flatten()
                .filter(|c| !c.is_starred)
                .is_some()
                .then_some(())
        })
    });

    // 搜索过滤（工具条搜索输入防抖直达的 store 方法）：唯一词只命中种子联系人
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store
                .update(cx, |store, cx| store.set_query("seed.e2e", &api, cx));
        });
    });
    let filtered = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.query == "seed.e2e" && store.contacts.len() == 1 && !store.loading)
                .then(|| store.contacts.clone())
        })
    });
    assert_eq!(
        filtered[0].email, "seed.e2e@postium.test",
        "搜索 seed.e2e 应只命中种子联系人"
    );
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e → 星标写操作与在途 load 的竞态：陈旧快照不得覆盖乐观态，
// 写完成后补跑 load 最终一致（write_gen 代数防护回归）
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn contacts_star_write_race_converges(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // 导航 Contacts → 视图懒建 + store 首次装载
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Contacts;
            cx.notify();
        });
    });
    let view = wait_for(cx, |cx| {
        with_app(cx, |app| shell.read(app).contacts_view.clone())
    });
    let contacts = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.list_version > 0 && !store.loading).then(|| store.contacts.clone())
        })
    });

    // 目标：种子中未星标的联系人（写操作会翻转其状态）
    let target = contacts
        .iter()
        .find(|c| c.email == "sender.secondary@postium.test")
        .expect("store 应含种子 sender.secondary");
    let id = target.id;
    let expected = !target.is_starred;

    // 同一 UI 帧内：先发起 load（改查询触发在途装载），紧接着乐观星标写入。
    // 完成回调只能在 UI 线程跑（必然晚于本帧），装载快照在写之后才回到
    // UI 线程 —— 陈旧快照覆盖乐观态的竞态场景。
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store.update(cx, |store, cx| {
                store.set_query("postium.test", &api, cx); // 触发在途 load
                store.toggle_starred(id, &api, cx); // 写路径：乐观翻转 + write_gen++
            });
        });
    });

    // 最终一致：装载全部收尾后，本地列表中该联系人保持翻转后的星标态
    // （陈旧快照被 write_gen 拦截，写完成后的补跑 load 收敛到 DB 真值）
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            let settled = !store.loading
                && store
                    .contacts
                    .iter()
                    .any(|c| c.id == id && c.is_starred == expected);
            settled.then_some(())
        })
    });

    // DB 同步收敛为同一真值
    wait_for(cx, |_| {
        let c = crate::runtime::runtime()
            .block_on(async { api.get_contact(id).await })
            .expect("联系人查询失败")
            .expect("联系人应存在");
        (c.is_starred == expected).then_some(())
    });
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e → Task 11: 合并交互 + 邮件详情联动
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn contact_merge_and_detail_add(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    // 注册主壳全局弱引用（生产由 open_main_window 注册；e2e 直接建壳需手动补）
    with_app_mut(cx, |app| {
        app.set_global(crate::app::MainShell(shell.downgrade()));
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // ── Part A：合并（行首复选多选 → 工具条「合并」，primary = 最后选中）──
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Contacts;
            cx.notify();
        });
    });
    let view = wait_for(cx, |cx| {
        with_app(cx, |app| shell.read(app).contacts_view.clone())
    });
    let contacts = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.list_version > 0 && !store.loading).then(|| store.contacts.clone())
        })
    });
    let primary_id = contacts
        .iter()
        .find(|c| c.email == "seed.e2e@postium.test")
        .expect("store 应含种子 seed.e2e")
        .id;
    let absorbed_id = contacts
        .iter()
        .find(|c| c.email == "sender.primary@postium.test")
        .expect("store 应含种子 sender.primary")
        .id;

    // 复选两枚（先点 absorbed 后点 primary）→ 直驱合并：sender.primary 并入 seed.e2e
    with_app_mut(cx, |app| {
        view.update(app, |v, cx| {
            v.toggle_merge_pick(absorbed_id, cx);
            v.toggle_merge_pick(primary_id, cx);
            v.merge_selected(cx);
        });
    });

    // 合并后补跑装载收敛：absorbed 从列表消失，primary 保留
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            let gone = !store.contacts.iter().any(|c| c.id == absorbed_id);
            let kept = store.contacts.iter().any(|c| c.id == primary_id);
            (!store.loading && gone && kept).then_some(())
        })
    });
    // DB 层：absorbed 已删除（邮箱转入 primary 的 extra_emails）
    wait_for(cx, |_| {
        crate::runtime::runtime()
            .block_on(async { api.get_contact(absorbed_id).await })
            .ok()
            .flatten()
            .is_none()
            .then_some(())
    });

    // ── Part B：邮件详情「联系人」联动 ──
    // 切回邮件视图，选中发件人为 sender.primary 的种子邮件（该联系人已被合并
    // 删除，其邮箱不再是任何联系人的主邮箱 → 走「不存在则创建」分支）
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::AllInbox;
            cx.notify();
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).emails.read(app).emails.is_empty()).then_some(())
        })
    });
    let email_id = with_app(cx, |app| {
        shell
            .read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .find(|e| e.sender_email == "sender.primary@postium.test")
            .map(|e| e.id)
            .expect("收件箱应含 sender.primary 发出的种子邮件")
    });
    // 行选中同款事件：SelectionChanged → 详情装载 + 详情视图重建
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(email_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    email_id,
                ));
                cx.notify();
            });
        });
    });
    let detail_view = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == email_id).then_some(view)
        })
    });

    // 联动处理器（第一驱）：联系人不存在 → 创建分支
    with_window_mut(cx, |window, app| {
        detail_view.update(app, |view, cx| {
            view.add_contact(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    let created = wait_for(cx, |_| {
        crate::runtime::runtime()
            .block_on(async {
                api.get_contact_by_email("sender.primary@postium.test".into())
                    .await
            })
            .ok()
            .flatten()
    });

    // 联动处理器（第二驱）：联系人已存在 → 切 Contacts 并按邮箱选中
    with_window_mut(cx, |window, app| {
        detail_view.update(app, |view, cx| {
            view.add_contact(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).nav == NavTarget::Contacts).then_some(())
        })
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.selected == Some(created.id)).then_some(())
        })
    });
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e → Task 15: 导入导出 UI 入口
//（store 导入路径：vCard/CSV 文本 → api 导入 → 列表刷新；工具条按钮存在性）
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn contacts_import_export_entry(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (shell.read(app).accounts.read(app).groups.len() >= 1).then_some(())
        })
    });

    // 导航 Contacts → 视图懒建 + store 首次装载
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.nav = NavTarget::Contacts;
            cx.notify();
        });
    });
    let view = wait_for(cx, |cx| {
        with_app(cx, |app| shell.read(app).contacts_view.clone())
    });
    let before = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (store.list_version > 0 && !store.loading).then(|| store.contacts.len())
        })
    });

    // ── 工具条按钮存在于渲染树（debug_selector 在绘制帧记录 bounds）──
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("contacts-import-btn")
            .zip(cx.debug_bounds("contacts-export-btn"))
            .map(|_| ())
    });

    // ── 导入路径（按钮文件 IO 后的直达 store 方法）：vCard 两有效卡 + 一缺邮箱卡 ──
    const TWO_CARDS_PLUS_NO_EMAIL: &str = concat!(
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:Alice Import\r\n",
        "EMAIL;TYPE=INTERNET:alice.import@postium.test\r\n",
        "END:VCARD\r\n",
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:Bob Import\r\n",
        "EMAIL;TYPE=INTERNET:bob.import@postium.test\r\n",
        "END:VCARD\r\n",
        "BEGIN:VCARD\r\n",
        "VERSION:3.0\r\n",
        "FN:NoEmail\r\n",
        "TEL:13811111111\r\n",
        "END:VCARD\r\n",
    );
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store.update(cx, |store, cx| {
                store.import_contacts_vcard_text(TWO_CARDS_PLUS_NO_EMAIL.to_string(), &api, cx)
            });
        });
    });
    // store 刷新后列表多出两枚（缺邮箱卡计入跳过，不进列表）
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            let alice = store
                .contacts
                .iter()
                .any(|c| c.email == "alice.import@postium.test");
            let bob = store
                .contacts
                .iter()
                .any(|c| c.email == "bob.import@postium.test");
            (!store.loading && store.contacts.len() == before + 2 && alice && bob).then_some(())
        })
    });
    // DB 层：两卡已入库（toast 在无 Root 防护下不 panic，见 notify.rs）
    let db_emails = wait_for(cx, |_| {
        crate::runtime::runtime().block_on(async {
            let a = api
                .get_contact_by_email("alice.import@postium.test".into())
                .await
                .ok()
                .flatten();
            let b = api
                .get_contact_by_email("bob.import@postium.test".into())
                .await
                .ok()
                .flatten();
            (a.is_some() && b.is_some()).then(|| (a.unwrap().id, b.unwrap().id))
        })
    });
    assert_ne!(db_emails.0, db_emails.1);

    // ── CSV 导入路径（同款 store 方法）：通用方言单行 ──
    with_app_mut(cx, |app| {
        let api = shell.read(app).api.clone();
        view.update(app, |v, cx| {
            v.store.update(cx, |store, cx| {
                store.import_contacts_csv_text(
                    "name,email\r\nCarol Import,carol.import@postium.test\r\n".to_string(),
                    &api,
                    cx,
                )
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = view.read(app).store.read(app);
            (!store.loading
                && store
                    .contacts
                    .iter()
                    .any(|c| c.email == "carol.import@postium.test"))
            .then_some(())
        })
    });

    // ── 导出冒烟（按钮接线层直调 api：CoreApi 层等价性已在 T14 测过）──
    let exported = crate::runtime::runtime()
        .block_on(async { api.export_contacts_vcard(None, None, false).await })
        .expect("导出 vCard 不应失败");
    assert!(exported.contains("BEGIN:VCARD"), "导出应为 vCard 文本");
    assert!(
        exported.contains("alice.import@postium.test"),
        "导出应含刚导入的联系人"
    );
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// settings.e2e → 左侧三区导航（通用/外观/账户）切换
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn settings_sections_navigate(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let api_for_view = api.clone();
    let (settings, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        let sync_store = cx.new(|cx| crate::stores::sync::SyncStore::new(cx));
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, sync_store.clone(), api_for_view.clone(), window, cx)
    });

    // 默认落在"通用"分区（对齐旧版 activeSection 初始值）
    let sec = with_app(cx, |app| settings.read(app).active_section);
    assert_eq!(sec, crate::views::settings::SettingsSection::General);

    // 切换到外观 → 账户 → 通用
    for want in [
        crate::views::settings::SettingsSection::Appearance,
        crate::views::settings::SettingsSection::Accounts,
        crate::views::settings::SettingsSection::General,
    ] {
        with_app_mut(cx, |app| {
            settings.update(app, |view, cx| {
                view.active_section = want;
                cx.notify();
            });
        });
        assert_eq!(with_app(cx, |app| settings.read(app).active_section), want);
    }
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// contacts.e2e → CardDAV：e2e 离线短路（同步空报告/发现空列表）+ 配置弹窗开闭
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn dav_offline_short_circuit(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    // e2e 模式（非真连库）：同步入口短路 → Ok 空报告（无网络调用）
    let report = crate::runtime::runtime()
        .block_on(async { api.sync_contacts(999).await })
        .expect("e2e 模式 sync_contacts 应短路成功");
    assert_eq!(
        (
            report.pulled,
            report.pushed,
            report.deleted,
            report.conflicts
        ),
        (0, 0, 0, 0),
        "短路报告应为全零: {report:?}"
    );
    assert!(report.errors.is_empty());

    // discover 同样短路 → 空列表
    let books = crate::runtime::runtime()
        .block_on(async {
            api.discover_addressbooks("https://dav.example/".into(), "user".into(), "pass".into())
                .await
        })
        .expect("e2e 模式 discover_addressbooks 应短路成功");
    assert!(books.is_empty(), "短路发现应为空列表");
}

#[gpui_pre::test]
fn dav_config_modal_opens_and_closes_on_escape(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, _sync_rx) = boot(dir.path());

    let api_for_view = api.clone();
    let (view, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        crate::views::contacts::ContactsView::new(api_for_view.clone(), window, cx)
    });

    // 打开 CardDAV 配置弹窗（焦点落弹窗）
    with_window_mut(cx, |window, app| {
        view.update(app, |v, cx| {
            v.open_dav_modal(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    // 等绑定/账号装载收敛（loading=false）
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            view.read(app)
                .dav
                .as_ref()
                .filter(|d| !d.loading)
                .map(|_| ())
        })
    });

    // ESC → 弹窗关闭
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let open = with_app(cx, |app| view.read(app).dav.is_some());
    assert!(!open, "ESC 应关闭 CardDAV 配置弹窗");
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 9：GPUI HTML 渲染层冒烟——IR 文档嵌入测试窗口渲染不 panic、文本可查询
// （行内段以内容为键写入渲染树 debug_selector，与 contacts-import 的
// debug_bounds 查询同款 gpui_pre 文本查询模式）
// ---------------------------------------------------------------------------
struct HtmlRenderProbe {
    doc: postium_html::HtmlDocument,
    /// 链接点击回调（Task 10：e2e 注入捕获闭包断言 SafeUrl 通路）。
    on_link: Arc<dyn Fn(&postium_html::SafeUrl) + 'static>,
}

impl gpui_kit::Render for HtmlRenderProbe {
    fn render(
        &mut self,
        _: &mut gpui_kit::Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        // Arc<dyn Fn> 不直接满足 impl Fn 泛型界（无 Arc<dyn> 的 Fn 覆盖实现），
        // 经闭包解引用调用（call 位点自动 deref 到 &dyn Fn）。
        let on_link = self.on_link.clone();
        let element = crate::views::email_html::render_html_document(
            &self.doc,
            false,
            crate::views::email_html::ImageCtx::default(),
            move |url: &postium_html::SafeUrl| on_link(url),
            cx,
        );
        gpui_kit::div().size_full().overflow_hidden().child(element)
    }
}

#[gpui_pre::test]
fn html_render_smoke(cx: &mut TestAppContext) {
    use postium_html::{
        Align, Block, BlockStyle, HtmlDocument, ImageRef, ImageStyle, InlineRun, InlineStyle,
        SafeUrl,
    };

    fn text(t: &str) -> InlineRun {
        InlineRun::Text {
            text: t.into(),
            style: InlineStyle::default(),
        }
    }

    let doc = HtmlDocument {
        blocks: vec![
            // 容器 + 内嵌段落
            Block::Container {
                style: BlockStyle {
                    background_color: Some([240, 244, 255, 255]),
                    ..Default::default()
                },
                children: vec![Block::Paragraph {
                    style: BlockStyle::default(),
                    runs: vec![text("容器段落文本")],
                    align: Align::Left,
                }],
            },
            // 段落：行内文本 + 链接 + br 换行
            Block::Paragraph {
                style: BlockStyle {
                    color: Some([178, 34, 34, 255]),
                    ..Default::default()
                },
                runs: vec![
                    text("普通段落"),
                    InlineRun::Link {
                        href: SafeUrl::Http("https://example.com/".into()),
                        runs: vec![text("链接文字")],
                    },
                    InlineRun::Br,
                    text("换行后文本"),
                ],
                align: Align::Left,
            },
            // 标题
            Block::Heading {
                level: 2,
                align: Align::Center,
                runs: vec![text("二级标题文本")],
            },
            // 一级标题（UA 默认字号 32px，锁标题字号非正文兜底 14px）
            Block::Heading {
                level: 1,
                align: Align::Left,
                runs: vec![text("一级标题文本")],
            },
            // 无序列表
            Block::List {
                ordered: false,
                items: vec![
                    vec![Block::Paragraph {
                        style: BlockStyle::default(),
                        runs: vec![text("列表项甲")],
                        align: Align::Left,
                    }],
                    vec![Block::Paragraph {
                        style: BlockStyle::default(),
                        runs: vec![text("列表项乙")],
                        align: Align::Left,
                    }],
                ],
            },
            // 引用
            Block::Quote {
                children: vec![Block::Paragraph {
                    style: BlockStyle::default(),
                    runs: vec![text("引用内容文本")],
                    align: Align::Left,
                }],
            },
            // 图片占位（Task 13 接管真实渲染）
            Block::Image {
                src: ImageRef::Invalid,
                alt: "替代文字图片".into(),
                style: ImageStyle::default(),
            },
            Block::Hr,
        ],
    };

    let (probe, cx) = cx.add_window_view(|_window, cx| {
        setup(cx);
        HtmlRenderProbe {
            doc,
            on_link: Arc::new(|_: &postium_html::SafeUrl| {}),
        }
    });
    let _ = &probe;

    cx.refresh().expect("测试窗口重绘调度失败");
    // 渲染不 panic 已由重绘覆盖；文本内容可查询：行内段以内容为键出现。
    // 探针键粒度 = 字号分组文本元素（同组段合并为一个文本元素，
    // 键取组文本前缀）：第二段三 run 同字号 → 单键；链接 run 的下划线/
    // 链接色、br 折行均在该组文本内呈现。
    let keys = [
        "html-txt-容器段落文本",
        "html-txt-普通段落链接文字换行后文本",
        "html-txt-二级标题文本",
        "html-txt-一级标题文本",
        "html-txt-列表项甲",
        "html-txt-列表项乙",
        "html-txt-引用内容文本",
        "html-txt-替代文字图片",
        "html-hr",
        "html-root",
    ];
    let mut missing = keys.to_vec();
    for _ in 0..200 {
        cx.run_until_parked();
        missing.retain(|k| {
            cx.debug_bounds(k).is_none_or(|b| {
                b.size.width <= gpui_kit::px(0.) || b.size.height <= gpui_kit::px(0.)
            })
        });
        if missing.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    assert!(missing.is_empty(), "渲染树缺失文本探针: {missing:?}");

    // 标题默认字号（评审修复 1 回归锁）：h1 文本元素行高 = 32px × 1.6 =
    // 51.2px；若字号死样式回退为正文兜底 14px，行高仅 22.4px。
    let h1_height = cx
        .debug_bounds("html-txt-一级标题文本")
        .expect("h1 文本探针应存在")
        .size
        .height;
    assert!(
        h1_height > gpui_kit::px(40.),
        "h1 应按级别默认字号渲染（行高应 >40px），实际 {h1_height}"
    );
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 10：表格 flex 模拟 + 链接交互——两列表格 + colspan 链接行渲染不 panic、
// 单元格文本探针可见、点击链接 on_link 回调收到 SafeUrl::Http
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn html_render_table_and_link(cx: &mut TestAppContext) {
    use std::sync::Mutex as StdMutex;

    use postium_html::{
        Align, Block, BlockStyle, HtmlDocument, InlineRun, InlineStyle, SafeUrl, TableCell,
        TableRow,
    };

    fn text(t: &str) -> InlineRun {
        InlineRun::Text {
            text: t.into(),
            style: InlineStyle::default(),
        }
    }
    fn para(runs: Vec<InlineRun>) -> Block {
        Block::Paragraph {
            style: BlockStyle::default(),
            runs,
            align: Align::Left,
        }
    }

    let doc = HtmlDocument {
        blocks: vec![Block::Table {
            style: BlockStyle::default(),
            rows: vec![
                // 两列表格：上排两个等权单元格
                TableRow {
                    cells: vec![
                        TableCell {
                            blocks: vec![para(vec![text("单元格甲")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                        TableCell {
                            blocks: vec![para(vec![text("单元格乙")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                    ],
                },
                // colspan [1,2] 行：colspan 权重生效断言载体（1:2 分布），
                // 第二格含链接（交互断言载体）
                TableRow {
                    cells: vec![
                        TableCell {
                            blocks: vec![para(vec![text("跨列前段")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                        TableCell {
                            blocks: vec![para(vec![InlineRun::Link {
                                href: SafeUrl::Http("https://example.com/table".into()),
                                runs: vec![text("表格链接文字")],
                            }])],
                            colspan: 2,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                    ],
                },
                // [1,1,1] 三等分对照行
                TableRow {
                    cells: vec![
                        TableCell {
                            blocks: vec![para(vec![text("三等分甲")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                        TableCell {
                            blocks: vec![para(vec![text("三等分乙")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                        TableCell {
                            blocks: vec![para(vec![text("三等分丙")])],
                            colspan: 1,
                            rowspan: 1,
                            width: None,
                            style: BlockStyle::default(),
                        },
                    ],
                },
            ],
        }],
    };

    let clicks: Arc<StdMutex<Vec<SafeUrl>>> = Arc::new(StdMutex::new(Vec::new()));
    let sink = clicks.clone();
    let (probe, cx) = cx.add_window_view(|_window, cx| {
        setup(cx);
        HtmlRenderProbe {
            doc,
            on_link: Arc::new(move |url: &postium_html::SafeUrl| {
                sink.lock().unwrap().push(url.clone())
            }),
        }
    });
    let _ = &probe;

    cx.refresh().expect("测试窗口重绘调度失败");

    // 表格渲染不塌陷：单元格文本以探针键出现在渲染树
    let keys = [
        "html-txt-单元格甲",
        "html-txt-单元格乙",
        "html-txt-跨列前段",
        "html-txt-表格链接文字",
        "html-txt-三等分甲",
        "html-txt-三等分乙",
        "html-txt-三等分丙",
    ];
    let mut missing = keys.to_vec();
    for _ in 0..200 {
        cx.run_until_parked();
        missing.retain(|k| {
            cx.debug_bounds(k).is_none_or(|b| {
                b.size.width <= gpui_kit::px(0.) || b.size.height <= gpui_kit::px(0.)
            })
        });
        if missing.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    assert!(
        missing.is_empty(),
        "渲染树缺失表格单元格文本探针: {missing:?}"
    );

    // colspan 权重生效（评审加固）：[1,2] 行宽度比 1:2（±5%）。探针 div 为
    // 单元格内 w_full 文本元素，宽度即单元格宽。
    let mut cell_w = |key: &'static str| {
        cx.debug_bounds(key)
            .unwrap_or_else(|| panic!("探针 {key} 应存在"))
            .size
            .width
    };
    let ratio = cell_w("html-txt-跨列前段") / cell_w("html-txt-表格链接文字");
    assert!(
        (0.475..=0.525).contains(&ratio),
        "[1,2] 行应按 colspan 权重 1:2 分布（±5%），实测 {ratio:.3}——\
         若回退为 50/50 说明 flex_basis 归零修复失效"
    );
    // [1,1,1] 三等分对照：三格两两差 ≤5%
    let thirds = [
        cell_w("html-txt-三等分甲"),
        cell_w("html-txt-三等分乙"),
        cell_w("html-txt-三等分丙"),
    ];
    let (min_w, max_w) = thirds.iter().fold((thirds[0], thirds[0]), |(lo, hi), &w| {
        (lo.min(w), hi.max(w))
    });
    assert!(
        max_w / min_w <= 1.05,
        "[1,1,1] 行应三等分（±5%），实测 {thirds:?}"
    );
    drop(cell_w);

    // 点击链接 → on_link 回调捕获 SafeUrl::Http（探针 div 全宽，链接文本起于左缘）
    let bounds = cx
        .debug_bounds("html-txt-表格链接文字")
        .expect("链接文本探针应存在");
    cx.simulate_click(
        gpui_pre::point(
            bounds.origin.x + gpui_kit::px(4.),
            bounds.origin.y + bounds.size.height / 2.,
        ),
        gpui_pre::Modifiers::default(),
    );
    cx.run_until_parked();

    let got = clicks.lock().unwrap();
    assert_eq!(got.len(), 1, "点击链接应恰好触发一次 on_link: {got:?}");
    assert_eq!(
        got.first(),
        Some(&SafeUrl::Http("https://example.com/table".into())),
        "on_link 应收到 SafeUrl::Http"
    );
}

// ---------------------------------------------------------------------------
// Task 11：email_detail 正文接入新渲染器——种子 HTML 邮件打开后正文文本以
// 新渲染器探针键出现（旧 gpui_kit text::html 路径不产生 html-txt-* 探针，
// 探针键由 email_html 渲染层在测试构建写入）
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn html_email_renders_via_new_renderer(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).emails.read(app).emails.is_empty()).then_some(())
        })
    });

    // 种子 HTML 邮件（body_html = <p>This is deterministic E2E body content …</p>）
    let email_id = with_app(cx, |app| {
        shell
            .read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .find(|e| {
                e.subject
                    .as_deref()
                    .unwrap_or("")
                    .contains("Primary Inbox Message 01")
            })
            .map(|e| e.id)
            .expect("收件箱应含 Primary Inbox Message 01 种子邮件")
    });

    // 行选中同款事件：SelectionChanged → 详情装载 + 详情视图重建
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(email_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    email_id,
                ));
                cx.notify();
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == email_id).then_some(())
        })
    });

    // 正文文本以新渲染器探针键可见（text_probe_key：内容字母数字前缀 16 字符）
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("html-txt-Thisisdeterminis")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 11：降级路径——>1MB 正文经 render_pipeline TooLarge → 纯文本降级 +
// 顶部提示条出现、不 panic。形态选择：>1MB 正文经 save_draft 真实落库
// （e2e 草稿路径本地写入，compose e2e 已覆盖），再按行选中同款事件打开
// 详情——走完整降级链路（DetailStore 装载 → 管线超限 → 降级渲染），
// 不用函数级直调（保留 e2e 全栈语义）。
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn html_email_oversize_falls_back(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).accounts.read(app).groups.is_empty()).then_some(())
        })
    });

    let draft_id = {
        let account_id = with_app(cx, |app| {
            shell.read(app).accounts.read(app).groups[0].account.id
        });
        let req = postium_core::service::mail_draft::SaveDraftRequest {
            draft_id: None,
            account_id,
            to: vec!["e2e.user@postium.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Oversize Fallback Probe".into(),
            body_html: format!("<p>{}</p>", "x".repeat(1024 * 1024 + 64)),
            body_text: "超大邮件降级后的纯文本正文".into(),
            attachments: vec![],
        };
        crate::runtime::runtime()
            .block_on(async move { api.save_draft(req).await })
            .expect("超大草稿应保存成功")
            .draft_id
    };
    assert!(draft_id > 0, "草稿应保存并返回正 id");

    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(draft_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    draft_id,
                ));
                cx.notify();
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == draft_id).then_some(())
        })
    });

    // 降级提示条可见（debug_selector 探针，contacts-import-btn 同款查询模式）；
    // 超限分支不进入渲染树构建，测试进程存活即不 panic 断言
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("email-html-fallback")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 11 评审修复 P1：纯文本邮件不得误显示降级提示条——body_html 缺失时
// DetailStore.html = Some("")，空 html 不得进 render_pipeline（Err(Empty)
// 会把每封纯文本邮件误标「无法渲染」）。断言：提示条缺席 + 纯文本正文可见。
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn plain_text_email_no_fallback_banner(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).accounts.read(app).groups.is_empty()).then_some(())
        })
    });

    // 纯文本草稿：body_html 空白（DetailStore 装载后 html = Some("")）
    let draft_id = {
        let account_id = with_app(cx, |app| {
            shell.read(app).accounts.read(app).groups[0].account.id
        });
        let req = postium_core::service::mail_draft::SaveDraftRequest {
            draft_id: None,
            account_id,
            to: vec!["e2e.user@postium.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Plain Text Probe".into(),
            body_html: String::new(),
            body_text: "纯文本邮件正文，不应出现降级提示条。".into(),
            attachments: vec![],
        };
        crate::runtime::runtime()
            .block_on(async move { api.save_draft(req).await })
            .expect("纯文本草稿应保存成功")
            .draft_id
    };

    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(draft_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    draft_id,
                ));
                cx.notify();
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == draft_id).then_some(())
        })
    });

    cx.refresh().expect("测试窗口重绘调度失败");
    // 正文先渲染到位（纯文本路径探针可见）
    wait_for(cx, |cx| {
        cx.debug_bounds("email-plain-body")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    // 多帧后提示条始终缺席（出现即红——纯文本邮件不得误标「无法渲染」）
    for _ in 0..10 {
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("email-html-fallback").is_none(),
            "纯文本邮件不得出现降级提示条"
        );
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 11 评审修复 P2：文档缓存内容感知失效——同 id 新正文到达后旧文档不得
// 命中（生产同 id 换内容路径：同步 UPSERT_EMAIL_SQL 按 (account,folder,uid)
// 原位更新 body_html；评审所述草稿重存实测为「新行 id + 删旧行」，同 id
// 新内容经由 DetailStore.html 到达，此处沿该接缝驱动真实渲染路径断言）。
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn html_cache_same_id_new_content_renders(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).accounts.read(app).groups.is_empty()).then_some(())
        })
    });

    let draft_id = {
        let account_id = with_app(cx, |app| {
            shell.read(app).accounts.read(app).groups[0].account.id
        });
        let req = postium_core::service::mail_draft::SaveDraftRequest {
            draft_id: None,
            account_id,
            to: vec!["e2e.user@postium.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Cache Content Probe".into(),
            body_html: "<p>CacheKeyAlpha</p>".into(),
            body_text: "CacheKeyAlpha".into(),
            attachments: vec![],
        };
        crate::runtime::runtime()
            .block_on(async move { api.save_draft(req).await })
            .expect("缓存探针草稿应保存成功")
            .draft_id
    };

    // 打开详情 → Alpha 文本经新渲染器可见
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(draft_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    draft_id,
                ));
                cx.notify();
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == draft_id).then_some(())
        })
    });
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("html-txt-CacheKeyAlpha")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });

    // 同 id 新正文到达（同步重取/重载路径同款接缝）→ 重渲染必须展示新内容：
    // 内容感知键 (id, hash) 未命中 → 走管线渲染 Beta；id-only 键会命中陈旧
    // Alpha 文档 → Beta 缺席即红
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.detail.update(cx, |store, cx| {
                store.html = Some("<p>CacheKeyBeta</p>".into());
                cx.notify();
            });
        });
    });
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("html-txt-CacheKeyBeta")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("html-txt-CacheKeyAlpha").is_none(),
        "同 id 新内容渲染后陈旧文本不得残留"
    );
    cx.run_until_parked();
}

// ---------------------------------------------------------------------------
// Task 13：图片管线 UI——含远程图邮件打开后默认拦截（占位框出现、无已加载
// 真图、「加载图片」条可见）；点击加载按钮 → images_enabled 翻转；e2e_offline
// 下 fetch_image 短路 Err → resolved 记录失败（None）→ 占位框保留（无网络
// 请求路径由 core 短路结构性保证，断言占位框保持即为断言形态，T12 交接）。
// ---------------------------------------------------------------------------
#[gpui_pre::test]
fn html_images_blocked_until_allowed(cx: &mut TestAppContext) {
    // 真实 I/O（tokio 桥）与确定性调度器混用：允许跨线程唤醒
    cx.dispatcher.allow_parking();
    let dir = tempfile::tempdir().unwrap();
    // 放行记忆走 ui-state.json 持久化（线上反馈①）：E2E 数据目录进程级共享
    //（见 force_e2e_data_dir），不得写入真实 ~/.postium（否则下一次运行的
    // 默认拦截断言会被记忆击穿）；与主题持久化测试串行化防文件写交错
    force_e2e_data_dir();
    let _guard = UI_STATE_FILE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let (api, sync_rx) = boot(dir.path());
    // 草稿保存的 async move 会移走 api；重装载体用提前克隆（放行记忆段用）
    let reload_api = api.clone();

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api.clone(), sync_rx, window, cx)
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            (!shell.read(app).accounts.read(app).groups.is_empty()).then_some(())
        })
    });

    // 含远程图的 HTML 草稿（e2e_offline 本地落库，正文带跟踪像素形态远程图）
    let draft_id = {
        let account_id = with_app(cx, |app| {
            shell.read(app).accounts.read(app).groups[0].account.id
        });
        let req = postium_core::service::mail_draft::SaveDraftRequest {
            draft_id: None,
            account_id,
            to: vec!["e2e.user@postium.test".into()],
            cc: vec![],
            bcc: vec![],
            subject: "Remote Image Probe".into(),
            body_html: "<p>RemoteImageProbeText</p>\
                        <img src=\"https://tracking.example.test/pixel.png\" alt=\"远程图片探针\">"
                .into(),
            body_text: "远程图片拦截探针正文".into(),
            attachments: vec![],
        };
        crate::runtime::runtime()
            .block_on(async move { api.save_draft(req).await })
            .expect("远程图草稿应保存成功")
            .draft_id
    };

    // 打开详情（行选中同款事件接缝，T11 同款）
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell.emails.update(cx, |store, cx| {
                store.selected = Some(draft_id);
                store.selected_version += 1;
                cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(
                    draft_id,
                ));
                cx.notify();
            });
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let view = shell.read(app).detail_view.clone()?;
            let detail = view.read(app).store.read(app).detail.clone()?;
            (detail.email.id == draft_id).then_some(())
        })
    });

    // 默认拦截：占位框出现、无已加载真图、「加载图片」条可见
    cx.refresh().expect("测试窗口重绘调度失败");
    wait_for(cx, |cx| {
        cx.debug_bounds("html-img")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    assert!(
        cx.debug_bounds("html-img-loaded").is_none(),
        "默认拦截下不得出现已加载真图"
    );
    wait_for(cx, |cx| {
        cx.debug_bounds("email-load-images")
            .filter(|b| b.size.width > gpui_kit::px(0.) && b.size.height > gpui_kit::px(0.))
            .map(|_| ())
    });
    assert!(
        !with_app(cx, |app| shell.read(app).detail.read(app).images.enabled),
        "打开邮件时 images_enabled 应为 false"
    );

    // 点击「加载图片」（banner 探针 div 紧贴按钮，中心命中）
    let bounds = cx
        .debug_bounds("email-load-images")
        .expect("加载图片条应存在");
    cx.simulate_click(
        gpui_pre::point(
            bounds.origin.x + bounds.size.width / 2.,
            bounds.origin.y + bounds.size.height / 2.,
        ),
        gpui_pre::Modifiers::default(),
    );
    cx.run_until_parked();

    // images_enabled 翻转 + 条消失
    assert!(
        with_app(cx, |app| shell.read(app).detail.read(app).images.enabled),
        "点击加载按钮后 images_enabled 应翻转为 true"
    );
    wait_for(cx, |cx| {
        cx.debug_bounds("email-load-images").is_none().then_some(())
    });

    // e2e_offline：fetch_image 短路 Err → resolved 记录失败（None）→ 占位框保留
    let (resolved_url, resolved_failed) = wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).detail.read(app);
            store
                .images
                .resolved
                .lock()
                .unwrap()
                .iter()
                .next()
                .map(|(url, img)| (url.clone(), img.is_none()))
        })
    });
    assert_eq!(
        resolved_url, "https://tracking.example.test/pixel.png",
        "resolved 键应为远程图 URL"
    );
    assert!(resolved_failed, "e2e_offline 短路 Err 应记录为失败（None）");
    cx.refresh().expect("测试窗口重绘调度失败");
    assert!(
        cx.debug_bounds("html-img")
            .is_some_and(|b| b.size.width > gpui_kit::px(0.)),
        "加载失败后占位框应保留"
    );
    assert!(
        cx.debug_bounds("html-img-loaded").is_none(),
        "e2e_offline 下不得出现已加载真图"
    );

    // ---- 放行记忆（线上反馈①）----
    // 1) ui-state.json 已记下发件人；
    // 2) 重开同一封邮件（force 走重装载路径）：放行态从持久化恢复，不再询问
    let sender = with_app(cx, |app| {
        shell.read(app)
            .detail
            .read(app)
            .detail
            .as_ref()
            .map(|d| crate::ui_state::normalize_sender(&d.email.sender_email))
            .filter(|s| !s.is_empty())
            .expect("详情应带发件人")
    });
    let state = crate::ui_state::load();
    assert!(
        state.allows_remote_images_from(&sender),
        "点击放行后应按发件人持久化记忆"
    );
    with_app_mut(cx, |app| {
        shell.update(app, |shell, cx| {
            shell
                .detail
                .update(cx, |store, cx| store.force_load(draft_id, &reload_api, cx))
        });
    });
    wait_for(cx, |cx| {
        with_app(cx, |app| {
            let store = shell.read(app).detail.read(app);
            (store.images.enabled && !store.loading).then_some(())
        })
    });
    cx.refresh().expect("测试窗口重绘调度失败");
    assert!(
        cx.debug_bounds("email-load-images").is_none(),
        "已放行发件人的邮件重开不得再询问"
    );
    cx.run_until_parked();
}
