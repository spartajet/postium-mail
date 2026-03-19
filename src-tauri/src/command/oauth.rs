//! OAuth 认证 Commands
//!
//! 使用 AuthManager 进行 OAuth 认证

use super::{AuthManagerState, DatabaseState, KeyringState};
use crate::models;
use crate::storage;
use crate::providers;

#[tauri::command]
pub async fn validate_oauth_token(
    provider: String,
    token: String,
) -> Result<bool, String> {
    match provider.as_str() {
        "google" => {
            let client = reqwest::Client::new();
            let response = client
                .get("https://www.googleapis.com/oauth2/v3/userinfo")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;
            Ok(response.status().is_success())
        }
        "microsoft" => {
            let client = reqwest::Client::new();
            let response = client
                .get("https://graph.microsoft.com/v1.0/me")
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| format!("请求失败: {}", e))?;
            Ok(response.status().is_success())
        }
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
pub async fn get_oauth_auth_url(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
) -> Result<(String, String), String> {
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 生成 OAuth 授权 URL
    let context = auth_manager
        .get_oauth_url(&email)
        .await
        .map_err(|e| e.to_string())?;

    Ok((context.auth_url, context.state))
}

#[tauri::command]
pub async fn exchange_oauth_code(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
    code: String,
    state: String,
) -> Result<models::account::AccountDto, String> {
    let db = db_state.clone_conn();
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 进行 OAuth 认证
    let auth_result = auth_manager
        .authenticate_oauth(&email, &code, &state)
        .await
        .map_err(|e| e.to_string())?;

    // 获取服务商配置
    let provider_pool = auth_manager.provider_pool();
    let provider = provider_pool
        .detect_provider(&email)
        .await
        .map_err(|e| e.to_string())?;

    let imap_config = provider.default_imap_config();
    let smtp_config = provider.default_smtp_config();

    // 构建账号创建请求
    let account_req = models::account::CreateAccountRequest {
        name: auth_result.display_name.unwrap_or_else(|| {
            email.split('@')
                .next()
                .unwrap_or("用户")
                .to_string()
        }),
        email: auth_result.email.clone(),
        provider: provider.provider_id().to_string(),
        password: String::new(),
        imap_host: Some(imap_config.host),
        imap_port: Some(imap_config.port as i32),
        imap_ssl: Some(matches!(imap_config.ssl, providers::SslMode::Implicit | providers::SslMode::StartTls)),
        smtp_host: Some(smtp_config.host),
        smtp_port: Some(smtp_config.port as i32),
        smtp_ssl: Some(matches!(smtp_config.ssl, providers::SslMode::StartTls)),
        color: Some("#0078D4".to_string()), // 默认颜色
        auth_type: Some("oauth2".to_string()),
        oauth_provider: Some(provider.provider_id().to_string()),
        oauth_token: auth_result.id_token,
        oauth_refresh_token: Some(
            // 注意：TokenManager 已经存储了 refresh_token，这里只是为了兼容
            // 实际使用时应该从 TokenManager 读取
            String::new()
        ),
        oauth_expires_at: auth_result.expires_at,
    };

    // 创建账号
    let account = storage::AccountRepository::create(
        &db,
        &keyring_state.app_handle,
        account_req.into(),
    )
    .await
    .map_err(|e| e.to_string())?;

    // 迁移 Token 从临时 account_id (0) 到实际的 account_id
    let token_manager = auth_manager.token_manager();
    token_manager
        .migrate_token_account(0, account.id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("成功创建 OAuth 账号: {}", email);

    Ok(account.into())
}

#[tauri::command]
pub async fn refresh_oauth_token(
    auth_manager_state: tauri::State<'_, AuthManagerState>,
    email: String,
    account_id: i32,
) -> Result<crate::crypto::OAuthToken, String> {
    let auth_manager = auth_manager_state.clone_manager();

    // 使用 AuthManager 刷新 token
    auth_manager
        .refresh_token(account_id, &email)
        .await
        .map_err(|e| e.to_string())?;

    // 获取新的 token 信息
    let token_manager = auth_manager.token_manager();
    let token = token_manager
        .get_oauth_token(account_id)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("成功刷新 OAuth token");

    Ok(token)
}
