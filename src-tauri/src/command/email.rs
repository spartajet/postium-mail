use crate::error::MailError;
use crate::infrastructure::storage::search::SearchResult;
use crate::service::email_service::{EmailDetail, EmailListResponse, SendEmailRequest};

#[tauri::command]
#[specta::specta]
pub async fn list_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<EmailListResponse, MailError> {
    tracing::debug!(account_id, folder = %folder, page, limit, "命令: 列出邮件");
    let result = service.list(account_id, &folder, page, limit).await?;
    tracing::debug!(account_id, folder = %folder, total = result.total, returned = result.emails.len(), "邮件列表");
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn get_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    id: i32,
) -> Result<EmailDetail, MailError> {
    tracing::debug!(id, "命令: 获取邮件详情");
    service.get(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn search_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<SearchResult>, MailError> {
    tracing::info!(query = %query, account_id, limit, "命令: 搜索邮件");
    let results = service.search(&query, account_id, limit).await?;
    tracing::info!(query = %query, count = results.len(), "搜索完成");
    Ok(results)
}

#[tauri::command]
#[specta::specta]
pub async fn mark_as_read(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    is_read: bool,
) -> Result<(), MailError> {
    tracing::info!(email_id, is_read, "命令: 标记已读/未读");
    service.mark_as_read(email_id, is_read).await
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_star(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<bool, MailError> {
    tracing::info!(email_id, "命令: 切换星标");
    service.toggle_star(email_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_ids: Vec<i32>,
) -> Result<usize, MailError> {
    tracing::info!(count = email_ids.len(), ids = ?email_ids, "命令: 删除邮件");
    let deleted = service.delete(email_ids).await?;
    tracing::info!(deleted, "邮件已删除");
    Ok(deleted)
}

#[tauri::command]
#[specta::specta]
pub async fn move_email_to_folder(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    folder: String,
) -> Result<(), MailError> {
    tracing::info!(email_id, folder = %folder, "命令: 移动邮件");
    service.move_to_folder(email_id, &folder).await
}

#[tauri::command]
#[specta::specta]
pub async fn send_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    request: SendEmailRequest,
) -> Result<String, MailError> {
    tracing::info!(
        account_id = request.account_id,
        to = request.to.len(),
        cc = request.cc.len(),
        bcc = request.bcc.len(),
        subject = %request.subject,
        "命令: 发送邮件"
    );
    let result = service.send(request).await?;
    tracing::info!("邮件发送成功");
    Ok(result)
}
