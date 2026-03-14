mod config;
mod crypto;
mod database;
mod migration;
mod models;
mod services;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex};
use tauri::Manager;

// 时间处理
use chrono;

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

// Stronghold Vault 状态
struct VaultState(crypto::SecureVault);

// OAuth 服务状态
struct OAuthState(services::oauth_service::OAuthService);

// ============================================================
// 账号管理 Commands
// ============================================================

#[tauri::command]
async fn add_account(
    db_state: tauri::State<'_, DatabaseState>,
    vault_state: tauri::State<'_, VaultState>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();

    // 使用 Stronghold 存储密码
    let password_key = format!("account_password_{}", account.email);
    vault_state.0.store_password(&password_key, &account.password).await
        .map_err(|e| e.to_string())?;

    // 保存账号信息到数据库（不包含明文密码）
    let account_without_password = models::account::CreateAccountRequest {
        password: String::new(),
        ..account
    };

    services::account_service::create(&db, account_without_password)
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
    vault_state: tauri::State<'_, VaultState>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();

    // 如果提供了新密码，更新到 Stronghold
    if !account.password.is_empty() {
        let password_key = format!("account_password_{}", account.email);
        vault_state.0.store_password(&password_key, &account.password).await
            .map_err(|e| e.to_string())?;
    }

    // 更新账号信息到数据库
    let account_without_password = models::account::CreateAccountRequest {
        password: String::new(),
        ..account
    };

    services::account_service::update(&db, id, account_without_password)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_account(
    db_state: tauri::State<'_, DatabaseState>,
    vault_state: tauri::State<'_, VaultState>,
    id: i32,
) -> Result<(), String> {
    let db = db_state.clone_conn();

    // 获取账号邮箱以删除密码
    if let Ok(Some(account)) = services::account_service::get_by_id(&db, id).await {
        let password_key = format!("account_password_{}", account.email);
        let _ = vault_state.0.remove_password(&password_key).await;
    }

    services::account_service::delete(&db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_account_connection(
    vault_state: tauri::State<'_, VaultState>,
    account: models::account::CreateAccountRequest,
) -> Result<services::imap_service::ConnectionTestResult, String> {
    // 从 Stronghold 获取密码
    let password_key = format!("account_password_{}", account.email);
    let password = if account.password.is_empty() {
        vault_state.0.get_password(&password_key).await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "密码未找到".to_string())?
    } else {
        account.password.clone()
    };

    // 获取服务器配置
    let host = account.imap_host.clone().unwrap_or_else(|| {
        match account.provider.as_str() {
            "gmail" => "imap.gmail.com".to_string(),
            "outlook" | "hotmail" => "outlook.office365.com".to_string(),
            "icloud" => "imap.mail.me.com".to_string(),
            "yahoo" => "imap.mail.yahoo.com".to_string(),
            _ => "imap.example.com".to_string(),
        }
    });

    let port = account.imap_port.unwrap_or(993);

    let auth = services::imap_service::ImapAuth::Password(password);

    services::imap_service::test_connection(&host, port as u16, &account.email, auth)
        .map_err(|e| e.to_string())
}

// ============================================================
// OAuth 2.0 Commands
// ============================================================

#[tauri::command]
async fn get_oauth_auth_url(
    _state: tauri::State<'_, OAuthState>,
    provider: String,
) -> Result<String, String> {
    match provider.as_str() {
        "microsoft" => {
            Ok("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string())
        }
        "google" => {
            Err("Google OAuth 暂未实现，请使用 Microsoft OAuth".to_string())
        }
        _ => {
            Err(format!("不支持的 OAuth 提供商: {}", provider))
        }
    }
}

#[tauri::command]
async fn exchange_oauth_code(
    vault_state: tauri::State<'_, VaultState>,
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

            // 将 Token 存储到 Stronghold
            let temp_account_id = email.len() as i32;
            vault_state.0.store_token(temp_account_id, &token).await
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
        "google" => {
            Err("Google OAuth 暂未实现".to_string())
        }
        _ => {
            Err(format!("不支持的 OAuth 提供商: {}", provider))
        }
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
        "google" => {
            Err("Google OAuth 暂未实现".to_string())
        }
        _ => {
            Err(format!("不支持的 OAuth 提供商: {}", provider))
        }
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
    let db = state.clone_conn();
    services::email_service::list(&db, account_id, &folder, page, limit)
        .await
        .map_err(|e| e.to_string())
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
    services::search_service::SearchService::search_emails(
        &db,
        account_id,
        &query,
        limit,
    )
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

// ============================================================
// 邮件同步 Commands
// ============================================================

#[tauri::command]
async fn sync_account(
    db_state: tauri::State<'_, DatabaseState>,
    vault_state: tauri::State<'_, VaultState>,
    account_id: i32,
) -> Result<usize, String> {
    let db = db_state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 从 Stronghold 获取密码
    let password_key = format!("account_password_{}", account.email);
    let _password = vault_state.0.get_password(&password_key).await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "密码未找到".to_string())?;

    // 连接 IMAP 并同步（暂时使用占位实现）
    let mut imap_service = services::imap_service::ImapService::new();

    // 同步收件箱
    let count = imap_service.sync_folder(account_id, &db, "INBOX")
        .map_err(|e| e.to_string())?;

    imap_service.logout()
        .map_err(|e| e.to_string())?;

    // 更新最后同步时间
    services::account_service::update_last_sync(&db, account_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(count)
}

#[tauri::command]
async fn send_email(
    db_state: tauri::State<'_, DatabaseState>,
    vault_state: tauri::State<'_, VaultState>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    let db = db_state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 从 Stronghold 获取密码
    let password_key = format!("account_password_{}", account.email);
    let password = vault_state.0.get_password(&password_key).await
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
        }.to_string()
    });

    let port = account.smtp_port.unwrap_or(587) as u16;

    smtp_service.connect(&host, port, &account.email, services::smtp_service::SmtpAuth::Password(password))
        .map_err(|e| e.to_string())?;

    let to_addresses: Vec<String> = request.to.iter()
        .map(|a| a.email.clone())
        .collect();

    let message_id = smtp_service.send_email(
        &account.email,
        to_addresses,
        &request.subject,
        &request.body_html,
        request.body_text.as_deref(),
    ).map_err(|e| e.to_string())?;

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
        // 配置 Stronghold 插件（仍然配置以支持其他功能）
        .plugin(tauri_plugin_stronghold::Builder::new(|password: &str| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            hasher.update(b"postium-mail-salt");
            hasher.finalize().to_vec()
        }).build())
        .setup(|app| {
            // 应用启动时初始化数据库和 Stronghold
            tauri::async_runtime::block_on(async move {
                // 建立 Stronghold vault 目录
                let vault_path = crypto::get_vault_path()
                    .expect("无法获取 vault 路径");
                tracing::info!("Stronghold vault: {}", vault_path);

                // 初始化 SecureVault（使用 iota_stronghold）
                let vault = crypto::SecureVault::new()
                    .await
                    .expect("SecureVault 初始化失败");
                app.manage(VaultState(vault));

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
                app.manage(OAuthState(
                    services::oauth_service::OAuthService::new()
                ));

                tracing::info!("Postium Mail 后端初始化完成");
                tracing::info!("密码存储: AES-256-GCM 加密文件 (vault.enc)");
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
            // OAuth 2.0
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
            // 邮件同步
            sync_account,
            send_email,
            // 测试
            greet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
