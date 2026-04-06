use crate::error::MailError;
use crate::service::label_service::{
    CreateLabelRequest, LabelDto, LabelService, UpdateLabelRequest,
};

#[tauri::command]
#[specta::specta]
pub async fn list_labels(
    service: tauri::State<'_, LabelService>,
    account_id: i32,
) -> Result<Vec<LabelDto>, MailError> {
    tracing::debug!(account_id, "命令: 列出标签");
    service.list_labels(account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_label(
    service: tauri::State<'_, LabelService>,
    request: CreateLabelRequest,
) -> Result<LabelDto, MailError> {
    tracing::info!(account_id = request.account_id, name = %request.name, "命令: 创建标签");
    let result = service.create_label(request).await?;
    tracing::info!(id = result.id, name = %result.name, "标签创建成功");
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn update_label(
    service: tauri::State<'_, LabelService>,
    id: i32,
    request: UpdateLabelRequest,
) -> Result<LabelDto, MailError> {
    tracing::info!(id, "命令: 更新标签");
    service.update_label(id, request).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_label(
    service: tauri::State<'_, LabelService>,
    id: i32,
) -> Result<(), MailError> {
    tracing::info!(id, "命令: 删除标签");
    service.delete_label(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_label_to_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    tracing::debug!(email_id, label_id, "命令: 为邮件添加标签");
    service.add_label_to_email(email_id, label_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_label_from_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
    label_id: i32,
) -> Result<(), MailError> {
    tracing::debug!(email_id, label_id, "命令: 移除邮件标签");
    service.remove_label_from_email(email_id, label_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_labels_for_email(
    service: tauri::State<'_, LabelService>,
    email_id: i32,
) -> Result<Vec<LabelDto>, MailError> {
    tracing::debug!(email_id, "命令: 获取邮件标签列表");
    service.get_labels_for_email(email_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_emails_by_label(
    service: tauri::State<'_, LabelService>,
    label_id: i32,
) -> Result<Vec<i32>, MailError> {
    tracing::debug!(label_id, "命令: 按标签列出邮件");
    service.list_emails_by_label(label_id).await
}
