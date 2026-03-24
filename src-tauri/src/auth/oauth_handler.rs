//! OAuth 处理器
//!
//! 处理 OAuth 2.0 授权流程，支持所有 OAuth 提供商

use std::sync::Arc;

use rand::Rng;

use crate::error::{OAuthError, Result};
use crate::providers::{MailProvider, OAuthTokenResponse, PkceVerifierStore};

/// URL 编码（用于 OAuth 参数）
fn url_encode(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u8)
            }
        })
        .collect()
}

/// OAuth 授权上下文
#[derive(Debug, Clone)]
pub struct AuthorizationContext {
    /// 授权 URL（用于在浏览器中打开）
    pub auth_url: String,
    /// CSRF 防护令牌
    pub state: String,
    /// PKCE code_verifier
    pub code_verifier: String,
    /// 服务商 ID
    pub provider: String,
}

/// OAuth 处理器
///
/// 负责 OAuth 2.0 流程处理：
/// - 授权 URL 生成（支持所有提供商）
/// - PKCE 流程集成
/// - 授权码交换
/// - Token 刷新
/// - XOAUTH2 字符串生成
pub struct OAuthHandler {
    /// PKCE 验证器存储
    pkce_store: Arc<PkceVerifierStore>,
    // Provider 配置缓存（避免重复解析）
    // provider_configs: Arc<RwLock<HashMap<String, OAuthConfig>>>,
}

impl OAuthHandler {
    /// 默认 CSRF token 长度
    const CSRF_TOKEN_LENGTH: usize = 32;

