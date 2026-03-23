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
pub use engine::FlowEngine;
pub use error::{MailError, Result};
pub use providers::{AccountType, MailProvider, OAuthConfig, ProviderPool};
pub use storage::database::init_database;
pub use sync::SyncManager; // 用于测试

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Listener, Manager};

use command::{
    AuthManagerState, DatabaseState, FlowEngineState, KeyringState, OAuthFlowResult,
    OAuthSessionManagerState, ProviderPoolState,
};

/// 处理 OAuth HTTP 回调
fn handle_oauth_http_callback(
    app_handle: tauri::AppHandle,
    code: String,
    state: String,
    error: Option<String>,
    error_description: Option<String>,
) {
    // 显示主窗口并聚焦
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_focus();
        let _ = window.unminimize();
        let _ = window.show();
    }

    // 获取状态并克隆 Arc（确保 async block 中拥有所有权）
    let session_manager = std::sync::Arc::clone(&app_handle.state::<OAuthSessionManagerState>().0);
    let auth_manager = app_handle.state::<AuthManagerState>().clone_manager();
    // 从 State<'_, DatabaseState> 中提取并克隆内部的 Arc
    let db_arc = std::sync::Arc::clone(&app_handle.state::<DatabaseState>().0);
    let db_state = command::DatabaseState(db_arc);

    // KeyringState 包含 AppHandle，可以直接克隆
    let keyring_state = command::KeyringState {
        app_handle: app_handle.clone(),
    };

    // 在异步运行时中处理
    tauri::async_runtime::spawn(async move {
        // 验证会话
        let session = match session_manager.verify_and_get_session(&state).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("验证 OAuth 会话失败: {}", e);
                // 发射错误事件
                let _ = app_handle.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id: String::new(),
                        status: "error".to_string(),
                        account: None,
                        error: Some(format!("无效的会话: {}", e)),
                    },
                );
                return;
            }
        };

        let session_id = session.session_id.clone();

        // 如果有错误，标记会话失败
        if let Some(err) = error {
            tracing::warn!("OAuth 授权失败: {}", err);
            let error_msg = error_description.as_deref().unwrap_or(&err);
            let _ = session_manager
                .set_session_error(&session_id, error_msg.to_string())
                .await;

            let _ = app_handle.emit(
                "oauth-flow-complete",
                OAuthFlowResult {
                    session_id: session_id.clone(),
                    status: "error".to_string(),
                    account: None,
                    error: error_description.or(Some(err)),
                },
            );
            return;
        }

        // 交换 token 并创建账号
        let email = session.email.clone();

        // 使用现有的 exchange_oauth_code 逻辑
        match exchange_and_create_account(
            db_state,
            keyring_state,
            auth_manager,
            email.clone(),
            code,
            state,
        )
        .await
        {
            Ok(account) => {
                tracing::info!("OAuth 流程成功创建账号: {}", account.email);

                // 更新会话状态
                let _ = session_manager
                    .set_session_account_id(&session_id, account.id)
                    .await;

                // 发射成功事件
                let _ = app_handle.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id,
                        status: "success".to_string(),
                        account: Some(account),
                        error: None,
                    },
                );
            }
            Err(e) => {
                tracing::error!("OAuth 流程失败: {}", e);

                // 更新会话状态
                let _ = session_manager
                    .set_session_error(&session_id, e.clone())
                    .await;

                // 发射错误事件
                let _ = app_handle.emit(
                    "oauth-flow-complete",
                    OAuthFlowResult {
                        session_id,
                        status: "error".to_string(),
                        account: None,
                        error: Some(e),
                    },
                );
            }
        }
    });
}

