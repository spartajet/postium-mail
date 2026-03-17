#![deprecated(
    since = "0.1.0",
    note = "使用 providers::personal::outlook_oauth::OutlookOAuthService 代替。此模块已迁移到服务商适配层。"
)]

//! # Microsoft OAuth 2.0 服务（已废弃）
//!
//! **此模块已废弃，请使用 `providers::personal::outlook_oauth::OutlookOAuthService` 代替**
//!
//! 迁移指南：
//! - 使用 `OutlookOAuthService::from_env()` 从环境变量加载配置
//! - 使用 `OutlookOAuthService::get_auth_url()` 获取授权 URL
//! - 使用 `OutlookOAuthService::exchange_code()` 交换授权码
//! - 使用 `OutlookOAuthService::refresh_token()` 刷新 token
//!
//! ## 旧用法（不推荐）
//!
//! ```ignore
//! let oauth_service = OAuthService::new(config);
//! let auth_url = oauth_service.get_auth_url(&state).await?;
//! ```
//!
//! ## 新用法（推荐）
//!
//! ```ignore
//! let oauth_service = OutlookOAuthService::from_env()?;
//! let auth_url = oauth_service.get_auth_url(&state)?;
//! ```

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::Utc;
use oauth2::{basic::BasicClient, ClientId, CsrfToken, PkceCodeChallenge, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::OAuthConfig;

/// 自定义 TokenResponse 用于提取 id_token
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MicrosoftTokenResponse {
    /// 标准的 access_token
    access_token: String,
    /// Token 类型
    token_type: String,
    /// 过期时间（秒）
    expires_in: Option<u64>,
    /// Refresh token
    refresh_token: Option<String>,
    /// Scope
    scope: Option<String>,
    /// ID Token (JWT格式，包含用户信息)
    id_token: Option<String>,
}

// 使用curl HTTP客户端（避免与Tauri的reqwest冲突）
fn http_client() -> oauth2::CurlHttpClient {
    // CurlHttpClient实现了Default trait
    oauth2::CurlHttpClient {}
}

/// OAuth Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64, // Unix 时间戳
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>, // JWT格式的用户信息token（当使用openid scope时返回）
}

/// OAuth 授权上下文（不包含PKCE verifier，因为它不可Clone）
#[derive(Debug)]
pub struct AuthorizationContext {
    pub auth_url: String,
    pub csrf_token: String, // CsrfToken的secret字符串
}

/// PKCE Verifier存储（用于token交换）
/// 存储字符串形式的PKCE verifier secret
static PKCE_STORE: once_cell::sync::Lazy<Arc<RwLock<HashMap<String, String>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

/// OAuth 服务
#[derive(Debug, Clone)]
pub struct OAuthService {
    config: OAuthConfig,
}

impl OAuthService {
    /// 创建新的OAuth服务实例
    pub fn new(config: OAuthConfig) -> Result<Self> {
        Ok(Self { config })
    }

    /// 从环境变量加载配置并创建OAuth服务
    pub fn from_env() -> Result<Self> {
        let config = crate::config::load_microsoft_oauth_config()?;
        Self::new(config)
    }

