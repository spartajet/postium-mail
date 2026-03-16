use anyhow::anyhow;
use base64::Engine;

use super::{DatabaseState, KeyringState, OAuthState};
use crate::crypto;
use crate::models;
use crate::services;

#[tauri::command]
pub async fn validate_oauth_token(provider: String, token: String) -> Result<bool, String> {
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
    state: tauri::State<'_, OAuthState>,
    provider: String,
) -> Result<(String, String), String> {
    let oauth_service = &state.0;

    match provider.as_str() {
        "microsoft" => {
            let auth_context = oauth_service
                .get_microsoft_auth_url()
                .await
                .map_err(|e| e.to_string())?;
            Ok((auth_context.auth_url, auth_context.csrf_token))
        }
        "google" => Err("Google OAuth 暂未实现，请使用 Microsoft OAuth".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
pub async fn exchange_oauth_code(
    db_state: tauri::State<'_, DatabaseState>,
    keyring_state: tauri::State<'_, KeyringState>,
    oauth_state: tauri::State<'_, OAuthState>,
    provider: String,
    code: String,
    csrf_state: String,
) -> Result<models::account::AccountDto, String> {
    let oauth_service = &oauth_state.0;

    match provider.as_str() {
        "microsoft" => {
            let token = oauth_service
                .exchange_microsoft_code(&code, &csrf_state)
                .await
                .map_err(|e| e.to_string())?;

            let id_token = token
                .id_token
                .as_ref()
                .ok_or_else(|| "未获取到 id_token，请确保 openid scope 已启用".to_string())?;
            let (email, display_name) =
                get_user_info_from_token(id_token).map_err(|e| e.to_string())?;

            tracing::info!(
                "从 token 解析用户信息: email={}, display_name={}",
                email,
                display_name
            );

            let db = db_state.clone_conn();
            let account_req = models::account::CreateAccountRequest {
                name: display_name,
                email: email.clone(),
                provider: "outlook".to_string(),
                password: String::new(),
                imap_host: Some("outlook.office365.com".to_string()),
                imap_port: Some(993),
                imap_ssl: Some(true),
                smtp_host: Some("smtp-mail.outlook.com".to_string()),
                smtp_port: Some(587),
                smtp_ssl: Some(true),
                color: Some("#0078D4".to_string()),
                auth_type: Some("oauth2".to_string()),
                oauth_provider: Some("microsoft".to_string()),
                oauth_token: Some(token.access_token.clone()),
                oauth_refresh_token: Some(token.refresh_token.clone()),
                oauth_expires_at: Some(token.expires_at),
            };

            let account =
                services::account_service::create(&db, &keyring_state.app_handle, account_req)
                    .await
                    .map_err(|e| e.to_string())?;

            tracing::info!("成功创建OAuth账号: {}", email);

            Ok(account.into())
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

#[tauri::command]
pub async fn refresh_oauth_token(
    oauth_state: tauri::State<'_, OAuthState>,
    provider: String,
    refresh_token: String,
) -> Result<crypto::OAuthToken, String> {
    let oauth_service = &oauth_state.0;

    match provider.as_str() {
        "microsoft" => {
            let token = oauth_service
                .refresh_microsoft_token(&refresh_token)
                .await
                .map_err(|e| e.to_string())?;

            tracing::info!("成功刷新OAuth token");

            Ok(crypto::OAuthToken {
                refresh_token: token.refresh_token,
                expires_at: token.expires_at,
            })
        }
        "google" => Err("Google OAuth 暂未实现".to_string()),
        _ => Err(format!("不支持的 OAuth 提供商: {}", provider)),
    }
}

/// 从 JWT access token 中解析用户信息
fn get_user_info_from_token(access_token: &str) -> anyhow::Result<(String, String)> {
    tracing::info!("========== JWT Token 解析 ==========");
    tracing::info!("  token 长度: {}", access_token.len());
    tracing::info!(
        "  token 前100字符: {}",
        &access_token[..100.min(access_token.len())]
    );

    let parts: Vec<&str> = access_token.split('.').collect();
    tracing::info!("  分段数量: {}", parts.len());

    if parts.len() != 3 {
        for (i, part) in parts.iter().enumerate() {
            tracing::info!("  段{} 长度: {}", i, part.len());
        }
        return Err(anyhow!(
            "无效的 JWT token 格式，期望3段，实际{}段",
            parts.len()
        ));
    }

    let payload = parts
        .get(1)
        .ok_or_else(|| anyhow!("JWT token 缺少 payload"))?;

    let payload_json = base64_url_decode(payload)?;

    let claims: serde_json::Value =
        serde_json::from_str(&payload_json).map_err(|e| anyhow!("解析 JWT payload 失败: {}", e))?;

    let email = claims
        .get("upn")
        .or_else(|| claims.get("email"))
        .or_else(|| claims.get("unique_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@example.com");

    let name = claims
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| email.split('@').next().unwrap_or("用户"));

    Ok((email.to_string(), name.to_string()))
}

/// Base64URL 解码
fn base64_url_decode(input: &str) -> anyhow::Result<String> {
    let input_padded = if input.len().is_multiple_of(4) {
        input.to_string()
    } else {
        let padding = "=".repeat(4 - (input.len() % 4));
        format!("{}{}", input, padding)
    };

    let input_standard = input_padded.replace('-', "+").replace('_', "/");

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&input_standard)
        .map_err(|e| anyhow!("Base64 解码失败: {}", e))?;

    String::from_utf8(bytes).map_err(|e| anyhow!("UTF-8 转换失败: {}", e))
}
