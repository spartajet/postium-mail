use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use chrono::Utc;
use oauth2::{
    basic::BasicClient, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RefreshToken, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::OAuthConfig;

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
        let config = crate::config::load_oauth_config()?;
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

        // 构建授权URL，添加所有必要的scopes
        // 注意：authorize_url接受一个函数，而不是CsrfToken值
        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(
                "https://outlook.office.com/IMAP.AccessAsUser.All".to_string(),
            ))
            .add_scope(Scope::new(
                "https://outlook.office.com/SMTP.Send".to_string(),
            ))
            .add_scope(Scope::new("offline_access".to_string()))
            .add_scope(Scope::new("profile".to_string()))
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("email".to_string()))
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
        use oauth2::{AuthUrl, RedirectUrl, TokenUrl};

        let client = BasicClient::new(ClientId::new(self.config.client_id.clone()))
            .set_auth_uri(AuthUrl::new(self.config.auth_url.clone())?)
            .set_token_uri(TokenUrl::new(self.config.token_url.clone())?)
            .set_redirect_uri(RedirectUrl::new(self.config.redirect_uri.clone())?);

        // 从内存中获取PKCE verifier secret字符串
        let verifier_secret = PKCE_STORE
            .read()
            .await
            .get(state)
            .ok_or_else(|| anyhow!("找不到PKCE verifier，state可能已过期"))?
            .clone();

        // 从字符串创建PkceCodeVerifier
        let pkce_verifier = PkceCodeVerifier::new(verifier_secret);

        // 创建授权码
        let code = AuthorizationCode::new(code.to_string());

        // 使用curl HTTP客户端（同步，需要在spawn_blocking中运行）
        let http = http_client();
        let token_response = tokio::task::spawn_blocking(move || {
            client
                .exchange_code(code)
                .set_pkce_verifier(pkce_verifier)
                .request(&http)
        })
        .await
        .map_err(|e| anyhow!("交换token失败: {}", e))?
        .map_err(|e| anyhow!("交换token失败: {}", e))?;

        // 计算过期时间
        let expires_in = token_response
            .expires_in()
            .map(|d| d.as_secs() as i64)
            .unwrap_or(3600);
        let expires_at = Utc::now().timestamp() + expires_in;

        // 清理PKCE verifier
        PKCE_STORE.write().await.remove(state);

        tracing::info!("成功交换OAuth token，过期时间: {}秒后", expires_in);

        Ok(OAuthToken {
            access_token: token_response.access_token().secret().clone(),
            refresh_token: token_response
                .refresh_token()
                .map(|t| t.secret().clone())
                .unwrap_or_default(),
            expires_at,
        })
    }

    /// 刷新Token
    pub async fn refresh_microsoft_token(&self, refresh_token: &str) -> Result<OAuthToken> {
        use oauth2::{AuthUrl, RedirectUrl, TokenUrl};

        let client = BasicClient::new(ClientId::new(self.config.client_id.clone()))
            .set_auth_uri(AuthUrl::new(self.config.auth_url.clone())?)
            .set_token_uri(TokenUrl::new(self.config.token_url.clone())?)
            .set_redirect_uri(RedirectUrl::new(self.config.redirect_uri.clone())?);

        // 使用curl HTTP客户端
        let http = http_client();
        let refresh_token_copy = refresh_token.to_string(); // 保存原始值用于返回
        let token_response = tokio::task::spawn_blocking(move || {
            client
                .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token_copy))
                .request(&http)
        })
        .await
        .map_err(|e| anyhow!("刷新token失败: {}", e))?
        .map_err(|e| anyhow!("刷新token失败: {}", e))?;

        let expires_in = token_response
            .expires_in()
            .map(|d| d.as_secs() as i64)
            .unwrap_or(3600);
        let expires_at = Utc::now().timestamp() + expires_in;

        tracing::info!("成功刷新OAuth token，过期时间: {}秒后", expires_in);

        Ok(OAuthToken {
            access_token: token_response.access_token().secret().clone(),
            refresh_token: token_response
                .refresh_token()
                .map(|t| t.secret().clone())
                .unwrap_or_else(|| refresh_token.to_string()),
            expires_at,
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