    /// 创建新的 OAuth 处理器
    pub fn new() -> Self {
        Self {
            pkce_store: Arc::new(PkceVerifierStore::new()),
            // provider_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 生成授权 URL（支持所有 OAuth 提供商）
    ///
    /// # 参数
    ///
    /// * `provider` - 邮件服务商实现
    ///
    /// # 返回
    ///
    /// 返回包含授权 URL 和 state 的上下文
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let handler = OAuthHandler::new();
    /// let provider = GmailProvider;
    /// let context = handler.get_authorization_url(&provider).await?;
    /// // 在浏览器中打开 context.auth_url
    /// ```
    pub async fn get_authorization_url(
        &self,
        provider: &dyn MailProvider,
    ) -> Result<AuthorizationContext> {
        // 1. 获取 OAuth 配置
        let oauth_config = provider
            .oauth_config()
            .ok_or_else(|| OAuthError::NetworkError("服务商不支持 OAuth".to_string()))?;

        oauth_config.validate()?;

        // 2. 生成 CSRF token (state)
        let state: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(Self::CSRF_TOKEN_LENGTH)
            .map(char::from)
            .collect();

        // 3. 生成 PKCE 验证器
        let (code_verifier, code_challenge) = self
            .pkce_store
            .generate_and_store(&state)
            .map_err(OAuthError::RefreshFailed)?;

        // 4. 构建 URL 参数
        let scope = oauth_config.scopes.join(" ");
        let params = [
            ("client_id", oauth_config.client_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", oauth_config.redirect_uri.as_str()),
            ("scope", scope.as_str()),
            ("state", state.as_str()),
            ("code_challenge", code_challenge.as_str()),
            ("code_challenge_method", "S256"),
        ];

        // 注意：tenant_id 不应该作为查询参数传递
        // 它已经包含在 auth_url 和 token_url 的路径中了
        // 例如：https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize

        // 5. 构建授权 URL
        let auth_url = format!(
            "{}?{}",
            oauth_config.auth_url,
            params
                .iter()
                .map(|(k, v)| format!("{}={}", k, url_encode(v)))
                .collect::<Vec<_>>()
                .join("&")
        );

        // 打印 OAuth 配置和授权 URL（用于调试）
        tracing::info!("========== OAuth 授权 URL 生成 ==========");
        tracing::info!("Provider: {}", provider.provider_id());
        tracing::info!("Client ID: {}", oauth_config.client_id);
        tracing::info!("Redirect URI: {}", oauth_config.redirect_uri);
        tracing::info!("State: {}", state);
        tracing::info!("Auth URL: {}", auth_url);
        tracing::info!("========================================");

        Ok(AuthorizationContext {
            auth_url,
            state,
            code_verifier,
            provider: provider.provider_id().to_string(),
        })
    }

    /// 交换授权码获取 Token
    ///
    /// # 参数
    ///
    /// * `provider` - 邮件服务商实现
    /// * `code` - OAuth 授权码
    /// * `state` - CSRF 防护令牌
    ///
    /// # 返回
    ///
    /// 返回 OAuth Token 响应
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let handler = OAuthHandler::new();
    /// let provider = GmailProvider;
    /// let token_response = handler.exchange_code(&provider, "auth_code", "state").await?;
    /// ```
    pub async fn exchange_code(
        &self,
        provider: &dyn MailProvider,
        code: &str,
        state: &str,
    ) -> Result<OAuthTokenResponse> {
        // 1. 获取 PKCE verifier
        let code_verifier = self
            .pkce_store
            .take(state)
            .ok_or(OAuthError::InvalidGrant)?;

        // 打印 verifier 信息（用于调试）
        tracing::info!("========== PKCE Verifier 检索 ==========");
        tracing::info!("State: {}", state);
        tracing::info!(
            "Verifier (前20字符): {}",
            &code_verifier.chars().take(20).collect::<String>()
        );
        tracing::info!("Verifier 长度: {}", code_verifier.len());
        tracing::info!("====================================");

        // 2. 获取 OAuth 配置
        let oauth_config = provider
            .oauth_config()
            .ok_or_else(|| OAuthError::NetworkError("服务商不支持 OAuth".to_string()))?;

        // 3. 构建请求体
        let mut params = vec![
            ("client_id", oauth_config.client_id.as_str()),
            ("code", code),
            ("redirect_uri", oauth_config.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", &code_verifier),
        ];

        // 如果有 client_secret，添加到参数中
        if let Some(ref client_secret) = oauth_config.client_secret {
            params.push(("client_secret", client_secret));
        }

        // 打印请求参数（用于调试）
        tracing::info!("========== OAuth Token 请求 ==========");
        tracing::info!("URL: {}", oauth_config.token_url);
        tracing::info!("Provider: {}", provider.provider_id());
        tracing::info!("Client ID: {}", oauth_config.client_id);
        tracing::info!(
            "Client Secret: {}",
            oauth_config
                .client_secret
                .as_ref()
                .map(|s| "***")
                .unwrap_or("None (Public Client)")
        );
        tracing::info!("Redirect URI: {}", oauth_config.redirect_uri);
        tracing::info!(
            "Code (前50字符): {}",
            &code.chars().take(50).collect::<String>()
        );
        tracing::info!("Code 长度: {}", code.len());
        tracing::info!("Verifier 长度: {}", code_verifier.len());
        tracing::info!("Grant Type: authorization_code");

        // 打印所有参数
        for (key, value) in &params {
            let value_preview = if *key == "code" || *key == "code_verifier" {
                format!(
                    "{} (长度: {})",
                    &value.chars().take(20).collect::<String>(),
                    value.len()
                )
            } else {
                value.to_string()
            };
            tracing::info!("  {}: {}", key, value_preview);
        }
        tracing::info!("====================================");

        // 构建表单数据用于调试
        let form_body = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, url_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        tracing::info!("实际表单数据: {}", form_body);

        // 4. 发送 HTTP POST 请求
        let client = reqwest::Client::new();
        let response = client
            .post(&oauth_config.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| OAuthError::NetworkError(e.to_string()))?;

        // 5. 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OAuthError::RefreshFailed(format!(
                "HTTP {}: {}",
                status.as_u16(),
                error_text
            ))
            .into());
        }

        // 6. 解析 JSON 响应
        let token_response: OAuthTokenResponse = response
            .json()
            .await
            .map_err(|e| OAuthError::RefreshFailed(format!("解析响应失败: {}", e)))?;

        tracing::info!("OAuth 授权码交换成功: provider={}", provider.provider_id());

        Ok(token_response)
    }

