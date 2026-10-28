//! Postium Mail 应用入口：数据目录 → 核心服务装配 → GPUI 主窗口。

mod app;
mod i18n;
mod nav;
mod notify;
mod runtime;
mod ui_state;
mod stores;
mod theme;
mod tray;
mod views;
mod window_state;

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
    postium_core::setup_logging();
    tracing::info!("Postium Mail (GPUI) 启动中...");

    let data_dir = resolve_data_dir();
    let e2e = std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1");
    let e2e_truth = std::env::var("POSTIUM_E2E_TRUTH").ok().as_deref() == Some("1");

    // 核心服务在 tokio 上装配完成后进入 UI（仅启动期一次）
    let (api, sync_rx) = runtime::runtime()
        .block_on(async { postium_core::api::CoreApi::init(&data_dir, e2e, e2e_truth).await })
        .expect("核心服务初始化失败");

    gpui_kit::application()
        .with_assets(gpui_kit_assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            postium_editor::init(cx);
            cx.set_global(i18n::I18nGlobal { lang: i18n::Lang::Zh });
            app::open_main_window(api, sync_rx, cx);
        });
}
