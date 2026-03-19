//! 文件夹操作 Commands

use super::DatabaseState;
use crate::storage::models;
use crate::storage;

#[tauri::command]
pub async fn get_folder_stats(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<Vec<models::folder::FolderDto>, String> {
    let db = state.clone_conn();
    let folders = storage::FolderRepository::get_by_account(&db, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folders.into_iter().map(|f| f.into()).collect())
}
