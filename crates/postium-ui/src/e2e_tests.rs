//! GPUI 版 E2E 功能测试（迁移自 e2e/test/specs/*.e2e.js）。
//!
//! 模式：真实全栈（CoreApi + SQLite + e2e seed 固定数据 + 全部视图/状态），
//! headless 窗口；用户操作以 on_click 同款 handler 驱动 +
//! simulate_keystrokes 输入框键入（TestAppContext 无元素定位 API），
//! 异步 settle 用 run_until_parked + 轮询。

use std::sync::Arc;

use chrono::Datelike as _;
use gpui_pre::{Focusable as _, TestAppContext, VisualTestContext};
use postium_core::api::CoreApi;

use crate::app::AppShell;
use crate::nav::NavTarget;

fn boot(dir: &std::path::Path) -> (Arc<CoreApi>, postium_core::api::SyncEventRx) {
    crate::runtime::runtime()
        .block_on(async { CoreApi::init(dir, true, false).await })
        .expect("核心服务初始化失败")
}

fn with_app<R>(cx: &mut VisualTestContext, f: impl FnOnce(&gpui_pre::App) -> R) -> R {
    cx.update(|_, app| f(app))
}

/// 在 &mut App 上执行（实体更新需要可变上下文）。
fn with_app_mut<R>(cx: &mut VisualTestContext, f: impl FnOnce(&mut gpui_pre::App) -> R) -> R {
    cx.update(|_, app| f(app))
}

/// 同时拿 window 与 &mut App（焦点设置等需要 window）。
fn with_window_mut<R>(cx: &mut VisualTestContext, f: impl FnOnce(&mut gpui_pre::Window, &mut gpui_pre::App) -> R) -> R {
    cx.update(|window, app| f(window, app))
}

