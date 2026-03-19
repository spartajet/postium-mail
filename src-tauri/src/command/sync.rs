//! 邮件同步 Commands

use std::sync::Arc;
use tauri_plugin_keyring::KeyringExt;

use super::{AuthManagerState, DatabaseState, KeyringState, ProviderPoolState};
use crate::crypto;
use crate::models;
use crate::protocols::smtp;
use crate::storage;
use crate::sync;

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

#[tauri::command]
pub async fn send_email(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    request: models::email::SendEmailRequest,
) -> Result<String, String> {
    let db = db_state.clone_conn();

    let account = storage::AccountRepository::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    let username = crypto::password_username(request.account_id);
    let keyring = keyring_state.app_handle.keyring();
    let password = keyring
        .get_password(crypto::KEYRING_SERVICE, &username)
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
