mod config;
mod crypto;
mod database;
mod migration;
mod models;
mod services;

use sea_orm::DbConn;
use std::sync::{Arc, Mutex};
use tauri::Manager;

// 全局数据库连接（使用 Arc<Mutex<>> 实现共享）
struct DatabaseState(Arc<Mutex<DbConn>>);

impl DatabaseState {
    fn clone_conn(&self) -> DbConn {
        // DbConn 实现了 Clone，可以直接 clone
        let guard = self.0.lock().unwrap_or_else(|e| {
            tracing::error!("数据库 Mutex 已被污染: {}", e);
            // 如果 Mutex 被污染，尝试恢复
            e.into_inner()
        });
        // DbConn 是 DatabaseConnection 的类型别名，可以直接 clone
        (*guard).clone()
    }
}

// OAuth 服务状态
use std::sync::Mutex as StdMutex;
struct OAuthState(services::oauth_service::OAuthService);

// ============================================================
// 账号管理 Commands
// ============================================================

#[tauri::command]
async fn add_account(
    state: tauri::State<'_, DatabaseState>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = state.clone_conn();
    services::account_service::create(&db, account)
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
    state: tauri::State<'_, DatabaseState>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = state.clone_conn();
    services::account_service::update(&db, id, account)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_account(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<(), String> {
    let db = state.clone_conn();
    services::account_service::delete(&db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn test_account_connection(
    account: models::account::CreateAccountRequest,
) -> Result<services::imap_service::ConnectionTestResult, String> {
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

    let auth = services::imap_service::ImapAuth::Password(account.password);

    services::imap_service::test_connection(&host, port as u16, &account.email, auth)
        .map_err(|e| e.to_string())
}

// ============================================================
// OAuth 2.0 Commands
// ============================================================

#[tauri::command]
async fn get_oauth_auth_url(
    _state: tauri::State<'_, StdMutex<OAuthState>>,
    provider: String,
) -> Result<String, String> {
    // TODO: 实现真实的 OAuth 授权 URL 生成
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
    _state: tauri::State<'_, StdMutex<OAuthState>>,
    provider: String,
    _code: String,
) -> Result<services::oauth_service::OAuthToken, String> {
    match provider.as_str() {
        "microsoft" => {
            // TODO: 实现真实的 token 交换
            tracing::warn!("exchange_oauth_code 使用占位实现");
            Ok(services::oauth_service::OAuthToken {
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

#[tauri::command]
async fn refresh_oauth_token(
    _state: tauri::State<'_, StdMutex<OAuthState>>,
    provider: String,
    _refresh_token: String,
) -> Result<services::oauth_service::OAuthToken, String> {
    match provider.as_str() {
        "microsoft" => {
            // TODO: 实现真实的 token 刷新
            tracing::warn!("refresh_oauth_token 使用占位实现");
            Ok(services::oauth_service::OAuthToken {
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
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<usize, String> {
    let db = state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 解密密码
    let encryptor = crypto::PasswordEncryptor::new()
        .map_err(|e| e.to_string())?;
    let password = encryptor.decrypt(&account.password)
        .map_err(|e| e.to_string())?;

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
    state: tauri::State<'_, DatabaseState>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    let db = state.clone_conn();

    // 获取账号信息
    let account = services::account_service::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    // 解密密码
    let encryptor = crypto::PasswordEncryptor::new()
        .map_err(|e| e.to_string())?;
    let password = encryptor.decrypt(&account.password)
        .map_err(|e| e.to_string())?;

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

    // TODO: 保存到已发送文件夹
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
        // TODO: 配置 Stronghold 插件
        // .plugin(tauri_plugin_stronghold::Builder::new(
        //     |password: &str| {
        //         // 简化的密码哈希函数
        //         use std::collections::hash_map::DefaultHasher;
        //         use std::hash::{Hash, Hasher};
        //         let mut hasher = DefaultHasher::new();
        //         password.hash(&mut hasher);
        //         hasher.finish().to_be_bytes().to_vec()
        //     }
        // ).build())
        .setup(|app| {
            // 应用启动时初始化数据库
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
                app.manage(StdMutex::new(OAuthState(
                    services::oauth_service::OAuthService::new()
                )));

                tracing::info!("Postium Mail 后端初始化完成");
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
