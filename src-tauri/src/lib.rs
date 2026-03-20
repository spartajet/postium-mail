//! # Postium Mail - 邮件客户端后端
//!
//! 跨平台桌面邮件客户端的 Rust 后端，基于 Tauri 框架构建。
//!
//! ## 架构概览
//!
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │                前端 (Vue 3)                  │
//! └──────────────────┬──────────────────────────┘
//!                   │ IPC / Events
//! ┌──────────────────▼──────────────────────────┐
//! │               Tauri 命令层                    │
//! │          (command 模块)                      │
//! └──────────────────┬──────────────────────────┘
//!                   │
//! ┌──────────────────▼──────────────────────────┐
//! │               业务逻辑层                       │
//! │  ┌─────────────────────────────────────┐    │
//! │  │ FlowEngine │ AuthManager │ SyncMgr  │    │
//! │  └─────────────────────────────────────┘    │
//! └──────────────────┬──────────────────────────┘
//!                   │
//! ┌──────────────────▼──────────────────────────┐
//! │               服务层                          │
//! │  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
//! │  │ Storage  │  │ Providers│  │ Protocols│  │
//! │  └──────────┘  └──────────┘  └──────────┘  │
//! └──────────────────────────────────────────────┘
//! ```
//!
//! ## 模块说明
//!
//! ### [`command`] - Tauri 命令处理
//!
//! 定义前端可调用的所有 IPC 命令，包括：
//! - 账号管理
//! - 邮件操作
//! - 同步管理
//! - FlowEngine 控制
//! - OAuth 认证
//!
//! ### [`auth`] - 认证管理
//!
//! 统一的认证管理，支持：
//! - 密码认证
//! - OAuth2 认证
//! - 企业认证
//! - Token 自动刷新
//!
//! ### [`providers`] - 邮件服务商
//!
//! 服务商抽象层，支持：
//! - 个人邮箱（Gmail, Outlook, QQ 等）
//! - 企业邮箱（Microsoft 365, Google Workspace）
//! - 自动服务商检测
//! - 默认服务器配置
//!
//! ### [`protocols`] - 邮件协议
//!
//! IMAP/SMTP 协议实现：
//! - [`imap`] - 异步 IMAP 客户端（基于 async-imap）
//! - [`smtp`] - SMTP 客户端（基于 lettre）
//!
//! ### [`sync`] - 邮件同步
//!
//! 完整的同步解决方案：
//! - 增量同步（CONDSTORE）
//! - 实时推送（IDLE）
//! - 进度跟踪
//! - 错误恢复
//!
//! ### [`storage`] - 数据持久化
//!
//! 数据库和缓存管理：
//! - SeaORM 数据访问
//! - Keyring 凭证存储
//! - 多级缓存策略
//! - FTS5 全文搜索
//!
//! ### [`engine`] - 流程引擎
//!
//! 基于 Tokio 的异步流程编排：
//! - 任务调度
//! - 状态管理
//! - 事件发布
//!
//! ### [`error`] - 错误处理
//!
//! 统一的错误类型定义。
//!
//! ## 功能特性
//!
//! - **多账号**: 支持同时管理多个邮箱账号
//! - **OAuth2**: 支持 Gmail、Outlook OAuth2 登录
//! - **增量同步**: 基于 CONDSTORE 的高效同步
//! - **实时推送**: 支持 IDLE 实时新邮件通知
//! - **全文搜索**: 基于 FTS5 的邮件内容搜索
//! - **安全存储**: 使用系统 Keyring 存储敏感信息
//!
//! ## 开发指南
//!
//! ### 日志配置
//!
//! 使用 `RUST_LOG` 环境变量控制日志级别：
//!
//! ```bash
//! # 默认级别（INFO）
//! RUST_LOG=info
//!
//! # 开发调试（DEBUG）
//! RUST_LOG=debug
//!
//! # 只显示本模块的 TRACE 日志
//! RUST_LOG=postium_mail=trace
//!
//! # 只显示某个子模块
//! RUST_LOG=postium_mail::sync=debug
//! ```
//!
//! ### 数据库
//!
//! 数据库文件位置：
//! - **Windows**: `%APPDATA%\com.postium.mail\data.db`
//! - **macOS**: `~/Library/Application Support/com.postium.mail/data.db`
//! - **Linux**: `~/.config/com.postium.mail/data.db`
//!
//! ### Keyring 存储
//!
//! 敏感信息存储在系统 Keyring 中：
//!
//! ```text
//! Service: "com.postium.mail"
//! Username: "password:<account_id>"    → 密码
//! Username: "oauth:<account_id>"       → OAuth Token
//! ```

#![allow(dead_code, ambiguous_glob_reexports, unused_variables, deprecated)]
mod command;
pub mod config;
mod crypto;
pub mod database; // 公开以支持测试

// 新增模块
pub mod auth; // 公开以支持测试
pub mod engine;
pub mod error;
pub mod protocols; // 协议层（IMAP/SMTP）
pub mod providers; // 公开以支持测试
pub mod storage; // 存储层
pub mod sync; // 公开以支持测试

// 重新导出关键类型
pub use auth::AuthManager;
pub use database::init_database;
pub use engine::FlowEngine;
pub use error::{MailError, Result};
pub use providers::{AccountType, MailProvider, OAuthConfig, ProviderPool};
pub use sync::SyncManager; // 用于测试

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use url::Url;

use command::{AuthManagerState, DatabaseState, FlowEngineState, KeyringState, ProviderPoolState};

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

/// 初始化 tracing 日志系统
///
/// 使用环境变量 `RUST_LOG` 控制日志级别，例如：
/// - `RUST_LOG=info` - 只显示 INFO 及以上级别
/// - `RUST_LOG=debug` - 显示 DEBUG 及以上级别（开发调试用）
/// - `RUST_LOG=postium_mail=trace` - 只对本模块使用 TRACE 级别
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO) // 默认级别
        .with_target(false) // 显示模块路径，便于调试
        .with_thread_ids(false) // 线程ID通常不需要
        .with_file(true) // 不显示文件名，减少日志冗余
        .with_line_number(true) // 不显示行号
        .compact() // 使用紧凑格式
        .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志系统
    init_tracing();

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

                app.manage(KeyringState {
                    app_handle: app.handle().clone(),
                });

                // ========== FlowEngine 初始化 ==========
                // 创建 AuthManager
                let auth_manager = std::sync::Arc::new(
                    auth::AuthManager::new(app.handle()).expect("无法创建 AuthManager"),
                );

                // 创建 ProviderPool（注册所有默认服务商）
                let provider_pool = std::sync::Arc::new(providers::ProviderPool::default());

                // 创建 SyncManager（使用 clone）
                let sync_manager = std::sync::Arc::new(sync::SyncManager::new(
                    db_arc.clone(),
                    app.handle().clone(),
                    auth_manager.clone(),
                    provider_pool.clone(),
                ));

                // 创建 FlowEngine
                let flow_engine =
                    engine::FlowEngine::new(db_arc, sync_manager, app.handle().clone());

                // 注册 FlowEngineState（包装在 Arc<tokio::sync::Mutex> 中）
                let flow_engine_state = std::sync::Arc::new(tokio::sync::Mutex::new(flow_engine));
                app.manage(FlowEngineState(flow_engine_state.clone()));

                // 注册 AuthManagerState 和 ProviderPoolState
                app.manage(AuthManagerState(auth_manager));
                app.manage(ProviderPoolState(provider_pool));

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
                        let stop_result =
                            timeout(Duration::from_secs(5), engine_guard.stop()).await;

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