fn wait_for<T>(cx: &mut VisualTestContext, mut f: impl FnMut(&mut VisualTestContext) -> Option<T>) -> T {
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
    cx.set_global(crate::i18n::I18nGlobal { lang: crate::i18n::Lang::Zh });
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

    let count = wait_for(cx, |cx| with_app(cx, |app| {
        let n = shell.read(app).accounts.read(app).groups.len();
        (n >= 2).then_some(n)
    }));
    assert_eq!(count, 2, "seed 应含两个账号");

    let ok = with_app(cx, |app| {
        let groups = &shell.read(app).accounts.read(app).groups;
        groups.iter().any(|g| g.account.email.contains("primary"))
            && groups.iter().any(|g| g.account.email.contains("secondary"))
    });
    assert!(ok, "seed 账号应含 primary 与 secondary");

    let emails = wait_for(cx, |cx| with_app(cx, |app| {
        let store = shell.read(app).emails.read(app);
        (!store.emails.is_empty()).then(|| store.emails.clone())
    }));
    assert!(
        emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Primary Inbox Message")),
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

    wait_for(cx, |cx| with_app(cx, |app| {
        let n = shell.read(app).accounts.read(app).groups.len();
        (n >= 2).then_some(n)
    }));

    let second_id = with_app(cx, |app| {
        shell.read(app)
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
            shell.nav = NavTarget::AccountFolder { account_id: second_id, folder: "INBOX".into() };
            shell.emails.update(cx, |store, cx| {
                store.navigate(shell.nav.clone(), &api, cx);
            });
        });
    });

    let emails = wait_for(cx, |cx| with_app(cx, |app| {
        let store = shell.read(app).emails.read(app);
        store
            .emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Secondary"))
            .then(|| store.emails.clone())
    }));
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
    wait_for(cx, |cx| with_app(cx, |app| {
        shell.read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Primary Inbox Message"))
            .then_some(())
    }));
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
    wait_for(cx, |cx| with_app(cx, |app| {
        let n = shell.read(app).accounts.read(app).groups.len();
        (n >= 1).then_some(n)
    }));

    fn navigate(
        shell: &gpui_kit::Entity<AppShell>,
        cx: &mut VisualTestContext,
        target: NavTarget,
    ) {
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

    navigate(&shell, cx, NavTarget::Category(postium_core::service::email_service::EmailCategory::Sent));
    wait_for(cx, |cx| with_app(cx, |app| {
        shell.read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Sent Confirmation"))
            .then_some(())
    }));

    navigate(&shell, cx, NavTarget::Category(postium_core::service::email_service::EmailCategory::Starred));
    wait_for(cx, |cx| with_app(cx, |app| {
        shell.read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Starred Reference"))
            .then_some(())
    }));

    navigate(&shell, cx, NavTarget::AllInbox);
    wait_for(cx, |cx| with_app(cx, |app| {
        shell.read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .any(|e| e.subject.as_deref().unwrap_or("").contains("Primary Inbox Message"))
            .then_some(())
    }));
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
    wait_for(cx, |cx| with_app(cx, |app| {
        (!shell.read(app).emails.read(app).emails.is_empty()).then_some(())
    }));

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

    let results = wait_for(cx, |cx| with_app(cx, |app| {
        let store = shell.read(app).emails.read(app);
        (store.list_version > before && !store.emails.is_empty())
            .then(|| store.emails.clone())
    }));
    assert!(results.len() >= 2, "Quarterly 应至少命中 Alpha/Beta");
    assert!(
        results
            .iter()
            .all(|e| e.subject.as_deref().unwrap_or("").contains("Quarterly Planning")),
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

    let target_id = wait_for(cx, |cx| with_app(cx, |app| {
        shell.read(app)
            .emails
            .read(app)
            .emails
            .iter()
            .find(|e| !e.is_starred && e.subject.as_deref().unwrap_or("").contains("Primary Inbox"))
            .map(|e| e.id)
    }));

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
    wait_for(cx, |cx| with_app(cx, |app| {
        let store = shell.read(app).emails.read(app);
        (!store.emails.iter().any(|e| e.id == target_id)).then_some(())
    }));
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
    wait_for(cx, |cx| with_app(cx, |app| compose.read(app).selected_account.clone()));

    // 键入收件人（焦点 + 键盘事件）
    with_window_mut(cx, |window, app| {
        let handle = compose.read(app).to_field.input.read(app).focus_handle(app);
        window.focus(&handle, app);
    });
    cx.simulate_keystrokes("peer@example.com");
    let to_value = with_app(cx, |app| compose.read(app).to_field.input.read(app).value().to_string());
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
    let draft_id = wait_for(cx, |cx| with_app(cx, |app| compose.read(app).draft_email_id));
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
            second.read(app).to_field.input.read(app).value().to_string(),
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
    // ui-state 持久化路径走 resolve_data_dir：测试内强制 E2E 目录
    unsafe {
        std::env::set_var("POSTIUM_E2E", "1");
        std::env::set_var("POSTIUM_DATA_DIR", dir.path().to_str().unwrap());
    }

    let (api, sync_rx) = boot(dir.path());

    let (shell, cx) = cx.add_window_view(|window, cx| {
        setup(cx);
        AppShell::new(api, sync_rx, window, cx)
    });

    use gpui_kit::component::ActiveTheme as _;
    let mode = |cx: &mut VisualTestContext| -> bool {
        with_app(cx, |app| app.theme().mode == gpui_kit::component::ThemeMode::Dark)
    };

    let initial_dark = mode(cx);
    with_window_mut(cx, |window, app| {
        shell.update(app, |shell, cx| {
            shell.toggle_theme(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    assert_ne!(mode(cx), initial_dark, "第一次切换应翻转主题");

    let saved = std::fs::read_to_string(dir.path().join("ui-state.json")).unwrap_or_default();
    assert!(saved.contains("theme"), "主题应写入 ui-state.json: {saved}");

    with_window_mut(cx, |window, app| {
        shell.update(app, |shell, cx| {
            shell.toggle_theme(&gpui_pre::ClickEvent::default(), window, cx);
        });
    });
    assert_eq!(mode(cx), initial_dark, "第二次切换应回到原主题");

    unsafe {
        std::env::remove_var("POSTIUM_E2E");
        std::env::remove_var("POSTIUM_DATA_DIR");
    }
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
                        e.subject.as_deref().unwrap_or("").contains("Calendar Today Alpha")
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
            && subjects.iter().any(|s| s.contains("Calendar Today Beta Starred")),
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
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, api_for_view.clone(), window, cx)
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
    assert!(provider_count >= 2, "服务商列表应非空，实际 {provider_count}");

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
            settings
                .read(app)
                .add
                .as_ref()
                .and_then(|a| a.pending_sync)
        })
    });
    assert!(pending > 0, "创建账号应返回正 id");
    let step = with_app(cx, |app| {
        settings.read(app).add.as_ref().map(|a| a.step)
    });
    assert_eq!(step, Some(crate::views::settings::AddStep::SyncScope), "提交后应进入同步范围步骤");

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
        compose.read(app).to_field.input.read(app).value().to_string()
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
    assert_eq!(emails, vec!["b@postium.test".to_string()], "删除 chip 应生效");
    cx.run_until_parked();
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
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, api_for_view.clone(), window, cx)
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
            shell.read(app).emails.read(app).emails.first().unwrap().clone()
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
        let accounts = crate::stores::accounts::init_accounts_store(&api_for_view, cx);
        crate::views::settings::SettingsView::new(accounts, api_for_view.clone(), window, cx)
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
        assert_eq!(
            with_app(cx, |app| settings.read(app).active_section),
            want
        );
    }
    cx.run_until_parked();
}
