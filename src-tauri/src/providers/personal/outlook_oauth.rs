//! Microsoft Outlook OAuth 服务
//!
//! 基于 OAuth 2.0 和 PKCE 的 Outlook 认证实现

use crate::error::{MailError, Result};
use oauth2::{
    basic::BasicClient, AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenUrl,
};
use serde::{Deserialize, Serialize};
use serde_json;

/// Outlook OAuth 服务
pub struct OutlookOAuthService {
    client_id: String,
    tenant: String,
    redirect_uri: String,
    scopes: Vec<String>,
}

impl OutlookOAuthService {
    /// 从配置创建新的 OAuth 服务
    pub fn new(
        client_id: String,
        tenant: String,
        redirect_uri: String,
        scopes: Vec<String>,
    ) -> Result<Self> {
        if client_id.is_empty() {
            return Err(MailError::Internal(
                "MICROSOFT_CLIENT_ID 未设置".to_string(),
            ));
        }

        Ok(Self {
            client_id,
            tenant,
            redirect_uri,
            scopes,
        })
    }

    /// 从环境变量加载配置并创建服务
    pub fn from_env() -> Result<Self> {
        let config = crate::config::load_microsoft_oauth_config()
            .map_err(|e| MailError::Internal(format!("加载 OAuth 配置失败: {}", e)))?;
        Ok(Self {
            client_id: config.client_id,
            tenant: config.tenant,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
        })
    }

    /// 获取授权 URL
    pub fn get_auth_url(&self, state: &str) -> Result<(String, String)> {
        // 构建 Microsoft OAuth 端点
        let auth_url_str = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
            self.tenant
        );
        let token_url_str = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant
        );

        let auth_url = AuthUrl::new(auth_url_str)
            .map_err(|e| MailError::Internal(format!("无效的授权端点: {}", e)))?;

        let token_url = TokenUrl::new(token_url_str)
            .map_err(|e| MailError::Internal(format!("无效的令牌端点: {}", e)))?;

        // 创建 OAuth 客户端（v5 API 只需要 client_id）
        let client = BasicClient::new(ClientId::new(self.client_id.clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(
                RedirectUrl::new(self.redirect_uri.clone())
                    .map_err(|e| MailError::Internal(format!("无效的重定向 URI: {}", e)))?,
            );

        // 生成 PKCE code verifier 和 challenge
        use rand::distributions::Alphanumeric;
        use rand::Rng;

        let code_verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(128)
            .map(char::from)
            .collect();

        // 使用 oauth_utils 中的方法创建 code_challenge
        let code_challenge_str =
            crate::providers::oauth_utils::PkceVerifierStore::create_code_challenge(&code_verifier);

        // 存储 verifier（使用全局存储）
        use crate::providers::oauth_utils::PkceVerifierStore;
        use once_cell::sync::Lazy;

        static VERIFIER_STORE: Lazy<PkceVerifierStore> = Lazy::new(PkceVerifierStore::new);

        VERIFIER_STORE
            .generate_and_store(state)
            .map_err(|e| MailError::Internal(format!("存储 verifier 失败: {}", e)))?;

        // 构建 scope
        let scopes = self
            .scopes
            .iter()
            .map(|s| Scope::new(s.clone()))
            .collect::<Vec<_>>();

        // 创建 PkceCodeChallenge（v5 API 使用 from_code_verifier_sha256）
        let pkce_verifier_for_challenge = PkceCodeVerifier::new(code_verifier.clone());
        let code_challenge =
            PkceCodeChallenge::from_code_verifier_sha256(&pkce_verifier_for_challenge);

        // 生成授权 URL（v5 API：闭包不接受参数）
        let (auth_url, csrf_token) = client
            .authorize_url(|| CsrfToken::new(state.to_string()))
            .add_scopes(scopes)
            .set_pkce_challenge(code_challenge)
            .url();

        Ok((auth_url.to_string(), csrf_token.secret().clone()))
    }

    /// 交换授权码获取 token
    pub async fn exchange_code(&self, code: &str, state: &str) -> Result<OutlookTokenResponse> {
        // 构建 Microsoft OAuth 端点
        let auth_url_str = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
            self.tenant
        );
        let token_url_str = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant
        );

        let auth_url = AuthUrl::new(auth_url_str.clone())
            .map_err(|e| MailError::Internal(format!("无效的授权端点: {}", e)))?;

        let token_url = TokenUrl::new(token_url_str.clone())
            .map_err(|e| MailError::Internal(format!("无效的令牌端点: {}", e)))?;

        // 创建 OAuth 客户端
        let client = BasicClient::new(ClientId::new(self.client_id.clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(
                RedirectUrl::new(self.redirect_uri.clone())
                    .map_err(|e| MailError::Internal(format!("无效的重定向 URI: {}", e)))?,
            );

        // 获取存储的 verifier
        use crate::providers::oauth_utils::PkceVerifierStore;
        use once_cell::sync::Lazy;

        static VERIFIER_STORE: Lazy<PkceVerifierStore> = Lazy::new(PkceVerifierStore::new);

        let code_verifier = VERIFIER_STORE
            .take(state)
            .ok_or_else(|| MailError::Internal("未找到 PKCE verifier，可能已过期".to_string()))?;

        // 将 code 转换为 AuthorizationCode
        use oauth2::AuthorizationCode;
        let code = AuthorizationCode::new(code.to_string());

        // 交换 token（使用自定义 HTTP 客户端）
        let pkce_verifier = PkceCodeVerifier::new(code_verifier);

        // 创建 HTTP 客户端
        let http_client = reqwest::Client::new();

        // 手动发送 token 请求（使用表单编码）
        let mut request_params = std::collections::HashMap::new();
        request_params.insert("grant_type".to_string(), "authorization_code".to_string());
        request_params.insert("code".to_string(), code.secret().clone());
        request_params.insert("redirect_uri".to_string(), self.redirect_uri.clone());
        request_params.insert("client_id".to_string(), self.client_id.clone());
        request_params.insert("code_verifier".to_string(), pkce_verifier.secret().clone());

        let response = http_client
            .post(token_url_str)
            .form(&request_params)
            .send()
            .await
            .map_err(|e| MailError::Internal(format!("交换 token 请求失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(MailError::Internal(format!(
                "交换 token 失败: {}",
                response.status()
            )));
        }

        // 解析响应
        let token_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| MailError::Internal(format!("解析 token 响应失败: {}", e)))?;

        let access_token = token_response["access_token"]
            .as_str()
            .ok_or_else(|| MailError::Internal("缺少 access_token".to_string()))?
            .to_string();

        let refresh_token = token_response["refresh_token"]
            .as_str()
            .map(|s| s.to_string());
        let expires_in = token_response["expires_in"].as_u64();

        Ok(OutlookTokenResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in,
            refresh_token,
            scope: self.scopes.join(" "),
            id_token: token_response["id_token"].as_str().map(|s| s.to_string()),
        })
    }

    /// 刷新 access token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<OutlookTokenResponse> {
        let client = reqwest::Client::new();
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant
        );

        let mut params = std::collections::HashMap::new();
        params.insert("client_id".to_string(), self.client_id.clone());
        params.insert("grant_type".to_string(), "refresh_token".to_string());
        params.insert("refresh_token".to_string(), refresh_token.to_string());

        let response = client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| MailError::Internal(format!("刷新 token 请求失败: {}", e)))?;

        if !response.status().is_success() {
            return Err(MailError::Internal(format!(
                "刷新 token 失败: {}",
                response.status()
            )));
        }

        let token_response: OutlookTokenResponse = response
            .json()
            .await
            .map_err(|e| MailError::Internal(format!("解析 token 响应失败: {}", e)))?;

        Ok(token_response)
    }
}

