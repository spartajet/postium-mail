//! 邮件同步 Commands
//!
//! 提供邮件同步和发送功能，包括：
//! - 带进度的邮件同步
//! - 简单邮件同步
//! - 发送邮件
//!
//! # 同步流程
//!
//! 1. 连接到 IMAP 服务器
//! 2. 同步文件夹列表
//! 3. 对每个文件夹：
//!    - 检测变更（使用 UID 搜索）
//!    - 下载新邮件
//!    - 更新邮件标志
//!    - 删除已移除的邮件
//!
//! # 同步事件
//!
//! 同步过程中会发送 `sync-progress-{accountId}` 事件，包含：
//! - `stage`: 当前阶段（连接中、同步文件夹、同步邮件等）
//! - `folder`: 当前处理的文件夹
//! - `current`: 当前进度
//! - `total`: 总数
//! - `message`: 状态消息

use std::sync::Arc;
use tauri_plugin_keyring::KeyringExt;

use super::{AuthManagerState, DatabaseState, KeyringState, ProviderPoolState};

use crate::auth::KEYRING_SERVICE;
use crate::auth::password_username;
use crate::protocols::smtp;
use crate::storage;
use crate::sync;

/// 同步账号邮件（带进度事件）
///
/// 执行完整的邮件同步，并通过事件实时报告同步进度。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `auth_manager_state` - AuthManager 状态，用于获取认证信息
/// * `provider_pool_state` - ProviderPool 状态，用于获取服务商配置
/// * `_keyring_state` - 密钥链状态（当前未使用，保留用于将来扩展）
/// * `app_handle` - Tauri 应用句柄，用于发送进度事件
/// * `account_id` - 要同步的账号 ID
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 进度事件
///
/// 同步过程中会发送 `sync-progress-{accountId}` 事件：
/// ```json
/// {
///   "stage": "SyncingFolders",  // Connecting, SyncingFolders, SyncingEmails, Completed, Error
///   "folder": "INBOX",
///   "current": 10,
///   "total": 100,
///   "message": "正在同步邮件..."
/// }
/// ```
///
/// # 同步结果
///
/// 完成后会在日志中输出统计信息：
/// - 同步的邮件总数
/// - 同步的文件夹数
/// - 错误数量
/// - 耗时（毫秒）
///
/// # 示例
/// ```rust,no_run,ignore
/// use crate::command::sync::sync_account_with_progress;
///
/// // 前端监听进度事件
/// listen(`sync-progress-${accountId}`, (event) => {
///   console.log(`进度: ${event.payload.current}/${event.payload.total}`);
/// });
///
/// // 触发同步
/// sync_account_with_progress(db_state, auth_state, provider_state, keyring_state, app_handle, 1).await?;
/// ```
#[tauri::command]
pub async fn sync_account_with_progress(
    db_state: tauri::State<'_, DatabaseState>,
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    provider_pool_state: tauri::State<'_, ProviderPoolState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), String> {
    let db = Arc::new(db_state.clone_conn());
    let auth_manager = auth_manager_state.clone_manager();
    let provider_pool = provider_pool_state.clone_pool();

    let sync_manager = sync::SyncManager::new(db, app_handle.clone(), auth_manager, provider_pool);

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

/// 同步账号邮件（简化版）
///
/// 执行邮件同步并返回同步的邮件总数，不发送进度事件。
/// 适用于不需要实时进度反馈的场景。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `auth_manager_state` - AuthManager 状态
/// * `provider_pool_state` - ProviderPool 状态
/// * `_keyring_state` - 密钥链状态（当前未使用）
/// * `app_handle` - Tauri 应用句柄
/// * `account_id` - 要同步的账号 ID
///
/// # 返回
/// 成功时返回同步的邮件总数，失败时返回错误信息字符串
///
/// # 与 sync_account_with_progress 的区别
///
/// | 特性 | sync_account | sync_account_with_progress |
/// |------|--------------|---------------------------|
/// | 进度事件 | 无 | 有 |
/// | 返回值 | 邮件总数 | 空值 |
/// | 适用场景 | 后台同步 | 用户触发的同步 |
///
/// # 示例
/// ```rust,no_run
/// use crate::command::sync::sync_account;
///
/// let total = sync_account(db_state, auth_state, provider_state, keyring_state, app_handle, 1).await?;
/// println!("同步了 {} 封邮件", total);
/// ```
#[tauri::command]
pub async fn sync_account(
    db_state: tauri::State<'_, DatabaseState>,
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    provider_pool_state: tauri::State<'_, ProviderPoolState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<usize, String> {
    let db = Arc::new(db_state.clone_conn());
    let auth_manager = auth_manager_state.clone_manager();
    let provider_pool = provider_pool_state.clone_pool();

    let sync_manager = sync::SyncManager::new(db, app_handle, auth_manager, provider_pool);

    let result = sync_manager
        .sync_account(account_id)
        .await
        .map_err(|e| format!("同步失败: {}", e))?;

    Ok(result.total_synced)
}

/// 发送邮件
///
/// 通过 SMTP 协议发送邮件。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `keyring_state` - 密钥链状态，用于获取 SMTP 密码
/// * `request` - 发送邮件请求，包含：
///   - `account_id`: 发件账号 ID
///   - `to`: 收件人列表
///   - `cc`: 抄送列表（可选）
///   - `bcc`: 密送列表（可选）
///   - `subject`: 邮件主题
///   - `body_text`: 纯文本正文
///   - `body_html`: HTML 正文（可选）
///   - `attachments`: 附件列表（可选）
///
/// # 返回
/// 成功时返回服务器分配的邮件 ID（Message-ID），失败时返回错误信息字符串
///
/// # SMTP 配置
///
/// 如果账号未配置 SMTP 服务器，根据 `provider` 自动选择：
/// - `gmail` → smtp.gmail.com:587 (STARTTLS)
/// - `outlook` / `hotmail` → smtp-mail.outlook.com:587
/// - `icloud` → smtp.mail.me.com:587
/// - `yahoo` → smtp.mail.yahoo.com:587
/// - 其他 → smtp.example.com:587
///
/// # 发送流程
///
/// 1. 从数据库获取账号配置
/// 2. 从密钥链获取 SMTP 密码
/// 3. 连接到 SMTP 服务器
/// 4. 构建邮件（发件人、收件人、正文等）
/// 5. 发送邮件
/// 6. 返回邮件 ID
///
/// # 示例
/// ```rust,no_run
/// use crate::command::sync::send_email;
/// use crate::models::email::{SendEmailRequest, EmailAddress};
/// use std::default::Default;
///
/// let request = SendEmailRequest {
///     account_id: 1,
///     to: vec![EmailAddress { email: "recipient@example.com".to_string(), name: None }],
///     subject: "测试邮件".to_string(),
///     body_text: "这是一封测试邮件。".to_string(),
///     body_html: Some("<p>这是一封测试邮件。</p>".to_string()),
///     ..Default::default()
/// };
///
/// let message_id = send_email(db_state, keyring_state, request).await?;
/// println!("邮件已发送，ID: {}", message_id);
/// ```
#[tauri::command]
pub async fn send_email(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    request: storage::SendEmailRequest,
) -> Result<String, String> {
    let db = db_state.clone_conn();

    let account = storage::AccountRepository::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    let username = password_username(request.account_id);
    let keyring = keyring_state.app_handle.keyring();
    let password = keyring
        .get_password(KEYRING_SERVICE, &username)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "密码未找到".to_string())?;

    // 使用新的 SMTP 客户端
    let smtp_client = smtp::SmtpClient::new();

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

    // 连接到 SMTP 服务器
    smtp_client
        .connect(
            &host,
            port,
            &account.email,
            smtp::SmtpAuth::Password(password),
        )
        .await
        .map_err(|e| e.to_string())?;

    // 构建收件人列表
    let to_addresses: Vec<String> = request.to.iter().map(|a| a.email.clone()).collect();

    // 构建发送请求
    let send_request = smtp::SendEmailRequest {
        from: account.email.clone(),
        to: to_addresses,
        cc: None,
        bcc: None,
        subject: request.subject.clone(),
        html_body: request.body_html.clone(),
        text_body: request.body_text.clone(),
        attachments: vec![],
    };

    // 发送邮件
    let result = smtp_client
        .send_email(send_request)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("邮件已发送: {}", result.message_id);

    Ok(result.message_id)
}
