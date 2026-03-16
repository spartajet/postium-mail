//! 邮件同步 Commands

use std::sync::Arc;
use tauri_plugin_keyring::KeyringExt;

use super::{DatabaseState, KeyringState};
use crate::crypto;
use crate::models;
use crate::services;

#[tauri::command]
pub async fn sync_account_with_progress(
    db_state: tauri::State<'_, DatabaseState>,
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), String> {
    let db = Arc::new(db_state.clone_conn());

    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle.clone());

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
    _keyring_state: tauri::State<'_, KeyringState>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<usize, String> {
    let db = Arc::new(db_state.clone_conn());

    let sync_manager = services::sync_manager::SyncManager::new(db, app_handle);

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

    let account = services::account_service::get_by_id(&db, request.account_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "账号不存在".to_string())?;

    let username = crypto::password_username(request.account_id);
    let keyring = keyring_state.app_handle.keyring();
    let password = keyring
        .get_password(crypto::KEYRING_SERVICE, &username)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "密码未找到".to_string())?;

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
