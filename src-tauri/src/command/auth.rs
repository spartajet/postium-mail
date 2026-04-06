use crate::domain::providers::detect::ProviderDetectionResult;
use crate::domain::providers::pool::ProviderPool;
use crate::domain::providers::ProviderInfo;
use crate::error::MailError;
use crate::infrastructure::auth::oauth2::{OAuth2AuthUrl, OAuth2PollResult};
use std::sync::Arc;

#[tauri::command]
#[specta::specta]
pub async fn detect_provider(
    email: String,
    pool: tauri::State<'_, Arc<ProviderPool>>,
) -> Result<ProviderDetectionResult, MailError> {
    tracing::debug!(email = %email, "检测邮箱服务商");
    let result = crate::domain::providers::detect::detect_provider(&email, &pool);
    tracing::info!(email = %email, provider = ?result.provider_id, "检测到服务商");
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_providers() -> Result<Vec<ProviderInfo>, MailError> {
    tracing::debug!("列出所有服务商");
    crate::domain::providers::detect::list_providers()
}

#[tauri::command]
#[specta::specta]
pub async fn start_oauth2(
    manager: tauri::State<'_, Arc<crate::infrastructure::auth::oauth2::OAuth2Manager>>,
    provider_id: String,
    email: String,
    display_name: Option<String>,
) -> Result<OAuth2AuthUrl, MailError> {
    tracing::info!(provider_id = %provider_id, email = %email, "命令: 启动 OAuth2");
    let result = manager
        .start_auth(&provider_id, &email, display_name.as_deref())
        .await?;
    tracing::info!(provider_id = %provider_id, port = result.port, "OAuth2 授权 URL 已生成");
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn poll_oauth2(
    manager: tauri::State<'_, Arc<crate::infrastructure::auth::oauth2::OAuth2Manager>>,
    state: String,
) -> Result<OAuth2PollResult, MailError> {
    tracing::trace!(state = %state, "轮询 OAuth2 状态");
    manager.poll_oauth2(&state).await
}
