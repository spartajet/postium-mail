//! OAuth 相关类型定义
//!
//! 提供 OAuth2 流程中使用的公共数据结构

use serde::{Deserialize, Serialize};

/// OAuth Token 响应
///
/// 从 OAuth 服务器返回的 Token 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenResponse {
    /// 访问令牌
    pub access_token: String,
    /// Token 类型（通常是 "Bearer"）
    pub token_type: String,
    /// 过期时间（秒）
    pub expires_in: Option<u64>,
    /// 刷新令牌
    pub refresh_token: Option<String>,
    /// 授权范围
    pub scope: Option<String>,
    /// ID Token（OpenID Connect）
    pub id_token: Option<String>,
}

/// OAuth 授权上下文
///
/// 包含启动 OAuth 流程所需的所有信息
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_token_response_creation() {
        let response = OAuthTokenResponse {
            access_token: "test_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("test_refresh_token".to_string()),
            scope: Some("https://mail.google.com/".to_string()),
            id_token: None,
        };

        assert_eq!(response.access_token, "test_access_token");
        assert_eq!(response.token_type, "Bearer");
        assert_eq!(response.expires_in, Some(3600));
    }

    #[test]
    fn test_authorization_context_creation() {
        let context = AuthorizationContext {
            auth_url: "https://accounts.google.com/o/oauth2/auth".to_string(),
            state: "random_state_123".to_string(),
            code_verifier: "code_verifier_abc".to_string(),
            provider: "gmail".to_string(),
        };

        assert_eq!(context.provider, "gmail");
        assert!(!context.auth_url.is_empty());
        assert!(!context.state.is_empty());
    }
}