/// 交换 OAuth 授权码并创建账号
async fn exchange_and_create_account(
    db_state: command::DatabaseState,
    keyring_state: command::KeyringState,
    auth_manager: std::sync::Arc<crate::auth::AuthManager>,
    email: String,
    code: String,
    state: String,
) -> std::result::Result<crate::storage::AccountDto, String> {
    use crate::providers;

    // 打印接收到的参数（用于调试）
    tracing::info!("========== 交换 OAuth Token ==========");
    tracing::info!("Email: {}", email);
    tracing::info!(
        "Code (前20字符): {}",
        &code.chars().take(20).collect::<String>()
    );
    tracing::info!("Code 长度: {}", code.len());
    tracing::info!("State: {}", state);
    tracing::info!("====================================");

    let db = db_state.clone_conn();

    // 使用 AuthManager 进行 OAuth 认证
    let auth_result = auth_manager
        .authenticate_oauth(&email, &code, &state)
        .await
        .map_err(|e| e.to_string())?;

    // 获取服务商配置
    let provider_pool = auth_manager.provider_pool();
    let provider = provider_pool
        .detect_provider(&email)
        .await
        .map_err(|e| e.to_string())?;

    let imap_config = provider.imap_config(&email);
    let smtp_config = provider.smtp_config(&email);

    // 构建账号创建请求
    let account_req = crate::storage::CreateAccountRequest {
        name: auth_result
            .display_name
            .unwrap_or_else(|| email.split('@').next().unwrap_or("用户").to_string()),
        email: auth_result.email.clone(),
        provider: provider.provider_id().to_string(),
        password: String::new(),
        imap_host: Some(imap_config.host),
        imap_port: Some(imap_config.port as i32),
        imap_ssl: Some(matches!(
            imap_config.ssl,
            providers::SslMode::Implicit | providers::SslMode::StartTls
        )),
        smtp_host: Some(smtp_config.host),
        smtp_port: Some(smtp_config.port as i32),
        smtp_ssl: Some(matches!(smtp_config.ssl, providers::SslMode::StartTls)),
        color: Some("#0078D4".to_string()),
        auth_type: Some("oauth2".to_string()),
        oauth_provider: Some(provider.provider_id().to_string()),
        oauth_token: auth_result.id_token,
        oauth_refresh_token: Some(String::new()),
        oauth_expires_at: auth_result.expires_at,
    };

    // 创建账号
    let account =
        crate::storage::AccountRepository::create(&db, &keyring_state.app_handle, account_req)
            .await
            .map_err(|e| e.to_string())?;

    // 迁移 Token
    let token_manager = auth_manager.token_manager();
    token_manager
        .migrate_token_account(0, account.id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(account.into())
}

/// 初始化 tracing 日志系统
///
/// 使用环境变量 `RUST_LOG` 控制日志级别，例如：
/// - `RUST_LOG=info` - 只显示 INFO 及以上级别
/// - `RUST_LOG=debug` - 显示 DEBUG 及以上级别（开发调试用）
/// - `RUST_LOG=postium_mail=trace` - 只对本模块使用 TRACE 级别
fn init_tracing() {
    // 配置日志过滤器，屏蔽第三方 crate 的冗余日志
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("debug")
                // 过滤 keyring 相关 crate 的日志
                .add_directive("keyring=error".parse().unwrap())
                .add_directive("tauri_plugin_keyring=error".parse().unwrap())
                .add_directive("secret_service=error".parse().unwrap())
                .add_directive("windows=error".parse().unwrap())
                .add_directive("windows_sys=error".parse().unwrap())
                .add_directive("tokio_native_tls=warn".parse().unwrap())
                .add_directive("native_tls=warn".parse().unwrap())
                .add_directive("async_imap=warn".parse().unwrap())
        });

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
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

            // 如果有 URL 参数，可能是其他应用尝试打开链接
            if !args.is_empty() {
                tracing::info!("单实例收到参数: {:?}", args);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
                let _ = window.unminimize();
                let _ = window.show();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
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

            // ========== OAuth HTTP 回调事件监听器 ==========
            let app_handle_for_callback = app.handle().clone();
            app.listen("oauth-http-callback", move |event| {
                let payload = event.payload();
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(payload) {
                    let code = data["code"].as_str().unwrap_or("").to_string();
                    let state = data["state"].as_str().unwrap_or("").to_string();
                    let error = data["error"].as_str().map(|s| s.to_string());
                    let error_description =
                        data["error_description"].as_str().map(|s| s.to_string());

                    handle_oauth_http_callback(
                        app_handle_for_callback.clone(),
                        code,
                        state,
                        error,
                        error_description,
                    );
                }
            });

            // ========== 数据库和服务初始化 ==========
            tauri::async_runtime::block_on(async move {
                use sea_orm::DbConn;
                let db: DbConn = storage::database::establish_connection()
                    .await
                    .expect("无法连接到数据库");

                storage::database::init_database(&db)
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

                // 启动 OAuth HTTP 服务器（在异步运行时中）
                {
                    let http_server = auth_manager.get_http_server();
                    if let Err(e) = http_server.start().await {
                        tracing::error!("OAuth HTTP 服务器启动失败: {}", e);
                    } else {
                        tracing::info!("OAuth HTTP 服务器启动成功");
                    }
                }

                // 创建 ProviderPool（注册所有默认服务商）
                let provider_pool = std::sync::Arc::new(providers::ProviderPool::default());

                // 获取 SessionManager 的引用（在 move auth_manager 之前）
                let session_manager = auth_manager.session_manager();

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
                let auth_manager_for_cleanup = auth_manager.clone();
                app.manage(AuthManagerState(auth_manager));
                app.manage(ProviderPoolState(provider_pool));
                // 注册 OAuthSessionManagerState（使用之前克隆的 session_manager）
                app.manage(command::OAuthSessionManagerState(session_manager));

                // 启动 FlowEngine
                {
                    let engine = flow_engine_state.lock().await;
                    if let Err(e) = engine.start().await {
                        tracing::error!("FlowEngine 启动失败: {}", e);
                    } else {
                        tracing::info!("FlowEngine 已启动");
                    }
                }

                // ========== FlowEngine 和 AuthManager 优雅关闭 ==========
                // 监听应用退出事件，停止 FlowEngine 和 AuthManager
                let _ = app.listen("tauri://destroy", move |_| {
                    let engine_state = flow_engine_state.clone();
                    let auth_manager = auth_manager_for_cleanup.clone();
                    tauri::async_runtime::block_on(async move {
                        use tokio::time::{timeout, Duration};

                        // 停止 FlowEngine
                        tracing::info!("正在停止 FlowEngine...");
                        let engine_guard = engine_state.lock().await;
                        let stop_result =
                            timeout(Duration::from_secs(5), engine_guard.stop()).await;

                        match stop_result {
                            Ok(Ok(())) => tracing::info!("FlowEngine 已停止"),
                            Ok(Err(e)) => tracing::error!("FlowEngine 停止失败: {}", e),
                            Err(_) => tracing::warn!("FlowEngine 停止超时，将强制退出"),
                        }
                        drop(engine_guard); // 释放锁

                        // 停止 AuthManager (包括 OAuth HTTP 服务器)
                        tracing::info!("正在停止 AuthManager...");
                        auth_manager.shutdown().await;
                    });
                });

                tracing::info!("Postium Mail 后端初始化完成");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号管理
            // command::add_account,
            command::list_accounts,
            command::get_account,
            // command::update_account,
            command::delete_account,
            // command::test_account_connection,
            // command::test_email_connection,
            // 服务商检测
            command::detect_provider,
            command::list_providers,
            // OAuth
            command::validate_oauth_token,
            command::get_oauth_auth_url,
            command::exchange_oauth_code,
            command::refresh_oauth_token,
            command::start_oauth_flow,
            command::cancel_oauth_flow,
            // 统一认证
            command::start_auth_command,
            // 邮件操作
            command::list_emails,
            command::get_email,
            command::search_emails_fts,
            command::mark_as_read,
            command::toggle_star,
            command::delete_emails,
            command::move_email_to_folder,
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
