//! OAuth 客户端
//!
//! 编排 OAuth2 认证流程，协调协议处理、会话管理和 Token 存储

use std::sync::Arc;

use crate::auth::AuthResult;
use crate::auth::oauth2::OAuthHandler;
use crate::auth::token::TokenManager;
use crate::error::{MailError, Result};
use crate::providers::{AuthType, ProviderPool};

/// OAuth 客户端
///
/// 负责编排 OAuth2 认证流程的各个环节
pub struct OAuthClient {
    /// OAuth 处理器
    oauth_handler: Arc<OAuthHandler>,
    /// Token 管理器
    token_manager: Arc<TokenManager>,
    /// 服务商池
    provider_pool: Arc<ProviderPool>,
}

impl OAuthClient {
    /// 创建新的 OAuth 认证器
    pub fn new(
        oauth_handler: Arc<OAuthHandler>,
        token_manager: Arc<TokenManager>,
        provider_pool: Arc<ProviderPool>,
    ) -> Self {
        Self {
            oauth_handler,
            token_manager,
            provider_pool,
        }
    }

    /// 执行 OAuth 认证
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `auth_code` - OAuth 授权码
    /// * `state` - OAuth 状态参数
    ///
    /// # 返回
    ///
    /// 返回认证结果，包含用户信息、Token 信息等
    pub async fn authenticate(
        &self,
        email: &str,
        auth_code: &str,
        state: &str,
    ) -> Result<AuthResult> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;
        let provider_info = provider.provider_info();
        let provider_id = provider_info.id.clone();

        // 2. 交换授权码
        let token_response = self
            .oauth_handler
            .exchange_code(provider, auth_code, state)
            .await?;

        // 3. 计算 Token 过期时间
        let expires_at = if let Some(expires_in) = token_response.expires_in {
            Some(chrono::Utc::now().timestamp() + expires_in as i64)
        } else {
            // 默认 1 小时后过期
            Some(chrono::Utc::now().timestamp() + 3600)
        };

        // 4. 存储到 TokenManager
        // 注意：account_id 需要外部设置，这里先使用临时值
        let temp_account_id = 0;
        let refresh_token = token_response
            .refresh_token
            .ok_or_else(|| MailError::Internal("OAuth 响应缺少 refresh_token".to_string()))?;

        self.token_manager
            .store_oauth_token(
                temp_account_id,
                &provider_id,
                &refresh_token,
                expires_at.unwrap(),
            )
            .await?;

        // 5. 解析用户信息（从 id_token）
        let (user_email, display_name) = if let Some(ref id_token) = token_response.id_token {
            Self::parse_user_info_from_id_token(id_token)?
        } else {
            // 如果没有 id_token，使用传入的 email
            let name = email.split('@').next().unwrap_or("用户").to_string();
            (email.to_string(), name)
        };

        tracing::info!(
            "OAuth 认证成功: email={}, provider={}, display_name={}",
            user_email,
            provider_id,
            display_name
        );

        Ok(AuthResult {
            account_id: None,
            email: user_email,
            display_name: Some(display_name),
            auth_type: AuthType::OAuth2,
            provider: provider_id,
            expires_at,
            id_token: token_response.id_token,
        })
    }

    /// 从 ID Token 中解析用户信息
    ///
    /// # 参数
    ///
    /// * `id_token` - JWT 格式的 ID Token
    ///
    /// # 返回
    ///
    /// 返回 (email, display_name) 元组
    fn parse_user_info_from_id_token(id_token: &str) -> Result<(String, String)> {
        use base64::Engine;

        // ID Token 格式: header.payload.signature
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() < 2 {
            return Err(MailError::Internal("无效的 ID Token 格式".to_string()));
        }

        // 解码 payload
        let payload = parts[1];
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|e| MailError::Internal(format!("解码 ID Token 失败: {}", e)))?;

        let json_str = String::from_utf8(decoded)
            .map_err(|e| MailError::Internal(format!("ID Token 不是有效的 UTF-8: {}", e)))?;

        // 解析 JSON
        let claims: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| MailError::Internal(format!("解析 ID Token JSON 失败: {}", e)))?;

        // 提取 email 和 name
        let email = claims
            .get("email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MailError::Internal("ID Token 缺少 email 字段".to_string()))?
            .to_string();

        let display_name = claims
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&email)
            .to_string();

        tracing::debug!(
            "从 ID Token 解析用户信息: email={}, name={}",
            email,
            display_name
        );

        Ok((email, display_name))
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;

    use super::*;

    #[test]
    fn test_parse_user_info_from_id_token() {
        // 构造一个简单的测试 token（不包含签名）
        let payload = serde_json::json!({
            "email": "user@example.com",
            "name": "Test User",
            "iss": "https://accounts.google.com"
        });

        let payload_str = serde_json::to_string(&payload).unwrap();
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_str);

        let fake_token = format!("header.{}.signature", encoded);

        let result = OAuthClient::parse_user_info_from_id_token(&fake_token);
        assert!(result.is_ok());

        let (email, name) = result.unwrap();
        assert_eq!(email, "user@example.com");
        assert_eq!(name, "Test User");
    }
}
