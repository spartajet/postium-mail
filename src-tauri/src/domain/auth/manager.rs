use crate::domain::auth::token_cache::TokenCache;

use crate::domain::providers::AuthType;
use crate::error::MailError;
use crate::infrastructure::auth::oauth2::OAuth2Manager;
use std::sync::Arc;
use std::sync::OnceLock;

/// 凭证类型
pub enum Credentials {
    Password(String),
    OAuth2 { access_token: String },
}

/// 认证管理器 — 管理密码和 OAuth 认证
pub struct AuthManager {
    token_cache: TokenCache,
    oauth2_manager: OnceLock<Arc<OAuth2Manager>>,
}
impl Default for AuthManager {
    fn default() -> Self {
        Self {
            token_cache: TokenCache::new(),
            oauth2_manager: OnceLock::new(),
        }
    }
}

impl AuthManager {
    /// 注入 OAuth2 笡理器（在 lib.rs 初始化时调用一次）
    pub fn set_oauth2_manager(&self, manager: Arc<OAuth2Manager>) {
        let _ = self.oauth2_manager.set(manager);
    }

    /// 统一凭证获取 — 根据 auth_type 返回凭证
    pub async fn get_credentials(
        &self,
        email: &str,
        auth_type: &AuthType,
        oauth_provider: Option<&str>,
    ) -> Result<Credentials, MailError> {
        match auth_type {
            AuthType::OAuth2 => {
                let provider_id = oauth_provider
                    .ok_or_else(|| MailError::OAuth2Error("缺少 oauth_provider".into()))?;

                // 1. 尝试从内存缓存取 access_token
                if let Some(token) = self.token_cache.get(email) {
                    tracing::debug!(email, "使用缓存的 access_token");
                    return Ok(Credentials::OAuth2 {
                        access_token: token,
                    });
                }

                // 2. 缓存未命中/过期 → 从 Keyring 取 refresh_token
                let refresh_token = self.get_password(email)?;

                // 3. 刷新 access_token
                let manager = self
                    .oauth2_manager
                    .get()
                    .ok_or_else(|| MailError::OAuth2Error("OAuth2Manager 未初始化".into()))?;

                tracing::info!(email, provider_id, "刷新 access_token");
                let token_result = manager.refresh_token(provider_id, &refresh_token).await?;
                tracing::info!("access_token 刷新成功: {:?}", token_result);
                // 4. 若返回新 refresh_token，更新 Keyring
                if let Some(new_rt) = &token_result.refresh_token
                    && new_rt != &refresh_token
                {
                    tracing::info!(email, "refresh_token 已轮换，更新 Keyring");
                    self.save_password(email, new_rt)?;
                }

                // 5. 缓存新 access_token
                let expires_at = if let Some(expires_in) = token_result.expires_in {
                    chrono::Utc::now().timestamp() + expires_in
                } else {
                    // 默认 3600 秒
                    chrono::Utc::now().timestamp() + 3600
                };
                self.token_cache
                    .store(email, token_result.access_token.clone(), expires_at);
                tracing::info!(expires_at, "access_token 已缓存");

                Ok(Credentials::OAuth2 {
                    access_token: token_result.access_token,
                })
            }
            AuthType::Password => {
                // Password 模式
                let password = self.get_password(email)?;
                Ok(Credentials::Password(password))
            }
        }
    }

    /// 缓存 access_token（创建账号后立即调用）
    pub fn cache_access_token(&self, email: &str, access_token: &str, expires_in: i64) {
        let expires_at = chrono::Utc::now().timestamp() + expires_in;
        self.token_cache
            .store(email, access_token.to_string(), expires_at);
    }

    /// 从 Keyring 获取密码
    pub fn get_password(&self, email: &str) -> Result<String, MailError> {
        tracing::debug!(email, "Keyring: 获取密码");
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        let password = entry.get_password().map_err(|e| {
            tracing::warn!(email, error = %e, "Keyring: 获取密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;
        tracing::debug!(email, "Keyring: 密码获取成功");
        Ok(password)
    }

    /// 保存密码到 Keyring
    pub fn save_password(&self, email: &str, password: &str) -> Result<(), MailError> {
        tracing::debug!(email, "Keyring: 保存密码");
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        entry.set_password(password).map_err(|e| {
            tracing::error!(email, error = %e, "Keyring: 保存密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;
        tracing::debug!(email, "Keyring: 密码保存成功");
        Ok(())
    }

    /// 删除 Keyring 中的密码
    pub fn delete_password(&self, email: &str) -> Result<(), MailError> {
        tracing::debug!(email, "Keyring: 删除密码");
        let service = "postium-mail";
        let entry = keyring::Entry::new(service, email)
            .map_err(|e| MailError::KeyringError(format!("{}", e)))?;
        entry.delete_credential().map_err(|e| {
            tracing::warn!(email, error = %e, "Keyring: 删除密码失败");
            MailError::KeyringError(format!("{}", e))
        })?;
        tracing::debug!(email, "Keyring: 密码删除成功");
        Ok(())
    }
}
