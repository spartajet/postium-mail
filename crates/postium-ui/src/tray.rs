//! 系统托盘（tray-icon crate）：显示窗口 / 写邮件 / 同步 / 退出。
//! 菜单事件经静态 channel，由 UI 线程轮询消费（对齐旧 sys/tray.rs 的菜单项）。

use std::sync::Arc;

use gpui_kit::App;
use postium_core::api::CoreApi;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::stores::sync::SyncStore;


thread_local! {
    /// 托盘图标**进程级持有**（线上反馈⑨续）：此前图标挂在 AppShell 的
    /// `_tray` 字段上——「最小化到托盘」销毁主窗口时 AppShell 实体一并
    /// drop，TrayIcon::Drop 即 Shell_NotifyIcon(NIM_DELETE)，图标随之消失
    /// （进程与轮询任务是 App 级的所以活着）。thread_local 而非 static：
    /// Windows TrayIcon 含裸 HWND 非 Send/Sync，且须与消息泵同线程；随
    /// 主线程终结（进程退出）析构，跨主窗口重建幂等复用。
    static TRAY_HOLD: std::cell::RefCell<Option<TrayIcon>> = const { std::cell::RefCell::new(None) };
}

/// 事件轮询任务只启动一次（主窗口重建时 setup_tray 幂等复用图标，轮询
/// 不得重复 spawn——静态 channel 单消费者语义）。
static TRAY_CONSUMER_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 托盘「显示窗口」/左键单击共享：主窗口存活 → 激活置前；已关闭（关闭到
/// 托盘）→ 重建主窗口。
fn show_main_window(api: &Arc<CoreApi>, cx: &mut App) {
    crate::app::show_main_window(api, cx);
}

/// 装配托盘并启动事件消费循环（幂等：图标已持有则直接复用——主窗口
/// 重建路径反复调用）。
///
/// 返回托盘是否可用（供关闭行为判定：无托盘环境真关闭，否则程序退出
/// 只能走托盘菜单）。
///
/// 托盘是可选能力：Linux 无 appindicator/无会话总线等环境下
/// tray-icon 内部会 panic（毒化其全局 Lazy），此处兜底捕获，
/// 失败降级为无托盘运行——保证 headless CI 与无托盘桌面可用。
pub fn setup_tray(
    api: Arc<CoreApi>,
    sync_store: gpui_kit::Entity<SyncStore>,
    cx: &mut App,
) -> bool {
    if TRAY_HOLD.with(|t| t.borrow().is_some()) {
        return true;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        setup_tray_inner(api, sync_store, cx)
    }));
    match result {
        Ok(Some(tray)) => {
            TRAY_HOLD.with(|t| *t.borrow_mut() = Some(tray));
            true
        }
        Ok(None) => false,
        Err(payload) => {
            tracing::warn!("托盘初始化失败（无托盘环境属预期）：{payload:?}");
            false
        }
    }
}

fn setup_tray_inner(
    api: Arc<CoreApi>,
    sync_store: gpui_kit::Entity<SyncStore>,
    cx: &mut App,
) -> Option<TrayIcon> {
    // 编译期定位 crate 内图标（不依赖 CWD，也不依赖待移除的旧轨目录）
    let icon_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/icon.ico");
    let icon = load_tray_icon(&icon_path);

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

    // 事件消费：UI 线程轮询（菜单事件量极小，250ms 足够）。幂等：图标
    // 进程级复用后 setup_tray 可能再次到达，轮询任务只启动一次。
    if TRAY_CONSUMER_STARTED
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        tracing::debug!("托盘事件轮询已启动，跳过重复 spawn");
    } else {
        spawn_tray_consumer(api.clone(), sync_store, cx);
    }

    match tray {
        Ok(t) => {
            tracing::info!("系统托盘已就绪");
            Some(t)
        }
        Err(e) => {
            tracing::warn!("托盘构建失败：{e}");
            None
        }
    }
}

/// 托盘菜单/点击事件轮询（App 级任务：不随主窗口生灭——关闭到托盘后
/// 依然消费事件）。
fn spawn_tray_consumer(
    api: Arc<CoreApi>,
    sync_store: gpui_kit::Entity<SyncStore>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        let menu_rx = MenuEvent::receiver();
        let tray_rx = tray_icon::TrayIconEvent::receiver();
        loop {
            // 左键单击托盘 → 显示主窗口（与菜单「显示窗口」同逻辑——线上
            // 反馈⑤前左键只激活 active_window，主窗口销毁（关闭到托盘）后
            // 点左键无响应）
            while let Ok(event) = tray_rx.try_recv() {
                if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } = event {
                    cx.update(|cx| {
                        show_main_window(&api, cx);
                    });
                }
            }
            while let Ok(event) = menu_rx.try_recv() {
                let id = event.id.0.as_str();
                match id {
                    "show" => {
                        cx.update(|cx| {
                            show_main_window(&api, cx);
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
                            // 仅主窗口存活时保存其状态（走 MainShell 定位窗口）
                            if let Some(main) = cx.try_global::<crate::app::MainShell>()
                                && let Some(shell) = main.0.upgrade()
                            {
                                let _ = shell.update(cx, |shell, cx| {
                                    let _ = shell.window_handle.update(cx, |_, window, _| {
                                        crate::window_state::save(window);
                                    });
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
}

/// 加载托盘图标：Windows 用 ICO 多尺寸路径；其他平台解码内嵌 PNG 为 RGBA。
/// （tray-icon 的 `Icon::from_path` 仅 Windows 存在，跨平台需 from_rgba。）
fn load_tray_icon(icon_path: &std::path::Path) -> tray_icon::Icon {
    #[cfg(windows)]
    {
        match tray_icon::Icon::from_path(icon_path, None) {
            Ok(icon) => icon,
            Err(e) => {
                tracing::warn!("托盘图标加载失败（{e}），使用内嵌 PNG 兜底");
                embedded_png_icon()
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = icon_path;
        embedded_png_icon()
    }
}

/// 解码内嵌的 128×128 PNG 并转为 RGBA 图标。
fn embedded_png_icon() -> tray_icon::Icon {
    const ICON_PNG: &[u8] = include_bytes!("../icons/icon.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    match decoder.read_info() {
        Ok(mut reader) => {
            let mut buf = vec![0u8; reader.output_buffer_size()];
            match reader.next_frame(&mut buf) {
                Ok(info) => tray_icon::Icon::from_rgba(buf, info.width, info.height)
                    .unwrap_or_else(|e| {
                        tracing::warn!("托盘图标 RGBA 转换失败（{e}）");
                        fallback_icon()
                    }),
                Err(e) => {
                    tracing::warn!("内嵌托盘图标解码失败（{e}）");
                    fallback_icon()
                }
            }
        }
        Err(e) => {
            tracing::warn!("内嵌托盘图标读取失败（{e}）");
            fallback_icon()
        }
    }
}

/// 1×1 透明兜底图标（保证托盘可用）。
fn fallback_icon() -> tray_icon::Icon {
    tray_icon::Icon::from_rgba(vec![0, 0, 0, 0], 1, 1)
        .expect("1x1 RGBA 常量必然合法")
}