    /// 刷新 Token
    ///
    /// # 参数
    ///
    /// * `provider` - 邮件服务商实现
    /// * `refresh_token` - 刷新令牌
    ///
    /// # 返回
    ///
    /// 返回新的 OAuth Token 响应
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let handler = OAuthHandler::new();
    /// let provider = GmailProvider;
    /// let new_token = handler.refresh_token(&provider, "refresh_token").await?;
    /// ```
    pub async fn refresh_token(
        &self,
        provider: &dyn MailProvider,
        refresh_token: &str,
    ) -> Result<OAuthTokenResponse> {
        // 1. 获取 OAuth 配置
        let oauth_config = provider
            .oauth_config()
            .ok_or_else(|| OAuthError::NetworkError("服务商不支持 OAuth".to_string()))?;

        // 2. 构建刷新请求
        let mut params = vec![
            ("client_id", oauth_config.client_id.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        // 如果有 client_secret，添加到参数中
        if let Some(ref client_secret) = oauth_config.client_secret {
            params.push(("client_secret", client_secret));
        }

        // 3. 发送 HTTP POST 请求
        let client = reqwest::Client::new();
        let response = client
            .post(&oauth_config.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| OAuthError::RefreshFailed(e.to_string()))?;

        // 4. 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OAuthError::RefreshFailed(format!(
                "HTTP {}: {}",
                status.as_u16(),
                error_text
            ))
            .into());
        }

        // 5. 解析 JSON 响应
        let token_response: OAuthTokenResponse = response
            .json()
            .await
            .map_err(|e| OAuthError::RefreshFailed(format!("解析响应失败: {}", e)))?;

        tracing::info!("Token 刷新成功: provider={}", provider.provider_id());

        Ok(token_response)
    }
}

impl Default for OAuthHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::GmailProvider;

    #[test]
    fn test_oauth_handler_new() {
        let handler = OAuthHandler::new();
        // 通过创建成功来验证
        assert_eq!(OAuthHandler::CSRF_TOKEN_LENGTH, 32);
    }

    #[test]
    fn test_oauth_handler_default() {
        let handler = OAuthHandler::default();
        // 通过创建成功来验证
        assert_eq!(OAuthHandler::CSRF_TOKEN_LENGTH, 32);
    }

    #[tokio::test]
    async fn test_generate_authorization_url() {
        let handler = OAuthHandler::new();
        let provider = GmailProvider;

        let context = handler.get_authorization_url(&provider).await.unwrap();

        assert!(!context.auth_url.is_empty());
        assert!(!context.state.is_empty());
        assert!(!context.code_verifier.is_empty());
        assert_eq!(context.provider, "gmail");

        // 验证 URL 包含必需参数
        assert!(context.auth_url.contains("client_id="));
        assert!(context.auth_url.contains("response_type=code"));
        assert!(context.auth_url.contains("redirect_uri="));
        assert!(context.auth_url.contains("scope="));
        assert!(context.auth_url.contains("state="));
        assert!(context.auth_url.contains("code_challenge="));
        assert!(context.auth_url.contains("code_challenge_method=S256"));
    }

    #[tokio::test]
    async fn test_pkce_flow() {
        let handler = OAuthHandler::new();
        let provider = GmailProvider;

        // 生成授权 URL
        let context = handler.get_authorization_url(&provider).await.unwrap();

        // 验证 PKCE verifier 存储
        let retrieved = handler.pkce_store.take(&context.state);
        assert_eq!(retrieved, Some(context.code_verifier));

        // 取出后应该被删除
        let retrieved_again = handler.pkce_store.take(&context.state);
        assert_eq!(retrieved_again, None);
    }

    #[tokio::test]
    async fn test_csrf_token_length() {
        assert_eq!(OAuthHandler::CSRF_TOKEN_LENGTH, 32);
    }

    #[tokio::test]
    async fn test_authorization_context_fields() {
        let handler = OAuthHandler::new();
        let provider = GmailProvider;

        let context = handler.get_authorization_url(&provider).await.unwrap();

        // 验证所有字段都不为空
        assert!(!context.auth_url.is_empty());
        assert!(!context.state.is_empty());
        assert!(!context.code_verifier.is_empty());
        assert_eq!(context.provider, "gmail");
    }

    #[test]
    fn test_url_encode_function() {
        // 测试 URL 编码功能
        let input = "user@example.com";
        let encoded = url_encode(input);

        // 验证 @ 被编码
        assert!(encoded.contains("%40"));
        assert!(encoded.contains("user"));
        assert!(encoded.contains("example.com"));
    }
}
