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
//! - 增量同步（UID 搜索）
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
//! - **增量同步**: 基于 UID 搜索的高效同步
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

#![allow(ambiguous_glob_reexports, unused_variables)]
mod command;
// 新增模块
pub mod auth; // 公开以支持测试
pub mod engine;
pub mod error;
pub mod protocols; // 协议层（IMAP/SMTP）
pub mod providers; // 公开以支持测试
pub mod storage; // 存储层
pub mod sync; // 公开以支持测试
pub mod sys;

// 重新导出关键类型
pub use auth::AuthManager;
// pub use engine::FlowEngine;
pub use error::{MailError, Result};
pub use providers::{AccountType, MailProvider, OAuthConfig, ProviderPool};
pub use storage::database::init_database;
pub use sync::SyncManager; // 用于测试

use tauri::{Listener, Manager};

use command::{AuthManagerState, DatabaseState, KeyringState, ProviderPoolState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志系统
    sys::log::init_tracing();

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
            sys::tray::init_tray(app.handle())?;

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

                // 注册 AuthManagerState 和 ProviderPoolState
                let auth_manager_for_cleanup = auth_manager.clone();
                app.manage(AuthManagerState(auth_manager));
                app.manage(ProviderPoolState(provider_pool));
                // 注册 OAuthSessionManagerState（使用之前克隆的 session_manager）
                app.manage(command::OAuthSessionManagerState(session_manager));

                // ========== FlowEngine 和 AuthManager 优雅关闭 ==========
                // 监听应用退出事件，停止 FlowEngine 和 AuthManager
                let _ = app.listen("tauri://destroy", move |_| {
                    // let engine_state = flow_engine_state.clone();
                    let auth_manager = auth_manager_for_cleanup.clone();
                    tauri::async_runtime::block_on(async move {
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
            command::list_accounts,
            command::get_account,
            command::delete_account,
            // 服务商检测
            command::detect_provider,
            command::list_providers,
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
            command::get_folder_stats,
            // 同步
            command::sync_account,
            command::sync_account_with_progress,
            command::send_email,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
