//! 邮件操作 Commands

use super::DatabaseState;
use crate::models;
use crate::services;
use crate::storage;

#[tauri::command]
pub async fn list_emails(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<storage::EmailListResponse, String> {
    tracing::info!("========== 邮件列表请求 ==========");
    tracing::info!(
        "参数: account_id={}, folder='{}', page={}, limit={}",
        account_id,
        folder,
        page,
        limit
    );

    let db = state.clone_conn();

    let result = storage::EmailRepository::list(
        &db,
        account_id,
        &folder,
        page as u64,
        limit as u64,
    )
    .await
    .map_err(|e| {
        tracing::error!("查询失败: {}", e);
        e.to_string()
    })?;

    tracing::info!(
        "查询成功: 返回 {} 封邮件，总计 {} 封",
        result.items.len(),
        result.total
    );

    Ok(result)
}

#[tauri::command]
pub async fn get_email(
    state: tauri::State<'_, DatabaseState>,
    id: i32,
) -> Result<models::email::EmailDetail, String> {
    let db = state.clone_conn();
    storage::EmailRepository::get_detail(&db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_emails_fts(
    state: tauri::State<'_, DatabaseState>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<services::search_service::SearchResult>, String> {
    let db = state.clone_conn();
    // 暂时保留旧的搜索服务
    services::search_service::SearchService::search_emails(&db, account_id, &query, limit)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_as_read(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    is_read: bool,
) -> Result<(), String> {
    let db = state.clone_conn();
    storage::EmailRepository::update_read_status(&db, email_id, is_read)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_star(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
) -> Result<bool, String> {
    let db = state.clone_conn();
    storage::EmailRepository::toggle_star(&db, email_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_emails(
    state: tauri::State<'_, DatabaseState>,
    email_ids: Vec<i32>,
) -> Result<usize, String> {
    let db = state.clone_conn();
    storage::EmailRepository::batch_delete(&db, email_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn move_email_to_folder(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    folder: String,
) -> Result<(), String> {
    let db = state.clone_conn();
    storage::EmailRepository::move_to_folder(&db, email_id, &folder)
        .await
        .map_err(|e| e.to_string())
}
