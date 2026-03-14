#![allow(dead_code, ambiguous_glob_reexports)]
mod config;
mod crypto;
mod database;
mod migration;
mod models;
pub mod services;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_keyring::KeyringExt;

// 时间处理

// 全局数据库连接（使用 Arc<Mutex<>> 实现共享）
struct DatabaseState(Arc<Mutex<DbConn>>);

impl DatabaseState {
    fn clone_conn(&self) -> DbConn {
        let guard = self.0.lock().unwrap_or_else(|e| {
            tracing::error!("数据库 Mutex 已被污染: {}", e);
            e.into_inner()
        });
        (*guard).clone()
    }
}

// Keyring 密钥环状态（存储 AppHandle 以便使用 keyring() 方法）
struct KeyringState {
    app_handle: tauri::AppHandle,
}

// OAuth 服务状态
struct OAuthState(services::oauth_service::OAuthService);

// ============================================================
// 账号管理 Commands
// ============================================================

#[tauri::command]
async fn add_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::create(&db, app_handle, account)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_accounts(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    services::account_service::get_all(&db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_account(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<Option<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    match services::account_service::get_by_id(&db, id).await {
        Ok(Some(account)) => Ok(Some(account.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn update_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::update(&db, app_handle, id, account)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
) -> Result<(), String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    services::account_service::delete(&db, app_handle, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_account_connection(
    _keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<services::imap_service::ConnectionTestResult, String> {
    let password = account.password.clone();

    // 获取服务器配置
    let host = account
        .imap_host
        .clone()
        .unwrap_or_else(|| match account.provider.as_str() {
            "gmail" => "imap.gmail.com".to_string(),
            "outlook" | "hotmail" => "outlook.office365.com".to_string(),
            "icloud" => "imap.mail.me.com".to_string(),
            "yahoo" => "imap.mail.yahoo.com".to_string(),
            _ => "imap.example.com".to_string(),
        });

    let port = account.imap_port.unwrap_or(993);

    let auth = services::imap_service::ImapAuth::Password(password);

    services::imap_service::test_connection(&host, port as u16, &account.email, auth)
        .map_err(|e| e.to_string())
}

/// 测试邮箱连接（用于添加账号前验证）
#[tauri::command]
async fn test_email_connection(
    email: String,
    password: String,
    provider: String,
    imap_host: Option<String>,
    imap_port: Option<u16>,
    _imap_ssl: Option<bool>,
    _smtp_host: Option<String>,
    _smtp_port: Option<u16>,
    _smtp_ssl: Option<bool>,
) -> Result<(), String> {
    // 获取服务器配置
    let host = imap_host.unwrap_or_else(|| match provider.as_str() {
        "gmail" => "imap.gmail.com".to_string(),
        "outlook" | "hotmail" => "outlook.office365.com".to_string(),
        "icloud" => "imap.mail.me.com".to_string(),
        "yahoo" => "imap.mail.yahoo.com".to_string(),
        _ => "imap.example.com".to_string(),
    });

    let port = imap_port.unwrap_or(993);

    let auth = services::imap_service::ImapAuth::Password(password);

    services::imap_service::test_connection(&host, port, &email, auth)
        .map_err(|e| format!("IMAP 连接失败: {}", e))?;

    Ok(())
}

// ============================================================
// OAuth 2.0 Commands
// ============================================================

/// 验证 OAuth Token 是否有效
#[tauri::command]
async fn validate_oauth_token(provider: String, token: String) -> Result<bool, String> {
    match provider.as_str() {
        "google" => {
            // 使用 Google UserInfo API 验证
            let client = reqwest::Client::new();
            let response = client
                .get("https://www.googleapis.com/oauth2/v3/userinfo")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;

            Ok(response.status().is_success())
        }
        "microsoft" => {
            // 使用 Microsoft Graph API 验证
            let client = reqwest::Client::new();
            let response = client
                .get("https://graph.microsoft.com/v1.0/me")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;

            Ok(response.status().is_success())
        }
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
async fn get_oauth_auth_url(
    _state: tauri::State<'_, OAuthState>,
    provider: String,
) -> Result<String, String> {
    match provider.as_str() {
        "microsoft" => {
            Ok("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string())
        }
        "google" => Err("Google OAuth 暂未实现，请使用 Microsoft OAuth".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
async fn exchange_oauth_code(
    keyring_state: tauri::State<'_, KeyringState>,
    _state: tauri::State<'_, OAuthState>,
    provider: String,
    email: String,
    _code: String,
) -> Result<models::account::AccountDto, String> {
    match provider.as_str() {
        "microsoft" => {
            tracing::warn!("exchange_oauth_code 使用占位实现");
            let token = crypto::OAuthToken {
                access_token: "placeholder_access_token".to_string(),
                refresh_token: "placeholder_refresh_token".to_string(),
                expires_at: 0,
            };

            // 将 Token 存储到 Keyring（JSON 序列化）
            let temp_account_id = email.len() as i32;
            let username = crypto::oauth_username(temp_account_id);
            let token_json = serde_json::to_string(&token).map_err(|e| e.to_string())?;
            let keyring = keyring_state.app_handle.keyring();
            keyring
                .set_password(crypto::KEYRING_SERVICE, &username, &token_json)
                .map_err(|e| e.to_string())?;

            Ok(models::account::AccountDto {
                id: temp_account_id,
                name: email.clone(),
                email,
                provider: "outlook".to_string(),
                imap_host: Some("outlook.office365.com".to_string()),
                imap_port: Some(993),
                imap_ssl: Some(true),
                smtp_host: Some("smtp-mail.outlook.com".to_string()),
                smtp_port: Some(587),
                smtp_ssl: Some(true),
                color: Some("#0078D4".to_string()),
                sync_enabled: true,
                last_sync_at: None,
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
                auth_type: "oauth".to_string(),
                oauth_provider: Some("microsoft".to_string()),
            })
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
async fn refresh_oauth_token(
    _state: tauri::State<'_, OAuthState>,
    provider: String,
    _refresh_token: String,
) -> Result<crypto::OAuthToken, String> {
    match provider.as_str() {
        "microsoft" => {
            tracing::warn!("refresh_oauth_token 使用占位实现");
            Ok(crypto::OAuthToken {
                access_token: "placeholder_access_token".to_string(),
                refresh_token: "placeholder_refresh_token".to_string(),
                expires_at: 0,
            })
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

// ============================================================
// 邮件操作 Commands
// ============================================================

#[tauri::command]
async fn list_emails(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<services::email_service::EmailListResponse, String> {
    tracing::info!("📨 [list_emails] ========== 邮件列表请求 ==========");
    tracing::info!(
        "📨 [list_emails] 参数: account_id={}, folder='{}', page={}, limit={}",
        account_id,
        folder,
        page,
        limit
    );

    let db = state.clone_conn();

    let result = services::email_service::list(&db, account_id, &folder, page, limit)
        .await
        .map_err(|e| {
            tracing::error!("📨 [list_emails] ❌ 查询失败: {}", e);
            e.to_string()
        })?;

    tracing::info!(
        "📨 [list_emails] ✅ 查询成功: 返回 {} 封邮件，总计 {} 封",
        result.emails.len(),
        result.total
    );

    tracing::info!("📨 [list_emails] ========== 请求结束 ==========");

    Ok(result)
}

#[tauri::command]
async fn get_email(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<models::email::EmailDetail, String> {
    let db = state.clone_conn();
    services::email_service::get_detail(&db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_emails_fts(
    state: tauri::State<'_, DatabaseState>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<services::search_service::SearchResult>, String> {
    let db = state.clone_conn();
    services::search_service::SearchService::search_emails(&db, account_id, &query, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn mark_as_read(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    is_read: bool,
) -> Result<(), String> {
    let db = state.clone_conn();
    services::email_service::update_read_status(&db, email_id, is_read)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_star(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
) -> Result<bool, String> {
    let db = state.clone_conn();
    services::email_service::toggle_star(&db, email_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_emails(
    state: tauri::State<'_, DatabaseState>,
    email_ids: Vec<i32>,
) -> Result<usize, String> {
    let db = state.clone_conn();
    services::email_service::batch_delete(&db, email_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn move_email_to_folder(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    folder: String,
) -> Result<(), String> {
    let db = state.clone_conn();
    services::email_service::move_to_folder(&db, email_id, &folder)
        .await
        .map_err(|e| e.to_string())
}

/// 获取账号的文件夹统计数据
#[tauri::command]
async fn get_folder_stats(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<Vec<models::folder::FolderDto>, String> {
    let db = state.clone_conn();
    let folders = services::folder_service::get_by_account(&db, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folders.into_iter().map(|f| f.into()).collect())
}

// ============================================================
// 邮件同步 Commands
// ============================================================

/// 带进度的账号同步命令（使用 SyncManager）
#[tauri::command]
async fn sync_account_with_progress(
    db_state: tauri::State<'_, DatabaseState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), String> {
    let db = std::sync::Arc::new(db_state.clone_conn());

    // 创建 SyncManager
    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle.clone());

    // 执行同步
    let result = sync_manager
        .sync_account(account_id)
        .await
        .map_err(|e| format!("同步失败: {}", e))?;

    tracing::info!(
        "同步完成: 账号 {}, 同步了 {} 封邮件, {} 个文件夹, {} 个错误, 耗时 {}ms",
        account_id,
        result.total_synced,
        result.folders_synced,
        result.errors,
        result.duration_ms
    );

    Ok(())
}

/// 同步账号（简化版，不发送进度事件）
#[tauri::command]
async fn sync_account(
    db_state: tauri::State<'_, DatabaseState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<usize, String> {
    let db = std::sync::Arc::new(db_state.clone_conn());

    // 创建 SyncManager
    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle);

    // 执行同步
    let result = sync_manager
        .sync_account(account_id)
        .await
        .map_err(|e| format!("同步失败: {}", e))?;

    Ok(result.total_synced)
}

#[tauri::command]
async fn send_email(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    let db = db_state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 从 Keyring 获取密码
    let username = crypto::password_username(request.account_id);
    let keyring = keyring_state.app_handle.keyring();
    let password = keyring
        .get_password(crypto::KEYRING_SERVICE, &username)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "密码未找到".to_string())?;

    // 连接 SMTP 并发送
    let mut smtp_service = services::smtp_service::SmtpService::new();

    let host = account.smtp_host.unwrap_or_else(|| {
        match account.provider.as_str() {
            "gmail" => "smtp.gmail.com",
            "outlook" | "hotmail" => "smtp-mail.outlook.com",
            "icloud" => "smtp.mail.me.com",
            "yahoo" => "smtp.mail.yahoo.com",
            _ => "smtp.example.com",
        }
        .to_string()
    });

    let port = account.smtp_port.unwrap_or(587) as u16;

    smtp_service
        .connect(
            &host,
            port,
            &account.email,
            services::smtp_service::SmtpAuth::Password(password),
        )
        .map_err(|e| e.to_string())?;

    let to_addresses: Vec<String> = request.to.iter().map(|a| a.email.clone()).collect();

    let message_id = smtp_service
        .send_email(
            &account.email,
            to_addresses,
            &request.subject,
            &request.body_html,
            request.body_text.as_deref(),
        )
        .map_err(|e| e.to_string())?;

    tracing::info!("邮件已发送: {}", message_id);

    Ok(message_id)
}

// ============================================================
// 旧的测试命令
// ============================================================

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// ============================================================
// 应用入口
// ============================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Keyring 插件（系统原生密钥链）
        .plugin(tauri_plugin_keyring::init())
        .setup(|app| {
            // 应用启动时初始化数据库和 Keyring
            tauri::async_runtime::block_on(async move {
                // 建立数据库连接
                let db = database::establish_connection()
                    .await
                    .expect("无法连接到数据库");

                // 初始化数据库表
                database::init_database(&db)
                    .await
                    .expect("数据库初始化失败");

                // 将数据库连接存储到应用状态中
                app.manage(DatabaseState(Arc::new(Mutex::new(db))));

                // 初始化 OAuth 服务
                app.manage(OAuthState(services::oauth_service::OAuthService::new()));

                // 存储 AppHandle 到 KeyringState，用于后续访问 keyring
                app.manage(KeyringState { app_handle: app.handle().clone() });

                tracing::info!("Postium Mail 后端初始化完成");
                tracing::info!("密码存储: 操作系统原生密钥链 (Tauri Plugin Keyring)");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 账号管理
            add_account,
            list_accounts,
            get_account,
            update_account,
            delete_account,
            test_account_connection,
            test_email_connection,
            // OAuth 2.0
            validate_oauth_token,
            get_oauth_auth_url,
            exchange_oauth_code,
            refresh_oauth_token,
            // 邮件操作
            list_emails,
            get_email,
            search_emails_fts,
            mark_as_read,
            toggle_star,
            delete_emails,
            move_email_to_folder,
            get_folder_stats,
            // 邮件同步
            sync_account,
            sync_account_with_progress,
            send_email,
            // 测试
            greet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
