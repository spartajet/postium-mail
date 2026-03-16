#![allow(dead_code, ambiguous_glob_reexports)]
mod command;
mod config;
mod crypto;
mod database;
mod migration;
mod models;
pub mod services;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use command::{DatabaseState, KeyringState, OAuthState};

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

                app.manage(DatabaseState(std::sync::Arc::new(std::sync::Mutex::new(
                    db,
                ))));

                let oauth_config = config::load_oauth_config().expect("无法加载OAuth配置");
                let oauth_service = services::oauth_service::OAuthService::new(oauth_config)
                    .expect("无法初始化OAuth服务");
                app.manage(OAuthState(oauth_service));

                app.manage(KeyringState {
                    app_handle: app.handle().clone(),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
