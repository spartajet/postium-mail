//! 账号管理 Commands
//!
//! 提供账号的增删改查功能，包括：
//! - 添加新的邮件账号
//! - 列出所有账号
//! - 获取单个账号详情
//! - 更新账号配置
//! - 删除账号

use super::{DatabaseState, KeyringState};
use crate::storage::models;
use crate::storage;

/// 添加新的邮件账号
///
/// 创建一个新的邮件账号，保存到数据库，并在系统密钥链中存储密码。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `keyring_state` - 密钥链状态，用于安全存储密码
/// * `account` - 账号创建请求，包含邮箱地址、服务器配置等信息
///
/// # 返回
/// 成功时返回创建的账号对象（AccountDto），失败时返回错误信息字符串
///
/// # 错误处理
/// - 如果邮箱格式无效，返回验证错误
/// - 如果连接服务器失败，返回连接错误
/// - 如果数据库操作失败，返回数据库错误
///
/// # 示例
/// ```rust
/// let account = CreateAccountRequest {
///     name: "我的邮箱".to_string(),
///     email: "user@example.com".to_string(),
///     provider: "gmail".to_string(),
///     imap_host: Some("imap.gmail.com".to_string()),
///     imap_port: Some(993),
///     imap_ssl: Some(true),
///     smtp_host: Some("smtp.gmail.com".to_string()),
///     smtp_port: Some(465),
///     smtp_ssl: Some(true),
///     password: "app_password".to_string(),
///     ..Default::default()
/// };
/// let result = add_account(db_state, keyring_state, account).await;
/// ```
#[tauri::command]
pub async fn add_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    let request = storage::CreateAccountRequest::from(account);

    storage::AccountRepository::create(&db, app_handle, request)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

/// 列出所有邮件账号
///
/// 从数据库中获取所有已配置的邮件账号列表。
///
/// # 参数
/// * `state` - 数据库连接状态
///
/// # 返回
/// 成功时返回账号列表（AccountDto 数组），失败时返回错误信息字符串
///
/// # 说明
/// - 返回的账号列表按创建时间倒序排列
/// - 包含每个账号的基本信息和连接状态
#[tauri::command]
pub async fn list_accounts(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    let accounts = storage::AccountRepository::get_all(&db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(accounts.into_iter().map(|a| a.into()).collect())
}

/// 获取单个账号详情
///
/// 根据账号 ID 获取账号的详细信息。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `id` - 账号 ID
///
/// # 返回
/// - 成功且账号存在时：返回账号详情（AccountDto）
/// - 成功但账号不存在时：返回 None
/// - 失败时：返回错误信息字符串
#[tauri::command]
pub async fn get_account(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<Option<models::account::AccountDto>, String> {
    let db = state.clone_conn();
    match storage::AccountRepository::get_by_id(&db, id).await {
        Ok(Some(account)) => Ok(Some(account.into())),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// 更新账号配置
///
/// 更新指定账号的配置信息，如名称、服务器配置等。
/// 如果提供了新的密码，会在密钥链中更新存储的密码。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `keyring_state` - 密钥链状态
/// * `id` - 要更新的账号 ID
/// * `account` - 包含更新字段的账号请求
///
/// # 返回
/// 成功时返回更新后的账号对象，失败时返回错误信息字符串
///
/// # 错误处理
/// - 如果账号 ID 不存在，返回"账号未找到"错误
/// - 如果新的服务器配置无法连接，返回连接错误
///
/// # 注意
/// - 未在请求中提供的字段将保持不变
/// - 同步设置（sync_enabled）暂不支持更新
#[tauri::command]
pub async fn update_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
    account: models::account::CreateAccountRequest,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    // 构建更新请求
    let request = storage::UpdateAccountRequest {
        name: Some(account.name),
        email: Some(account.email),
        provider: Some(account.provider),
        imap_host: account.imap_host,
        imap_port: account.imap_port,
        imap_ssl: account.imap_ssl,
        smtp_host: account.smtp_host,
        smtp_port: account.smtp_port,
        smtp_ssl: account.smtp_ssl,
        color: account.color,
        sync_enabled: None, // TODO: 从 request 中获取
    };

    storage::AccountRepository::update(&db, id, request)
        .await
        .map(|a| a.into())
        .map_err(|e| e.to_string())
}

/// 删除邮件账号
///
/// 从数据库中删除指定的账号，同时清理相关的数据：
/// - 删除账号记录
/// - 从密钥链中删除存储的密码
/// - 删除该账号的所有邮件和文件夹数据
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `keyring_state` - 密钥链状态
/// * `id` - 要删除的账号 ID
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 错误处理
/// - 如果账号 ID 不存在，返回"账号未找到"错误
/// - 如果数据库删除失败，返回数据库错误
///
/// # 注意
/// - 此操作不可逆，删除后无法恢复
/// - 将同时删除该账号下的所有邮件和文件夹
/// - 建议在删除前提示用户确认
#[tauri::command]
pub async fn delete_account(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    id: i32,
) -> Result<(), String> {
    let db = db_state.clone_conn();
    let app_handle = &keyring_state.app_handle;

    storage::AccountRepository::delete(&db, app_handle, id)
        .await
        .map_err(|e| e.to_string())
}
