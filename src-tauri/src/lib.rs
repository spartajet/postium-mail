mod config;
mod crypto;
mod database;
mod models;
mod services;

use sea_orm::DbConn;
use std::sync::Mutex;
use tauri::Manager;

// 全局数据库连接（简化实现，生产环境应使用连接池）
struct DatabaseState(DbConn);

// 辅助函数：获取数据库引用（避免 MutexGuard 跨 await）
fn with_db<F, R>(
    state: &Mutex<DatabaseState>,
    f: F,
) -> R
where
    F: FnOnce(&DbConn) -> R,
{
    let db = state.lock().unwrap();
    f(&db.0)
}

// ============================================================
// 账号管理 Commands
// ============================================================

#[tauri::command]
async fn add_account(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::account_service::create(db, account)
                .await
                .map(|a| a.into())
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn list_accounts(
    state: tauri::State<'_, Mutex<DatabaseState>>,
) -> Result<Vec<models::account::AccountDto>, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::account_service::get_all(db)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn get_account(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    id: i32,
) -> Result<Option<models::account::AccountDto>, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            match services::account_service::get_by_id(db, id).await {
                Ok(Some(account)) => Ok(Some(account.into())),
                Ok(None) => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        })
    })
}

#[tauri::command]
async fn update_account(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::account_service::update(db, id, account)
                .await
                .map(|a| a.into())
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn delete_account(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    id: i32,
) -> Result<(), String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::account_service::delete(db, id)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn test_account_connection(
    _account: models::account::CreateAccountRequest,
) -> Result<bool, String> {
    // TODO: 实现真实的 IMAP 连接测试
    // 目前暂时返回 true
    Ok(true)
}

// ============================================================
// 邮件操作 Commands
// ============================================================

#[tauri::command]
async fn list_emails(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<services::email_service::EmailListResponse, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::list(db, account_id, &folder, page, limit)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn get_email(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    id: i32,
) -> Result<models::email::EmailDetail, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::get_detail(db, id)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn search_emails_fts(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<services::search_service::SearchResult>, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::search_service::SearchService::search_emails(
                db,
                account_id,
                &query,
                limit,
            )
            .await
            .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn mark_as_read(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    email_id: i32,
    is_read: bool,
) -> Result<(), String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::update_read_status(db, email_id, is_read)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn toggle_star(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    email_id: i32,
) -> Result<bool, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::toggle_star(db, email_id)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn delete_emails(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    email_ids: Vec<i32>,
) -> Result<usize, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::batch_delete(db, email_ids)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

#[tauri::command]
async fn move_email_to_folder(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    email_id: i32,
    folder: String,
) -> Result<(), String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async {
            services::email_service::move_to_folder(db, email_id, &folder)
                .await
                .map_err(|e| e.to_string())
        })
    })
}

// ============================================================
// 邮件同步 Commands
// ============================================================

#[tauri::command]
async fn sync_account(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    account_id: i32,
) -> Result<usize, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async move {
            // 获取账号信息
            let account = services::account_service::get_by_id(db, account_id)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "账号不存在".to_string())?;

            // 解密密码
            let encryptor = crypto::PasswordEncryptor::new()
                .map_err(|e| e.to_string())?;
            let password = encryptor.decrypt(&account.password)
                .map_err(|e| e.to_string())?;

            // 连接 IMAP 并同步
            let mut imap_service = services::imap_service::ImapService::new();

            let host = account.imap_host.unwrap_or_else(|| {
                match account.provider.as_str() {
                    "gmail" => "imap.gmail.com",
                    "outlook" | "hotmail" => "outlook.office365.com",
                    "icloud" => "imap.mail.me.com",
                    "yahoo" => "imap.mail.yahoo.com",
                    _ => "imap.example.com",
                }.to_string()
            });

            let port = account.imap_port.unwrap_or(993) as u16;
            let use_ssl = account.imap_ssl.unwrap_or(true);

            imap_service.connect(&host, port, use_ssl)
                .await
                .map_err(|e| e.to_string())?;

            imap_service.login(&account.email, &password)
                .await
                .map_err(|e| e.to_string())?;

            // 同步收件箱
            let count = imap_service.sync_folder(account_id, db, "INBOX")
                .await
                .map_err(|e| e.to_string())?;

            imap_service.logout()
                .await
                .map_err(|e| e.to_string())?;

            // 更新最后同步时间
            services::account_service::update_last_sync(db, account_id)
                .await
                .map_err(|e| e.to_string())?;

            Ok(count)
        })
    })
}

#[tauri::command]
async fn send_email(
    state: tauri::State<'_, Mutex<DatabaseState>>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    with_db(&state, |db| {
        tauri::async_runtime::block_on(async move {
            // 获取账号信息
            let account = services::account_service::get_by_id(db, request.account_id)
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

            smtp_service.connect_with_credentials(&host, port, &account.email, &password)
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
        })
    })
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
                app.manage(Mutex::new(DatabaseState(db)));

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