/// Outlook Token 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlookTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: String,
    pub id_token: Option<String>,
}

impl OutlookTokenResponse {
    /// 检查 token 是否即将过期（5分钟内）
    pub fn is_expiring_soon(&self) -> bool {
        if let Some(expires_in) = self.expires_in {
            // 简单检查：如果 token 已经使用了超过 55 分钟（假设有效期 1 小时）
            expires_in < 300 // 5 分钟
        } else {
            false
        }
    }

    /// 从 JSON 字符串解析
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| MailError::Internal(format!("解析 token JSON 失败: {}", e)))
    }

    /// 序列化为 JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| MailError::Internal(format!("序列化 token 失败: {}", e)))
    }

    /// 生成 XOAUTH2 认证字符串
    pub fn to_xoauth2(&self, email: &str) -> String {
        crate::providers::oauth_utils::generate_xoauth2_string(email, &self.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outlook_oauth_service_new() {
        let service = OutlookOAuthService::new(
            "test-client-id".to_string(),
            "common".to_string(),
            "postium-mail://oauth/callback".to_string(),
            vec!["scope1".to_string(), "scope2".to_string()],
        );

        assert!(service.is_ok());
        let service = service.unwrap();
        assert_eq!(service.client_id, "test-client-id");
        assert_eq!(service.tenant, "common");
    }

    #[test]
    fn test_outlook_oauth_service_empty_client_id() {
        let service = OutlookOAuthService::new(
            "".to_string(),
            "common".to_string(),
            "postium-mail://oauth/callback".to_string(),
            vec![],
        );

        assert!(service.is_err());
    }

    #[test]
    fn test_outlook_token_response_expiring() {
        // 即将过期的 token（300 秒 = 5 分钟）
        let expiring = OutlookTokenResponse {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(200),
            refresh_token: None,
            scope: "scope".to_string(),
            id_token: None,
        };

        assert!(expiring.is_expiring_soon());

        // 未过期的 token
        let not_expiring = OutlookTokenResponse {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: "scope".to_string(),
            id_token: None,
        };

        assert!(!not_expiring.is_expiring_soon());
    }

    #[test]
    fn test_outlook_token_json() {
        let original = OutlookTokenResponse {
            access_token: "test_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("test_refresh_token".to_string()),
            scope: "scope1 scope2".to_string(),
            id_token: None,
        };

        let json = original.to_json().unwrap();
        let parsed = OutlookTokenResponse::from_json(&json).unwrap();

        assert_eq!(parsed.access_token, original.access_token);
        assert_eq!(parsed.refresh_token, original.refresh_token);
    }

    #[test]
    fn test_outlook_token_to_xoauth2() {
        let token = OutlookTokenResponse {
            access_token: "test_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: None,
            scope: "scope".to_string(),
            id_token: None,
        };

        let xoauth2 = token.to_xoauth2("user@example.com");

        // 验证是 base64 编码
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;

        assert!(xoauth2
            .chars()
            .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));

        // 验证解码后包含正确的信息
        let decoded = BASE64.decode(&xoauth2).unwrap();
        let decoded_str = String::from_utf8(decoded).unwrap();
        assert!(decoded_str.contains("user=user@example.com"));
        assert!(decoded_str.contains("auth=Bearer test_token"));
    }
}
