//! Token 刷新调度器
//!
//! 负责管理和调度 OAuth Token 的刷新操作

use std::sync::Arc;

use crate::auth::oauth_handler::OAuthHandler;
use crate::auth::token::TokenManager;
use crate::auth::AuthState;
use crate::error::{MailError, Result};
use crate::providers::{AuthType, MailProvider, ProviderPool};

/// Token 刷新器
///
/// 负责管理 OAuth Token 的生命周期，包括刷新和状态检查
pub struct TokenRefresher {
    /// OAuth 处理器
    oauth_handler: Arc<OAuthHandler>,
    /// Token 管理器
    token_manager: Arc<TokenManager>,
    /// 服务商池
    provider_pool: Arc<ProviderPool>,
}

impl TokenRefresher {
    /// 创建新的 Token 刷新器
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

    /// 刷新单个账号的 Token
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `email` - 邮箱地址
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误信息
    pub async fn refresh_token(&self, account_id: i32, email: &str) -> Result<()> {
        // 1. 检测服务商
        let provider = self.provider_pool.detect_provider(email).await?;

        // 2. 获取当前的 refresh_token
        let token = self.token_manager.get_oauth_token(account_id).await?;

        // 3. 刷新 Token
        let new_token_response = self
            .oauth_handler
            .refresh_token(provider, &token.refresh_token)
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
    pub async fn refresh_expiring_tokens(&self, accounts: Vec<(i32, String)>) -> Result<Vec<i32>> {
        let mut refreshed = Vec::new();
        let total = accounts.len();

        // 获取即将过期的账号（5分钟内）
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

    /// 检查并刷新所有即将过期的 Token
    ///
    /// # 参数
    ///
    /// * `accounts` - 所有需要检查的账号列表
    ///
    /// # 返回
    ///
    /// 返回刷新结果的统计信息（成功数，失败数）
    pub async fn refresh_all_needed(&self, accounts: Vec<(i32, String)>) -> Result<(usize, usize)> {
        let mut success_count = 0;
        let mut fail_count = 0;

        // 获取即将过期的账号（5分钟内）
        let expiring = self.token_manager.get_expiring_accounts(300).await?;

        for (account_id, email) in accounts {
            if expiring.contains(&account_id) {
                match self.refresh_token(account_id, &email).await {
                    Ok(_) => {
                        success_count += 1;
                    }
                    Err(e) => {
                        fail_count += 1;
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

        Ok((success_count, fail_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意：TokenRefresher 的功能测试需要完整的依赖项
    // 这里只进行编译测试，实际的功能测试应该在集成测试中进行
    #[test]
    fn test_token_refresher_compile() {
        // 这个测试只验证 TokenRefresher 能够正确编译
        // 实际的功能测试需要 mock 或集成测试环境
        assert!(true);
    }
}
