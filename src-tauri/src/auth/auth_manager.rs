//! 认证管理器
//!
//! 统一管理各种认证方式，协调各认证处理器

use std::sync::Arc;

use base64::Engine;
use tauri::AppHandle;

use crate::providers::{ProviderPool, AuthType};
use crate::error::{MailError, Result};
use crate::auth::oauth_handler::{OAuthHandler, AuthorizationContext};
use crate::auth::token_manager::TokenManager;
use crate::auth::password_auth::PasswordAuth;
use crate::auth::enterprise_auth::EnterpriseAuth;

/// 认证凭证
#[derive(Debug, Clone)]
pub enum AuthCredential {
    /// OAuth 授权码
    OAuthCode { code: String, state: String },
    /// 密码
    Password(String),
    /// 应用专用密码
    AppPassword(String),
}

/// 认证结果
#[derive(Debug, Clone)]
pub struct AuthResult {
    /// 账号 ID（需要外部设置）
    pub account_id: Option<i32>,
    /// 邮箱地址
    pub email: String,
    /// 显示名称
    pub display_name: Option<String>,
    /// 认证类型
    pub auth_type: AuthType,
    /// 服务商 ID
    pub provider: String,
    /// Token 过期时间（仅 OAuth）
    pub expires_at: Option<i64>,
    /// ID Token（仅 OAuth，包含用户信息）
    pub id_token: Option<String>,
}

/// IMAP 认证信息
#[derive(Debug, Clone)]
pub enum ImapAuthInfo {
    /// 密码认证
    Password { username: String, password: String },
    /// OAuth 认证
    OAuth { email: String, xoauth2: String },
}

/// SMTP 认证信息
#[derive(Debug, Clone)]
pub enum SmtpAuthInfo {
    /// 密码认证
    Password { username: String, password: String },
    /// OAuth 认证
    OAuth { email: String, xoauth2: String },
}

/// 认证状态
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// 已认证
    Authenticated,
    /// Token 即将过期
    ExpiringSoon,
    /// Token 已过期
    Expired,
    /// 认证失败
    Failed,
    /// 需要重新授权
    ReauthorizationRequired,
}

/// 认证管理器
///
/// 负责：
/// - 统一认证入口
/// - OAuth 认证流程
/// - 密码认证流程
/// - Token 刷新
/// - 批量刷新
pub struct AuthManager {
    /// OAuth 处理器
    oauth_handler: Arc<OAuthHandler>,
    /// Token 管理器
    token_manager: Arc<TokenManager>,
    /// 密码认证处理器
    password_auth: Arc<PasswordAuth>,
    /// 企业认证处理器
    enterprise_auth: Arc<EnterpriseAuth>,
    /// 服务商池
    provider_pool: Arc<ProviderPool>,
}

impl AuthManager {
    /// 创建新的认证管理器
    ///
    /// # 参数
    ///
    /// * `app_handle` - Tauri 应用句柄
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let manager = AuthManager::new(&app_handle)?;
    /// ```
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let provider_pool = Arc::new(ProviderPool::new());
        let oauth_handler = Arc::new(OAuthHandler::new());
        let token_manager = Arc::new(TokenManager::new(app_handle)?);
        let password_auth = Arc::new(PasswordAuth::new(app_handle)?);
        let enterprise_auth = Arc::new(EnterpriseAuth::new());

