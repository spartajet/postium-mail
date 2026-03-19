//! 连接测试 Commands

use super::KeyringState;
use crate::models;
use crate::protocols::imap::{test_connection, ImapAuth, ConnectionTestResult};

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
