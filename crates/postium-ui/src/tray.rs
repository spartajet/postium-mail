//! 系统托盘（tray-icon crate）：显示窗口 / 写邮件 / 同步 / 退出。
//! 菜单事件经静态 channel，由 UI 线程轮询消费（对齐旧 sys/tray.rs 的菜单项）。

use std::sync::Arc;

use gpui_kit::App;
use postium_core::api::CoreApi;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::stores::sync::SyncStore;

#[allow(dead_code)]
struct TrayActions {
    #[allow(dead_code)]
    show: u32,
    compose: u32,
    sync: u32,
    quit: u32,
}

static ACTIONS_ID: u32 = 1;

/// 装配托盘并启动事件消费循环。
pub fn setup_tray(
    api: Arc<CoreApi>,
    sync_store: gpui_kit::Entity<SyncStore>,
    cx: &mut App,
) -> Option<TrayIcon> {
    // 编译期定位 crate 内图标（不依赖 CWD，也不依赖待移除的旧轨目录）
    let icon_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.ico");
    let icon = match tray_icon::Icon::from_path(icon_path, None) {
        Ok(icon) => icon,
        Err(e) => {
            tracing::warn!("托盘图标加载失败（{e}），托盘不可用");
            return None;
        }
    };

    let show = MenuItem::with_id("show", "显示窗口", true, None);
    let compose = MenuItem::with_id("compose", "写邮件", true, None);
    let sync = MenuItem::with_id("sync", "同步", true, None);
    let quit = MenuItem::with_id("quit", "退出", true, None);
    let menu = Menu::new();
    let _ = menu.append_items(&[&show, &compose, &sync, &quit]);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Postium Mail")
        .with_icon(icon)
        .build();

    // 事件消费：UI 线程轮询（菜单事件量极小，250ms 足够）
    let api = api.clone();
    cx.spawn(async move |cx| {
        let menu_rx = MenuEvent::receiver();
        let tray_rx = tray_icon::TrayIconEvent::receiver();
        loop {
            // 左键单击托盘 → 显示主窗口
            while let Ok(event) = tray_rx.try_recv() {
                if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } = event {
                    cx.update(|cx| {
                        if let Some(handle) = cx.active_window() {
                            let _ = handle.update(cx, |_, window, _| {
                                window.activate_window();
                            });
                        }
                    });
                }
            }
            while let Ok(event) = menu_rx.try_recv() {
                let id = event.id.0.as_str();
                match id {
                    "show" => {
                        cx.update(|cx| {
                            if let Some(handle) = cx.active_window() {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.activate_window();
                                });
                            }
                        });
                    }
                    "compose" => {
                        cx.update(|cx| {
                            let api = api.clone();
                            cx.defer(move |cx| {
                                crate::views::compose::open_compose_window(
                                    crate::views::compose::ComposeMode::New,
                                    &api,
                                    cx,
                                );
                            });
                        });
                    }
                    "sync" => {
                        let api = api.clone();
                        cx.update(|cx| {
                            sync_store.update(cx, |store, cx| {
                                store.sync_all(&api, cx);
                            });
                        });
                    }
                    "quit" => {
                        cx.update(|cx| {
                            if let Some(handle) = cx.active_window() {
                                let _ = handle.update(cx, |_, window, _| {
                                    crate::window_state::save(window);
                                });
                            }
                            cx.quit();
                        });
                    }
                    _ => {}
                }
            }
            smol::Timer::after(std::time::Duration::from_millis(250)).await;
        }
    })
    .detach();

    let _ = ACTIONS_ID;
    tracing::info!("系统托盘已就绪");
    tray.ok()
}