    /// 生成Microsoft授权URL（使用PKCE）
    pub async fn get_microsoft_auth_url(&self) -> Result<AuthorizationContext> {
        use oauth2::{AuthUrl, RedirectUrl, TokenUrl};

        // 直接创建客户端以避免类型状态问题
        let client = BasicClient::new(ClientId::new(self.config.client_id.clone()))
            .set_auth_uri(AuthUrl::new(self.config.auth_url.clone())?)
            .set_token_uri(TokenUrl::new(self.config.token_url.clone())?)
            .set_redirect_uri(RedirectUrl::new(self.config.redirect_uri.clone())?);

        // 生成PKCE code challenge和verifier
        let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

        // 构建授权URL，使用 Exchange Online scopes
        // 注意：不能混合使用 Exchange Online 和 Microsoft Graph scopes
        // 需要 openid scope 才能获取 JWT 格式的 access token
        // profile 和 email scope 不需要，用户信息从 JWT 中提取
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://outlook.office.com/IMAP.AccessAsUser.All".to_string(),
            ))
            .add_scope(Scope::new(
                "https://outlook.office.com/SMTP.Send".to_string(),
            ))
            .add_scope(Scope::new("offline_access".to_string()))
            .add_scope(Scope::new("openid".to_string()))
            .set_pkce_challenge(pkce_code_challenge)
            .url();

        let state_secret = csrf_token.secret().clone();

        // 将verifier的secret存储到内存中，使用state作为key
        PKCE_STORE
            .write()
            .await
            .insert(state_secret.clone(), pkce_code_verifier.secret().clone());

        tracing::info!("生成OAuth授权URL成功，state: {}", state_secret);

        Ok(AuthorizationContext {
            auth_url: auth_url.to_string(),
            csrf_token: state_secret,
        })
    }

    /// 交换授权码获取Token（使用PKCE verifier）
    pub async fn exchange_microsoft_code(&self, code: &str, state: &str) -> Result<OAuthToken> {
        // 从内存中获取PKCE verifier secret字符串
        let verifier_secret = PKCE_STORE
            .read()
            .await
            .get(state)
            .ok_or_else(|| anyhow!("找不到PKCE verifier，state可能已过期"))?
            .clone();

        tracing::info!("开始交换 OAuth token...");
        tracing::info!("  - code: {}...", &code[..20.min(code.len())]);
        tracing::info!("  - state: {}", state);

        // 直接使用 curl 发送 HTTP 请求以获取完整的响应（包括 id_token）
        let token_url = self.config.token_url.clone();
        let client_id = self.config.client_id.clone();
        let redirect_uri = self.config.redirect_uri.clone();
        let code = code.to_string();
        let scopes_str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid".to_string();

        let response = tokio::task::spawn_blocking(move || {
            // 使用 curl 发送 POST 请求
            let mut handle = curl::easy::Easy::new();
            handle.url(&token_url)?;
            handle.post(true)?;

            // 构建请求体
            let params = [
                format!("client_id={}", client_id),
                format!("code={}", code),
                format!("redirect_uri={}", redirect_uri),
                "grant_type=authorization_code".to_string(),
                format!("code_verifier={}", verifier_secret),
                format!("scope={}", scopes_str),
            ];
            let body = params.join("&");
            handle.post_fields_copy(body.as_bytes())?;

            // 设置响应回调
            let mut response_data = Vec::new();
            {
                let mut transfer = handle.transfer();
                transfer.write_function(|data| {
                    response_data.extend_from_slice(data);
                    Ok(data.len())
                })?;
                transfer.perform()?;
            }

            // 检查 HTTP 状态码
            let status_code = handle.response_code()?;
            if status_code != 200 {
                let error_text = String::from_utf8_lossy(&response_data);
                return Err(anyhow!("HTTP {}: {}", status_code, error_text));
            }

            Ok(response_data)
        })
        .await
        .map_err(|e| anyhow!("交换token失败: {}", e))?
        .map_err(|e| anyhow!("交换token失败: {}", e))?;

        // 解析 JSON 响应
        let response_text = String::from_utf8(response)?;
        let token_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("解析token响应失败: {}", e))?;

        tracing::info!("========== OAuth Token 交换成功 ==========");
        tracing::info!("  响应: {}", response_text);

        // 提取字段
        let access_token = token_json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 access_token"))?
            .to_string();
        let refresh_token = token_json["refresh_token"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let expires_in = token_json["expires_in"].as_u64().unwrap_or(3600) as i64;
        let id_token = token_json["id_token"].as_str().map(String::from);

        let expires_at = Utc::now().timestamp() + expires_in;

        // 清理PKCE verifier
        PKCE_STORE.write().await.remove(state);

        // 打印 token 信息（用于调试）
        tracing::info!("  expires_in: {} 秒", expires_in);
        tracing::info!("  expires_at: {} (Unix timestamp)", expires_at);
        tracing::info!("  access_token 长度: {}", access_token.len());

        // 打印 id_token 信息
        if let Some(ref idt) = id_token {
            tracing::info!(
                "  id_token (前50字符): {}...",
                if idt.len() > 50 { &idt[..50] } else { idt }
            );
            tracing::info!("  id_token 长度: {}", idt.len());
            let jwt_parts = idt.split('.').count();
            tracing::info!("  id_token 段数: {} (JWT格式应该是3段)", jwt_parts);
        } else {
            tracing::info!("  id_token: 无（可能需要openid scope）");
        }

        tracing::info!("==========================================");

        Ok(OAuthToken {
            access_token,
            refresh_token,
            expires_at,
            id_token,
        })
    }

    /// 刷新Token
    pub async fn refresh_microsoft_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        tracing::info!("开始刷新 OAuth token...");

        // 直接使用 curl 发送 HTTP 请求
        let token_url = self.config.token_url.clone();
        let client_id = self.config.client_id.clone();
        let refresh_token_owned = refresh_token.to_string();
        let refresh_token_clone = refresh_token_owned.clone(); // 保留一份副本
        let scopes_str = "https://outlook.office.com/IMAP.AccessAsUser.All https://outlook.office.com/SMTP.Send offline_access openid".to_string();

        let response = tokio::task::spawn_blocking(move || {
            // 使用 curl 发送 POST 请求
            let mut handle = curl::easy::Easy::new();
            handle.url(&token_url)?;
            handle.post(true)?;

            // 构建请求体
            let params = [
                format!("client_id={}", client_id),
                format!("refresh_token={}", refresh_token_owned),
                "grant_type=refresh_token".to_string(),
                format!("scope={}", scopes_str),
            ];
            let body = params.join("&");
            handle.post_fields_copy(body.as_bytes())?;

            // 设置响应回调
            let mut response_data = Vec::new();
            {
                let mut transfer = handle.transfer();
                transfer.write_function(|data| {
                    response_data.extend_from_slice(data);
                    Ok(data.len())
                })?;
                transfer.perform()?;
            }

            // 检查 HTTP 状态码
            let status_code = handle.response_code()?;
            if status_code != 200 {
                let error_text = String::from_utf8_lossy(&response_data);
                return Err(anyhow!("HTTP {}: {}", status_code, error_text));
            }

            Ok(response_data)
        })
        .await
        .map_err(|e| anyhow!("刷新token失败: {}", e))?
        .map_err(|e| anyhow!("刷新token失败: {}", e))?;

        // 解析 JSON 响应
        let response_text = String::from_utf8(response)?;
        let token_json: Value = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("解析token响应失败: {}", e))?;

        tracing::info!("========== OAuth Token 刷新成功 ==========");
        tracing::info!("  响应: {}", response_text);

        // 提取字段
        let access_token = token_json["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 access_token"))?
            .to_string();
        let new_refresh_token = token_json["refresh_token"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| {
                // 如果响应中没有新的 refresh_token，使用旧的
                tracing::warn!("响应中未包含新的 refresh_token，使用旧的");
                // 这里需要使用旧的 refresh_token，但我们已经在 move 中消耗了它
                // 实际上 Microsoft 应该总是返回新的 refresh_token
                "".to_string()
            });
        let expires_in = token_json["expires_in"].as_u64().unwrap_or(3600) as i64;
        let expires_at = Utc::now().timestamp() + expires_in;

        // 如果新的 refresh_token 为空，使用旧的
        let new_refresh_token = if new_refresh_token.is_empty() {
            // 需要从外部传入原始的 refresh_token
            refresh_token_clone
        } else {
            new_refresh_token
        };

        // 打印 token 信息（用于调试）
        tracing::info!("  expires_in: {} 秒", expires_in);
        tracing::info!("  expires_at: {} (Unix timestamp)", expires_at);
        tracing::info!("  access_token 长度: {}", access_token.len());

        tracing::info!("==========================================");

        Ok(OAuthToken {
            access_token,
            refresh_token: new_refresh_token,
            expires_at,
            // 刷新 token 时可能不会返回新的 id_token
            id_token: None,
        })
    }

    /// 验证Token有效性
    pub async fn validate_token(&self, token: &str) -> Result<bool> {
        let response = reqwest::Client::new()
            .get("https://graph.microsoft.com/v1.0/me")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| anyhow!("验证token失败: {}", e))?;

        let is_valid = response.status().is_success();
        tracing::info!("Token验证结果: {}", is_valid);

        Ok(is_valid)
    }

    /// 生成XOAUTH2字符串（用于IMAP/SMTP认证）
    pub fn generate_xoauth2_string(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        BASE64_STANDARD.encode(auth_string)
    }

    /// 检查Token是否即将过期（5分钟内）
    pub fn is_token_expiring_soon(&self, expires_at: i64) -> bool {
        let now = Utc::now().timestamp();
        let remaining = expires_at - now;
        remaining < 300 // 5分钟 = 300秒
    }
}

