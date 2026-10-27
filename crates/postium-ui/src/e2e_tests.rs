//! GPUI 版 E2E 功能测试（迁移自 e2e/test/specs/*.e2e.js）。
//!
//! 模式：真实全栈（CoreApi + SQLite + e2e seed 固定数据 + 全部视图/状态），
//! headless 窗口；用户操作以 on_click 同款 handler 驱动 +
//! simulate_keystrokes 输入框键入（TestAppContext 无元素定位 API），
//! 异步 settle 用 run_until_parked + 轮询。

use std::sync::Arc;

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
        let handle = compose.read(app).to.read(app).focus_handle(app);
        window.focus(&handle, app);
    });
    cx.simulate_keystrokes("peer@example.com");
    let to_value = with_app(cx, |app| compose.read(app).to.read(app).value().to_string());
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
        let handle = first.read(app).to.read(app).focus_handle(app);
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
            second.read(app).to.read(app).value().to_string(),
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
