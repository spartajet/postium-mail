//! 连接测试 Commands
//!
//! 提供邮件服务器连接测试功能，包括：
//! - 测试账号配置的 IMAP 连接
//! - 测试自定义服务器配置的连接
//!
//! # 功能说明
//!
//! 这些命令用于在用户添加或编辑账号配置后验证连接是否正常。
//! 支持常见邮件服务商的自动配置，也支持自定义服务器设置。

use super::KeyringState;
use crate::storage::models;
use crate::protocols::imap::{test_connection, ImapAuth, ConnectionTestResult};

/// 测试账号配置的 IMAP 连接
///
/// 根据账号配置自动检测或使用指定的 IMAP 服务器进行连接测试。
///
/// # 参数
/// * `_keyring_state` - 密钥链状态（当前未使用，保留用于将来扩展）
/// * `account` - 账号创建请求，包含邮箱地址、服务商、服务器配置等信息
///
/// # 返回
/// 成功时返回连接测试结果（ConnectionTestResult），包含：
/// - `success`: 是否连接成功
/// - `message`: 详细的消息说明
/// - `capabilities`: 服务器支持的 IMAP 能力列表（如果连接成功）
///
/// 失败时返回错误信息字符串
///
/// # 服务器配置规则
///
/// 如果 `account.imap_host` 为 None，则根据 `provider` 自动选择：
/// - `gmail` → imap.gmail.com:993
/// - `outlook` / `hotmail` → outlook.office365.com:993
/// - `icloud` → imap.mail.me.com:993
/// - `yahoo` → imap.mail.yahoo.com:993
/// - 其他 → imap.example.com:993
///
/// # 示例
/// ```rust,no_run
/// use crate::command::connection::test_account_connection;
/// use crate::models::account::CreateAccountRequest;
/// use std::default::Default;
///
/// let account = CreateAccountRequest {
///     email: "user@gmail.com".to_string(),
///     provider: "gmail".to_string(),
///     password: "app_password".to_string(),
///     imap_host: None,
///     imap_port: Some(993),
///     ..Default::default()
/// };
/// let result = test_account_connection(keyring_state, account).await;
/// ```
#[tauri::command]
pub async fn test_account_connection(
    _keyring_state: tauri::State<'_, KeyringState>,
    account: models::account::CreateAccountRequest,
) -> Result<ConnectionTestResult, String> {
    let password = account.password.clone();

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
    let auth = ImapAuth::Password(password);

    test_connection(&host, port as u16, &account.email, auth)
        .await
        .map_err(|e| e.to_string())
}

/// 测试自定义邮箱配置的连接
///
/// 使用提供的服务器配置直接测试 IMAP 连接，不依赖已保存的账号数据。
/// 适用于用户在添加账号前验证服务器配置是否正确。
///
/// # 参数
/// * `email` - 邮箱地址
/// * `password` - 密码或应用专用密码
/// * `provider` - 邮件服务商标识（如 "gmail", "outlook"）
/// * `imap_host` - IMAP 服务器主机名（为 None 时自动检测）
/// * `imap_port` - IMAP 服务器端口（为 None 时使用 993）
/// * `_imap_ssl` - 是否使用 SSL（当前未使用）
/// * `_smtp_host` - SMTP 服务器主机名（当前未使用，预留）
/// * `_smtp_port` - SMTP 服务器端口（当前未使用，预留）
/// * `_smtp_ssl` - SMTP SSL 设置（当前未使用，预留）
///
/// # 返回
/// 成功时返回空值，失败时返回包含 "IMAP 连接失败: " 前缀的错误信息
///
/// # 自动检测规则
///
/// 当 `imap_host` 为 None 时，根据 `provider` 自动选择：
/// - `gmail` → imap.gmail.com
/// - `outlook` / `hotmail` → outlook.office365.com
/// - `icloud` → imap.mail.me.com
/// - `yahoo` → imap.mail.yahoo.com
/// - 其他 → imap.example.com
///
/// # 示例
/// ```rust,no_run
/// use crate::command::connection::test_email_connection;
///
/// // 测试 Gmail 连接
/// test_email_connection(
///     "user@gmail.com".to_string(),
///     "app_password".to_string(),
///     "gmail".to_string(),
///     None,
///     None,
///     Some(true),
///     None,
///     None,
///     None,
/// ).await?;
///
/// // 测试自定义服务器
/// test_email_connection(
///     "user@example.com".to_string(),
///     "password".to_string(),
///     "custom".to_string(),
///     Some("imap.example.com".to_string()),
///     Some(993),
///     Some(true),
///     None,
///     None,
///     None,
/// ).await?;
/// ```
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn test_email_connection(
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
    let host = imap_host.unwrap_or_else(|| match provider.as_str() {
        "gmail" => "imap.gmail.com".to_string(),
        "outlook" | "hotmail" => "outlook.office365.com".to_string(),
        "icloud" => "imap.mail.me.com".to_string(),
        "yahoo" => "imap.mail.yahoo.com".to_string(),
        _ => "imap.example.com".to_string(),
    });

    let port = imap_port.unwrap_or(993);
    let auth = ImapAuth::Password(password);

    test_connection(&host, port, &email, auth)
        .await
        .map_err(|e| format!("IMAP 连接失败: {}", e))?;

    Ok(())
}
