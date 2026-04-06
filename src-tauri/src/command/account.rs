use crate::error::MailError;
use crate::service::account_service::AccountService;
use crate::service::{AccountDto, CreateAccountRequest, UpdateAccountRequest};

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    service: tauri::State<'_, AccountService>,
) -> Result<Vec<AccountDto>, MailError> {
    tracing::debug!("命令: 列出所有账号");
    let accounts = service.list().await?;
    tracing::debug!(count = accounts.len(), "返回 {} 个账号", accounts.len());
    Ok(accounts)
}

#[tauri::command]
#[specta::specta]
pub async fn get_account(
    service: tauri::State<'_, AccountService>,
    id: i32,
) -> Result<AccountDto, MailError> {
    tracing::debug!(id, "命令: 获取账号");
    service.get(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    service: tauri::State<'_, AccountService>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    tracing::info!(email = %request.email, provider = %request.provider, "命令: 创建账号");
    let result = service.create(request).await?;
    tracing::info!(id = result.id, email = %result.email, "账号创建成功");
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    service: tauri::State<'_, AccountService>,
    request: UpdateAccountRequest,
) -> Result<AccountDto, MailError> {
    tracing::info!(id = request.id, "命令: 更新账号");
    service.update(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_account(
    service: tauri::State<'_, AccountService>,
    id: i32,
) -> Result<(), MailError> {
    tracing::info!(id, "命令: 删除账号");
    service.delete(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_account_password(
    service: tauri::State<'_, AccountService>,
    id: i32,
    password: String,
) -> Result<(), MailError> {
    tracing::info!(id, "命令: 更新账号密码");
    service.update_password(id, password).await
}