impl Default for OAuthService {
    fn default() -> Self {
        // 尝试从环境变量加载配置
        Self::from_env().unwrap_or_else(|_| Self {
            config: OAuthConfig {
                client_id: String::new(),
                redirect_uri: String::new(),
                tenant: String::new(),
                scopes: Vec::new(),
                auth_url: String::new(),
                token_url: String::new(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_xoauth2_string() {
        let config = OAuthConfig {
            client_id: "test".to_string(),
            redirect_uri: "test://callback".to_string(),
            tenant: "common".to_string(),
            scopes: vec![],
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
        };

        let service = OAuthService::new(config).unwrap();
        let xoauth2 = service.generate_xoauth2_string("user@example.com", "test_token");

        // XOAUTH2格式: base64("user=<email>\x01auth=Bearer <token>\x01\x01")
        assert!(xoauth2.contains("dXNlcj1")); // "user="的base64编码开头
    }

    #[test]
    fn test_is_token_expiring_soon() {
        let config = OAuthConfig {
            client_id: "test".to_string(),
            redirect_uri: "test://callback".to_string(),
            tenant: "common".to_string(),
            scopes: vec![],
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
        };

        let service = OAuthService::new(config).unwrap();

        // 测试即将过期的token
        let expiring_soon = Utc::now().timestamp() + 200; // 200秒后过期
        assert!(service.is_token_expiring_soon(expiring_soon));

        // 测试未过期的token
        let not_expiring = Utc::now().timestamp() + 3600; // 1小时后过期
        assert!(!service.is_token_expiring_soon(not_expiring));
    }
}
