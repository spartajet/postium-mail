//! Postium Mail 应用入口：数据目录 → 核心服务装配 → GPUI 主窗口。
//!
//! Windows 子系统（线上反馈⑩）：release 构建设为 `windows`——不分配控制台
//! 窗口（GUI 应用默认 console 子系统会随启动弹黑窗）。debug 保留控制台
//! 便于开发期看 stderr 日志；两配置的日志都同时落
//! `~/.postium/logs/postium.log`（滚动文件，见 sys/log.rs）。

// 位置必须在所有 items 之前（crate 级内部属性）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;
mod nav;
mod notify;
mod runtime;
mod screenshot;
mod sound;
mod ui_state;
mod stores;
mod theme;
mod tray;
mod views;
mod window_state;

#[cfg(test)]
mod e2e_mail_menu_tests;
#[cfg(test)]
mod e2e_mail_view_tests;
#[cfg(test)]
mod e2e_tests;

use std::path::PathBuf;

/// 解析应用数据目录（对齐 Tauri 版逻辑，见 src-tauri/src/lib.rs:resolve_data_dir）。
pub fn resolve_data_dir() -> PathBuf {
    if std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1") {
        return PathBuf::from(
            std::env::var("POSTIUM_DATA_DIR").expect("POSTIUM_E2E=1 时必须设置 POSTIUM_DATA_DIR"),
        );
    }
    dirs::home_dir().expect("无法获取用户目录").join(".postium")
}

fn main() {
    let data_dir = resolve_data_dir();
    // 日志 flush worker 的 guard 必须全程持有（drop 即停 flush，缓冲丢失）
    let _log_guard = postium_core::setup_logging(data_dir.join("logs"));
    tracing::info!("Postium Mail (GPUI) 启动中...");

    // --screenshot <file.png>：为 Craft Anything 构建检查输出截图后退出
    let screenshot = screenshot::init_from_args();
    let screenshot_mode = screenshot.is_some();

    let e2e = std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1");
    let e2e_truth = std::env::var("POSTIUM_E2E_TRUTH").ok().as_deref() == Some("1");

    // 核心服务在 tokio 上装配完成后进入 UI（仅启动期一次）。
    // 事件通道由入口创建：tx 存入 CoreApi（IMAP 引擎复用），rx 交给主窗口消费。
    let (sync_tx, sync_rx) =
        smol::channel::bounded::<postium_core::domain::sync::SyncProgressEvent>(256);
    let api = runtime::runtime()
        .block_on(async {
            postium_core::api::CoreApi::init(&data_dir, e2e, e2e_truth, sync_tx).await
        })
        .expect("核心服务初始化失败");

    gpui_kit::application()
        .with_assets(gpui_kit_assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            postium_editor::init(cx);
            // 窗口全关后应用驻留（线上反馈⑤：关闭到托盘）——非 macOS 的
            // QuitMode::Default 在最后一个窗口关闭时直接退出进程（托盘
            // 一并带走）。改 Explicit：仅显式 cx.quit() 退出（托盘菜单/
            // 关闭行为=退出）。
            cx.set_quit_mode(gpui_kit::QuitMode::Explicit);
            cx.set_global(i18n::I18nGlobal { lang: i18n::Lang::Zh });
            app::open_main_window(api, Some(sync_rx), cx);
            // 截图模式：不激活应用（不抢用户焦点）
            if screenshot_mode {
                cx.activate(false);
            }
        });
}
