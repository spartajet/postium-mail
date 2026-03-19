#![allow(dead_code, ambiguous_glob_reexports, unused_variables, deprecated)]
mod command;
pub mod config;
mod crypto;
mod database;
mod migration;
mod models;
pub mod services;

// 新增模块
pub mod auth;  // 公开以支持测试
pub mod engine;
pub mod error;
pub mod providers;  // 公开以支持测试
pub mod sync;  // 公开以支持测试

// 重新导出关键类型
pub use auth::AuthManager;
pub use engine::FlowEngine;
pub use error::{MailError, Result};
pub use providers::{AccountType, MailProvider, OAuthConfig, ProviderPool};
pub use sync::SyncManager;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use command::{DatabaseState, FlowEngineState, KeyringState, OAuthState};

/// 处理 OAuth Deep Link 回调
fn handle_oauth_deep_link(app: &tauri::AppHandle, url: &str) {
    tracing::info!("收到 Deep Link: {}", url);

    let parsed_url = match Url::parse(url) {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("解析 Deep Link URL 失败: {}", e);
            return;
        }
    };

    let host = parsed_url.host_str().unwrap_or("");
    let path = parsed_url.path();

    if host != "oauth" || path != "/callback" {
        tracing::warn!("忽略非 OAuth Deep Link: host={}, path={}", host, path);
        return;
    }

    let query_params: std::collections::HashMap<String, String> = parsed_url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let code = query_params.get("code").cloned().unwrap_or_default();
    let state = query_params.get("state").cloned().unwrap_or_default();
    let error = query_params.get("error").cloned();
    let error_description = query_params.get("error_description").cloned();

    tracing::info!(
        "OAuth Deep Link 参数: code={}, state={}, error={:?}",
        if code.is_empty() { "无" } else { "有" },
        if state.is_empty() { "无" } else { "有" },
        error
    );

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.unminimize();
        let _ = window.show();
    }

    let emit_result = if let Some(err) = error {
        app.emit(
            "oauth-deep-link-callback",
            serde_json::json!({
                "error": err,
                "errorDescription": error_description.unwrap_or_default()
            }),
        )
    } else {
        app.emit(
            "oauth-deep-link-callback",
            serde_json::json!({
                "code": code,
                "state": state
            }),
        )
    };

    if let Err(e) = emit_result {
        tracing::error!("发射 OAuth Deep Link 事件失败: {}", e);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("单实例检测：收到参数 {:?}", args);

            for arg in args {
                if arg.starts_with("postium-mail://") {
                    tracing::info!("单实例转发 Deep Link: {}", arg);
                    handle_oauth_deep_link(app, &arg);
                    return;
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
                let _ = window.show();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_keyring::init())
        .setup(|app| {
            // ========== 系统托盘初始化 ==========
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.unminimize();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                            let _ = window.unminimize();
                        }
                    }
                })
                .build(app)?;

            // ========== 窗口关闭事件处理 ==========
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                        tracing::info!("窗口已隐藏到系统托盘");
                    }
                });
            }

            tracing::info!("系统托盘初始化完成");

            // ========== Deep Link 事件处理器 ==========
            let app_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    handle_oauth_deep_link(&app_handle, url.as_str());
                }
            });

            #[cfg(target_os = "windows")]
            {
                if let Err(e) = app.deep_link().register("postium-mail") {
                    tracing::warn!("注册 Deep Link 协议失败: {}", e);
                } else {
                    tracing::info!("Deep Link 协议注册成功");
                }
            }

            // ========== 数据库和服务初始化 ==========
            tauri::async_runtime::block_on(async move {
                let db = database::establish_connection()
                    .await
                    .expect("无法连接到数据库");

                database::init_database(&db)
                    .await
                    .expect("数据库初始化失败");

                // 将 db 包装在 Arc 中，以便多处使用
                let db_arc = std::sync::Arc::new(db);

                app.manage(DatabaseState(std::sync::Arc::new(std::sync::Mutex::new(
                    (*db_arc).clone(),
                ))));

                let oauth_config =
                    config::load_microsoft_oauth_config().expect("无法加载OAuth配置");
                let oauth_service = services::oauth_service::OAuthService::new(oauth_config)
                    .expect("无法初始化OAuth服务");
                app.manage(OAuthState(oauth_service));

                app.manage(KeyringState {
                    app_handle: app.handle().clone(),
                });

                // ========== FlowEngine 初始化 ==========
                // 创建 AuthManager
                let auth_manager = std::sync::Arc::new(
                    auth::AuthManager::new(&app.handle())
                        .expect("无法创建 AuthManager"),
                );

                // 创建 ProviderPool（与 AuthManager 使用相同的实例）
                let provider_pool = std::sync::Arc::new(providers::ProviderPool::new());

                // 创建 SyncManager
                let sync_manager = std::sync::Arc::new(sync::SyncManager::new(
                    db_arc.clone(),
                    app.handle().clone(),
                    auth_manager,
                    provider_pool,
                ));

                // 创建 FlowEngine
                let flow_engine = engine::FlowEngine::new(
                    db_arc,
                    sync_manager,
                    app.handle().clone(),
                );

                // 注册 FlowEngineState（包装在 Arc<tokio::sync::Mutex> 中）
                let flow_engine_state = std::sync::Arc::new(tokio::sync::Mutex::new(
                    flow_engine,
                ));
                app.manage(FlowEngineState(flow_engine_state.clone()));

                // 启动 FlowEngine
                {
                    let engine = flow_engine_state.lock().await;
                    if let Err(e) = engine.start().await {
                        tracing::error!("FlowEngine 启动失败: {}", e);
                    } else {
                        tracing::info!("FlowEngine 已启动");
                    }
                }

                // ========== FlowEngine 优雅关闭 ==========
                // 监听应用退出事件，停止 FlowEngine
                let _ = app.listen("tauri://destroy", move |_| {
                    let engine_state = flow_engine_state.clone();
                    tauri::async_runtime::block_on(async move {
                        use tokio::time::{timeout, Duration};

                        tracing::info!("正在停止 FlowEngine...");
                        let engine_guard = engine_state.lock().await;
                        let stop_result = timeout(Duration::from_secs(5), engine_guard.stop()).await;

                        match stop_result {
                            Ok(Ok(())) => tracing::info!("FlowEngine 已停止"),
                            Ok(Err(e)) => tracing::error!("FlowEngine 停止失败: {}", e),
                            Err(_) => tracing::warn!("FlowEngine 停止超时，将强制退出"),
                        }
                    });
                });

                tracing::info!("Postium Mail 后端初始化完成");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号管理
            command::add_account,
            command::list_accounts,
            command::get_account,
            command::update_account,
            command::delete_account,
            command::test_account_connection,
            command::test_email_connection,
            // OAuth
            command::validate_oauth_token,
            command::get_oauth_auth_url,
            command::exchange_oauth_code,
            command::refresh_oauth_token,
            // 邮件操作
            command::list_emails,
            command::get_email,
            command::search_emails_fts,
            command::mark_as_read,
            command::toggle_star,
            command::delete_emails,
            command::move_email_to_folder,
            command::get_folder_stats,
            // 同步
            command::sync_account,
            command::sync_account_with_progress,
            command::send_email,
            // FlowEngine 管理
            command::get_flow_engine_status,
            command::add_sync_task,
            command::remove_sync_task,
            command::pause_sync_task,
            command::resume_sync_task,
            command::trigger_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