        Ok(Self {
            oauth_handler,
            token_manager,
            password_auth,
            enterprise_auth,
            provider_pool,
        })
    }

    /// 获取 OAuth 授权 URL
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// 返回授权上下文（包含授权 URL 和 state）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let context = manager.get_oauth_url("user@example.com").await?;
    /// // 在浏览器中打开 context.auth_url
    /// ```
    pub async fn get_oauth_url(&self, email: &str) -> Result<AuthorizationContext> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;

        // 2. 获取授权 URL
        let context = self
            .oauth_handler
            .get_authorization_url(provider.as_ref())
            .await?;

        tracing::info!("生成 OAuth 授权 URL: email={}", email);

        Ok(context)
    }

    /// OAuth 认证
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `auth_code` - OAuth 授权码
    /// * `state` - CSRF 防护令牌
    ///
    /// # 返回
    ///
    /// 返回认证结果（包含用户信息和 id_token）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let result = manager.authenticate_oauth("user@example.com", "code", "state").await?;
    /// ```
    pub async fn authenticate_oauth(
        &self,
        email: &str,
        auth_code: &str,
        state: &str,
    ) -> Result<AuthResult> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;
        let provider_id = provider.provider_id().to_string();

        // 2. 交换授权码
        let token_response = self
            .oauth_handler
            .exchange_code(provider.as_ref(), auth_code, state)
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
            self.get_user_info_from_id_token(id_token)?
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

    /// 密码认证
    ///
    /// # 参数
    ///
    /// * `email` - 邮箱地址
    /// * `password` - 密码
    ///
    /// # 返回
    ///
    /// 返回认证结果
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let result = manager.authenticate_password("user@example.com", "password").await?;
    /// ```
    pub async fn authenticate_password(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthResult> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;
        let provider_id = provider.provider_id().to_string();

        // 2. 验证密码（可选）
        // 暂时跳过实际验证，直接存储

        // 3. 存储密码
        // 注意：account_id 需要外部设置，这里先使用临时值
        let temp_account_id = 0;
        self.password_auth
            .store_password(temp_account_id, password)
            .await?;

        // 提取显示名称（从 email 的用户名部分）
        let display_name = email.split('@').next().map(|s| s.to_string());

        tracing::info!("密码认证成功: email={}, provider={}", email, provider_id);

        Ok(AuthResult {
            account_id: None,
            email: email.to_string(),
            display_name,
            auth_type: AuthType::Password,
            provider: provider_id,
            expires_at: None,
            id_token: None,
        })
    }

    /// 刷新 Token
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// - `Ok(())` - 刷新成功
    /// - `Err(_)` - 刷新失败
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// manager.refresh_token(1, "user@example.com").await?;
    /// ```
    pub async fn refresh_token(&self, account_id: i32, email: &str) -> Result<()> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;

        // 2. 获取当前的 refresh_token
        let token = self.token_manager.get_oauth_token(account_id).await?;

        // 3. 刷新 Token
        let new_token_response = self
            .oauth_handler
            .refresh_token(provider.as_ref(), &token.refresh_token)
            .await?;

        // 4. 计算新的过期时间
        let new_expires_at = if let Some(expires_in) = new_token_response.expires_in {
            chrono::Utc::now().timestamp() + expires_in as i64
        } else {
            // 默认 1 小时后过期
            chrono::Utc::now().timestamp() + 3600
        };

        // 5. 更新 Token
        let new_refresh_token = new_token_response
            .refresh_token
            .unwrap_or(token.refresh_token);

        self.token_manager
            .update_token(account_id, &new_refresh_token, new_expires_at)
            .await?;

        tracing::info!("Token 刷新成功: account_id={}, email={}", account_id, email);

        Ok(())
    }

    /// 验证凭证状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回认证状态
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let state = manager.validate_credentials(1, AuthType::OAuth2).await?;
    /// ```
    pub async fn validate_credentials(
        &self,
        account_id: i32,
        auth_type: &AuthType,
    ) -> Result<AuthState> {
        match auth_type {
            AuthType::OAuth2 => {
                // 检查 Token 是否过期
                if self.token_manager.is_token_expired(account_id).await? {
                    Ok(AuthState::Expired)
                } else if self
                    .token_manager
                    .is_token_expiring_soon(account_id)
                    .await?
                {
                    Ok(AuthState::ExpiringSoon)
                } else {
                    Ok(AuthState::Authenticated)
                }
            }
            AuthType::Password | AuthType::AppPassword => {
                // 密码认证无法检测状态
                Ok(AuthState::Authenticated)
            }
            AuthType::Auto => {
                // 自动检测
                Ok(AuthState::Authenticated)
            }
        }
    }

    /// 批量刷新即将过期的 Token
    ///
    /// # 参数
    ///
    /// * `accounts` - 账号 ID 和邮箱地址的映射
    ///
    /// # 返回
    ///
    /// 返回成功刷新的账号 ID 列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let accounts = vec![(1, "user1@example.com"), (2, "user2@example.com")];
    /// let refreshed = manager.refresh_expiring_tokens(accounts).await?;
    /// ```
    pub async fn refresh_expiring_tokens(
        &self,
        accounts: Vec<(i32, String)>,
    ) -> Result<Vec<i32>> {
        let mut refreshed = Vec::new();
        let total = accounts.len();

        // 获取即将过期的账号
        let expiring = self.token_manager.get_expiring_accounts(300).await?;

        for (account_id, email) in accounts {
            if expiring.contains(&account_id) {
                match self.refresh_token(account_id, &email).await {
                    Ok(_) => {
                        refreshed.push(account_id);
                    }
                    Err(e) => {
                        tracing::error!(
                            "Token 刷新失败: account_id={}, email={}, error={}",
                            account_id,
                            email,
                            e
                        );
                    }
                }
            }
        }

        tracing::info!("批量刷新完成: {}/{} 成功", refreshed.len(), total);

        Ok(refreshed)
    }

    /// 获取 IMAP 认证信息
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回 IMAP 认证信息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let auth_info = manager.get_imap_auth(1, "user@example.com", &AuthType::OAuth2).await?;
    /// ```
    pub async fn get_imap_auth(
        &self,
        account_id: i32,
        email: &str,
        auth_type: &AuthType,
    ) -> Result<ImapAuthInfo> {
        let auth_type = match auth_type {
            AuthType::Auto => &AuthType::OAuth2,
            other => other,
        };

        match auth_type {
            AuthType::OAuth2 => {
                // 1. 获取服务商（用于刷新 token）
                let provider = self
                    .provider_pool
                    .detect_provider(email)
                    .await
                    .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

                // 2. 获取 access_token（带自动刷新）
                let access_token = self
                    .token_manager
                    .get_access_token(account_id, || async {
                        // 缓存未命中或已过期，执行刷新

                        // 2.1 获取 refresh_token
                        let refresh_token = self
                            .token_manager
                            .get_oauth_token(account_id)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("获取 refresh_token 失败: {}", e))
                            })?;

                        // 2.2 刷新 token
                        let token_response = self
                            .oauth_handler
                            .refresh_token(provider.as_ref(), &refresh_token.refresh_token)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("刷新 access_token 失败: {}", e))
                            })?;

                        // 2.3 提取 access_token（注意：不是 Option）
                        Ok(token_response.access_token)
                    })
                    .await?;

                // 3. 生成 XOAUTH2 字符串
                let xoauth2 = self
                    .oauth_handler
                    .generate_xoauth2(email, &access_token);

                Ok(ImapAuthInfo::OAuth {
                    email: email.to_string(),
                    xoauth2,
                })
            }
            AuthType::Password | AuthType::AppPassword => {
                // 获取密码
                let password = self.password_auth.get_password(account_id).await?;

                Ok(ImapAuthInfo::Password {
                    username: email.to_string(),
                    password,
                })
            }
            AuthType::Auto => {
                // unreachable because we replaced Auto with OAuth2 above
                unreachable!()
            }
        }
    }

    /// 获取 SMTP 认证信息
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    /// * `auth_type` - 认证类型
    ///
    /// # 返回
    ///
    /// 返回 SMTP 认证信息
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let auth_info = manager.get_smtp_auth(1, "user@example.com", &AuthType::OAuth2).await?;
    /// ```
    pub async fn get_smtp_auth(
        &self,
        account_id: i32,
        email: &str,
        auth_type: &AuthType,
    ) -> Result<SmtpAuthInfo> {
        let auth_type = match auth_type {
            AuthType::Auto => &AuthType::OAuth2,
            other => other,
        };

        match auth_type {
            AuthType::OAuth2 => {
                // 1. 获取服务商（用于刷新 token）
                let provider = self
                    .provider_pool
                    .detect_provider(email)
                    .await
                    .map_err(|e| MailError::Internal(format!("检测服务商失败: {}", e)))?;

                // 2. 获取 access_token（带自动刷新）
                let access_token = self
                    .token_manager
                    .get_access_token(account_id, || async {
                        // 缓存未命中或已过期，执行刷新

                        // 2.1 获取 refresh_token
                        let refresh_token = self
                            .token_manager
                            .get_oauth_token(account_id)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("获取 refresh_token 失败: {}", e))
                            })?;

                        // 2.2 刷新 token
                        let token_response = self
                            .oauth_handler
                            .refresh_token(provider.as_ref(), &refresh_token.refresh_token)
                            .await
                            .map_err(|e| {
                                MailError::Internal(format!("刷新 access_token 失败: {}", e))
                            })?;

                        // 2.3 提取 access_token（注意：不是 Option）
                        Ok(token_response.access_token)
                    })
                    .await?;

                // 3. 生成 XOAUTH2 字符串
                let xoauth2 = self
                    .oauth_handler
                    .generate_xoauth2(email, &access_token);

                Ok(SmtpAuthInfo::OAuth {
                    email: email.to_string(),
                    xoauth2,
                })
            }
            AuthType::Password | AuthType::AppPassword => {
                // 获取密码
                let password = self.password_auth.get_password(account_id).await?;

                Ok(SmtpAuthInfo::Password {
                    username: email.to_string(),
                    password,
                })
            }
            AuthType::Auto => {
                // unreachable because we replaced Auto with OAuth2 above
                unreachable!()
            }
        }
    }

    /// 更新账号 ID（在创建账号后调用）
    ///
    /// # 参数
    ///
    /// * `temp_id` - 临时账号 ID
    /// * `new_id` - 新的账号 ID
    pub async fn update_account_id(&self, temp_id: i32, new_id: i32) -> Result<()> {
        // TODO: 实现账号 ID 更新逻辑
        // 需要迁移 Token 和密码存储
        tracing::info!("更新账号 ID: {} -> {}", temp_id, new_id);
        Ok(())
    }

    /// 获取服务商池
    pub fn provider_pool(&self) -> &ProviderPool {
        &self.provider_pool
    }

    /// 获取 Token 管理器
    pub fn token_manager(&self) -> &TokenManager {
        &self.token_manager
    }

    /// 获取密码认证处理器
    pub fn password_auth(&self) -> &PasswordAuth {
        &self.password_auth
    }

    /// 获取企业认证处理器
    pub fn enterprise_auth(&self) -> &EnterpriseAuth {
        &self.enterprise_auth
    }

    /// 从 JWT ID Token 中解析用户信息
    ///
    /// # 参数
    ///
    /// * `id_token` - JWT ID Token 字符串
    ///
    /// # 返回
    ///
    /// 返回 (email, display_name) 元组
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let (email, name) = manager.get_user_info_from_id_token(id_token).await?;
    /// ```
    pub fn get_user_info_from_id_token(&self, id_token: &str) -> Result<(String, String)> {
        // 分割 JWT
        let parts: Vec<&str> = id_token.split('.').collect();
        if parts.len() != 3 {
            return Err(MailError::Internal(format!(
                "无效的 JWT token 格式，期望3段，实际{}段",
                parts.len()
            )));
        }

        // 解码 payload
        let payload = parts.get(1).ok_or_else(|| {
            MailError::Internal("JWT token 缺少 payload".to_string())
        })?;

        let payload_json = self.base64_url_decode(payload)?;

        // 解析 JSON
        let claims: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| MailError::Internal(format!("解析 JWT payload 失败: {}", e)))?;

        // 提取 email（支持多种字段名）
        let email = claims
            .get("upn")      // Microsoft User Principal Name
            .or_else(|| claims.get("email"))
            .or_else(|| claims.get("unique_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown@example.com")
            .to_string();

        // 提取 display name
        let display_name = claims
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // 如果没有 name 字段，使用 email 的用户名部分
                email.split('@').next().unwrap_or("用户").to_string()
            });

        tracing::info!(
            "从 ID Token 解析用户信息: email={}, display_name={}",
            email,
            display_name
        );

        Ok((email, display_name))
    }

    /// Base64URL 解码（辅助方法）
    fn base64_url_decode(&self, input: &str) -> Result<String> {
        // 添加 padding
        let input_padded = if input.len().is_multiple_of(4) {
            input.to_string()
        } else {
            let padding = "=".repeat(4 - (input.len() % 4));
            format!("{}{}", input, padding)
        };

        // 转换为标准 base64
        let input_standard = input_padded.replace('-', "+").replace('_', "/");

        // 解码
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&input_standard)
            .map_err(|e| MailError::Internal(format!("Base64 解码失败: {}", e)))?;

        String::from_utf8(bytes)
            .map_err(|e| MailError::Internal(format!("UTF-8 转换失败: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    #[test]
    fn test_auth_manager_new() {
        // 注意：测试需要 Tauri AppHandle，这里只测试结构
        // 实际测试需要集成测试环境
        // TODO: 实现实际的集成测试
    }

    #[test]
    fn test_auth_credential_oauth_code() {
        let credential = AuthCredential::OAuthCode {
            code: "test_code".to_string(),
            state: "test_state".to_string(),
        };
        match credential {
            AuthCredential::OAuthCode { code, state } => {
                assert_eq!(code, "test_code");
                assert_eq!(state, "test_state");
            }
            _ => panic!("Unexpected credential type"),
        }
    }

    #[test]
    fn test_auth_credential_password() {
        let credential = AuthCredential::Password("test_password".to_string());
        match credential {
            AuthCredential::Password(pwd) => {
                assert_eq!(pwd, "test_password");
            }
            _ => panic!("Unexpected credential type"),
        }
    }

    #[test]
    fn test_auth_result_creation() {
        let result = AuthResult {
            account_id: Some(1),
            email: "user@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            auth_type: AuthType::OAuth2,
            provider: "gmail".to_string(),
            expires_at: Some(1234567890),
            id_token: Some("test_id_token".to_string()),
        };

        assert_eq!(result.account_id, Some(1));
        assert_eq!(result.email, "user@example.com");
        assert_eq!(result.display_name, Some("Test User".to_string()));
        assert_eq!(result.auth_type, AuthType::OAuth2);
        assert_eq!(result.provider, "gmail");
        assert_eq!(result.expires_at, Some(1234567890));
        assert_eq!(result.id_token, Some("test_id_token".to_string()));
    }

    #[test]
    fn test_imap_auth_info_password() {
        let auth_info = ImapAuthInfo::Password {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
        };

        match auth_info {
            ImapAuthInfo::Password { username, password } => {
                assert_eq!(username, "user@example.com");
                assert_eq!(password, "password");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_imap_auth_info_oauth() {
        let auth_info = ImapAuthInfo::OAuth {
            email: "user@example.com".to_string(),
            xoauth2: "test_xoauth2".to_string(),
        };

        match auth_info {
            ImapAuthInfo::OAuth { email, xoauth2 } => {
                assert_eq!(email, "user@example.com");
                assert_eq!(xoauth2, "test_xoauth2");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_smtp_auth_info_password() {
        let auth_info = SmtpAuthInfo::Password {
            username: "user@example.com".to_string(),
            password: "password".to_string(),
        };

        match auth_info {
            SmtpAuthInfo::Password { username, password } => {
                assert_eq!(username, "user@example.com");
                assert_eq!(password, "password");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_smtp_auth_info_oauth() {
        let auth_info = SmtpAuthInfo::OAuth {
            email: "user@example.com".to_string(),
            xoauth2: "test_xoauth2".to_string(),
        };

        match auth_info {
            SmtpAuthInfo::OAuth { email, xoauth2 } => {
                assert_eq!(email, "user@example.com");
                assert_eq!(xoauth2, "test_xoauth2");
            }
            _ => panic!("Unexpected auth info type"),
        }
    }

    #[test]
    fn test_auth_state_authenticated() {
        assert_eq!(AuthState::Authenticated, AuthState::Authenticated);
    }

    #[test]
    fn test_auth_state_expiring_soon() {
        assert_eq!(AuthState::ExpiringSoon, AuthState::ExpiringSoon);
    }

    #[test]
    fn test_auth_state_expired() {
        assert_eq!(AuthState::Expired, AuthState::Expired);
    }
}
