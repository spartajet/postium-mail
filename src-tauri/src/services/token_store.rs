use anyhow::{anyhow, Result};
use crate::crypto::SecureVault;
use crate::services::oauth_service::{OAuthService, OAuthToken};

/// Token 存储服务
pub struct TokenStore {
    vault: SecureVault,
}

impl TokenStore {
    pub fn new(vault: SecureVault) -> Self {
        Self { vault }
    }

    /// 保存 OAuth Token 到 Stronghold
    pub async fn save_token(&self, account_id: i32, token: &OAuthToken) -> Result<()> {
        self.vault.store_token(account_id, token).await
    }

    /// 从 Stronghold 获取 OAuth Token
    pub async fn get_token(&self, account_id: i32) -> Result<Option<OAuthToken>> {
        self.vault.get_token(account_id).await
    }

    /// 获取有效 Token（自动刷新过期 Token）
    pub async fn get_valid_token(
        &self,
        account_id: i32,
        oauth_service: &OAuthService,
    ) -> Result<String> {
        let token = self.get_token(account_id).await?
            .ok_or_else(|| anyhow!("Token not found"))?;

        // 检查是否过期（提前 5 分钟刷新）
        let now = chrono::Utc::now().timestamp();
        if token.expires_at < now + 300 {
            // 即将过期，刷新 Token
            tracing::info!("Token 即将过期，正在刷新...");
            let new_token = oauth_service.refresh_microsoft_token(token.refresh_token).await?;
            self.save_token(account_id, &new_token).await?;
            tracing::info!("Token 刷新成功");
            return Ok(new_token.access_token);
        }

        Ok(token.access_token)
    }
}
