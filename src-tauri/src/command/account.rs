//! 账号管理 Commands

use super::{DatabaseState, KeyringState};
use crate::storage::models;
use crate::storage;

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
