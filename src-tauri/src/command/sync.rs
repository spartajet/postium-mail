use crate::domain::sync::FolderStat;
use crate::error::MailError;

#[tauri::command]
#[specta::specta]
pub async fn sync_account(
    service: tauri::State<'_, crate::service::SyncService>,
    app_handle: tauri::AppHandle,
    account_id: i32,
) -> Result<(), MailError> {
    tracing::info!(account_id, "命令: 手动同步账号");
    let result = service
        .sync_account_with_progress(app_handle, account_id)
        .await;
    match &result {
        Ok(()) => tracing::info!(account_id, "手动同步完成"),
        Err(e) => tracing::error!(account_id, error = %e, "手动同步失败"),
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn get_folder_stats(
    service: tauri::State<'_, crate::service::SyncService>,
    account_id: i32,
) -> Result<Vec<FolderStat>, MailError> {
    tracing::debug!(account_id, "命令: 获取文件夹统计");
    service.get_folder_stats(account_id).await
}
