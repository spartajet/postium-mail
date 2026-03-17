//! Token 管理器
//!
//! 管理 OAuth Token 的存储、刷新和过期检测

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;
use tokio::sync::RwLock;

use crate::crypto::{OAuthToken, KEYRING_SERVICE, oauth_username};
use crate::error::{MailError, OAuthError, Result, StorageError};

/// Token 元数据（内存缓存）
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    /// 账号 ID
    pub account_id: i32,
    /// 服务商 ID (e.g., "microsoft", "google")
    pub provider: String,
    /// Token 过期时间（Unix 时间戳）
    pub expires_at: i64,
    /// 刷新次数
    pub refresh_count: u32,
    /// 最后刷新时间（Unix 时间戳）
    pub last_refresh_at: i64,
}

/// Token 管理器
///
/// 负责 OAuth Token 的：
/// - 存储到 Keyring（只存储 refresh_token）
/// - 内存缓存元数据（快速查询）
/// - 过期检测
/// - 批量刷新查询
pub struct TokenManager {
    /// Tauri 应用句柄
    app_handle: Arc<AppHandle>,
    /// 内存缓存：account_id -> TokenMetadata
    cache: Arc<RwLock<HashMap<i32, TokenMetadata>>>,
    /// Token 过期检测阈值（秒），默认 300 秒（5 分钟）
    expiry_threshold: i64,
}

impl TokenManager {
    /// 默认过期阈值（秒）
    const DEFAULT_EXPIRY_THRESHOLD: i64 = 300; // 5 分钟

    /// 创建新的 TokenManager
    ///
    /// # 参数
    ///
    /// * `app_handle` - Tauri 应用句柄
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let manager = TokenManager::new(&app_handle)?;
    /// ```
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle: Arc::new(app_handle.clone()),
            cache: Arc::new(RwLock::new(HashMap::new())),
            expiry_threshold: Self::DEFAULT_EXPIRY_THRESHOLD,
        })
    }

    /// 创建带自定义过期阈值的 TokenManager
    ///
    /// # 参数
    ///
    /// * `app_handle` - Tauri 应用句柄
    /// * `expiry_threshold` - 过期检测阈值（秒）
    pub fn with_threshold(app_handle: &AppHandle, expiry_threshold: i64) -> Result<Self> {
        Ok(Self {
            app_handle: Arc::new(app_handle.clone()),
            cache: Arc::new(RwLock::new(HashMap::new())),
            expiry_threshold,
        })
    }

    /// 存储 OAuth Token（只存储 refresh_token）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `provider` - 服务商 ID (e.g., "microsoft", "google")
    /// * `refresh_token` - 刷新令牌
    /// * `expires_at` - 过期时间（Unix 时间戳）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let expires_at = Utc::now().timestamp() + 3600; // 1 小时后过期
    /// manager.store_oauth_token(1, "microsoft", "refresh_token_value", expires_at).await?;
    /// ```
    pub async fn store_oauth_token(
        &self,
        account_id: i32,
        provider: &str,
        refresh_token: &str,
        expires_at: i64,
    ) -> Result<()> {
        // 1. 构造 OAuthToken 结构
        let token = OAuthToken {
            refresh_token: refresh_token.to_string(),
            expires_at,
        };

        // 2. 序列化为 JSON
        let token_json = serde_json::to_string(&token)
            .map_err(|e| MailError::Internal(format!("序列化 token 失败: {}", e)))?;

        // 3. 存储到 Keyring
        let username = oauth_username(account_id);
        let keyring = self.app_handle.keyring();
        keyring
            .set_password(KEYRING_SERVICE, &username, &token_json)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?;

        // 4. 更新内存缓存
        let metadata = TokenMetadata {
            account_id,
            provider: provider.to_string(),
            expires_at,
            refresh_count: 0,
            last_refresh_at: Utc::now().timestamp(),
        };

        self.cache.write().await.insert(account_id, metadata);

        tracing::info!(
            "存储 OAuth token 成功: account_id={}, provider={}, expires_at={}",
            account_id,
            provider,
            expires_at
        );

        Ok(())
    }

    /// 获取 OAuth Token（从 Keyring 读取）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// 返回包含 refresh_token 和 expires_at 的 OAuthToken
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let token = manager.get_oauth_token(1).await?;
    /// println!("Token 过期时间: {}", token.expires_at);
    /// ```
    pub async fn get_oauth_token(&self, account_id: i32) -> Result<OAuthToken> {
        // 1. 从 Keyring 读取
        let username = oauth_username(account_id);
        let keyring = self.app_handle.keyring();
        let token_json = keyring
            .get_password(KEYRING_SERVICE, &username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?
            .ok_or_else(|| {
                MailError::Storage(StorageError::NotFound(format!(
                    "账号 {} 的 OAuth Token 不存在",
                    account_id
                )))
            })?;

        // 2. 反序列化
        let token: OAuthToken = serde_json::from_str(&token_json).map_err(|e| {
            MailError::Internal(format!("反序列化 token 失败: {}", e))
        })?;

        tracing::debug!("获取 OAuth token 成功: account_id={}", account_id);

        Ok(token)
    }

    /// 检查 Token 是否即将过期
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// - `true` - Token 即将过期（在阈值内）
    /// - `false` - Token 未过期或已过期
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// if manager.is_token_expiring_soon(1).await? {
    ///     // 刷新 token
    /// }
    /// ```
    pub async fn is_token_expiring_soon(&self, account_id: i32) -> Result<bool> {
        let now = Utc::now().timestamp();

        // 优先从缓存检查
        if let Some(metadata) = self.cache.read().await.get(&account_id) {
            let remaining = metadata.expires_at - now;
            return Ok(remaining < self.expiry_threshold && remaining > 0);
        }

        // 缓存未命中，从 Keyring 加载
        let metadata = self.refresh_cache(account_id).await?;
        let remaining = metadata.expires_at - now;
        Ok(remaining < self.expiry_threshold && remaining > 0)
    }

    /// 检查 Token 是否已过期
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// - `true` - Token 已过期
    /// - `false` - Token 未过期
    pub async fn is_token_expired(&self, account_id: i32) -> Result<bool> {
        let now = Utc::now().timestamp();

        // 优先从缓存检查
        if let Some(metadata) = self.cache.read().await.get(&account_id) {
            return Ok(metadata.expires_at <= now);
        }

        // 缓存未命中，从 Keyring 加载
        let metadata = self.refresh_cache(account_id).await?;
        Ok(metadata.expires_at <= now)
    }

    /// 更新 Token（刷新后调用）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `new_refresh_token` - 新的刷新令牌
    /// * `new_expires_at` - 新的过期时间
    pub async fn update_token(
        &self,
        account_id: i32,
        new_refresh_token: &str,
        new_expires_at: i64,
    ) -> Result<()> {
        // 1. 读取现有元数据以获取 provider 和 refresh_count
        let (provider, refresh_count) = {
            let cache = self.cache.read().await;
            if let Some(metadata) = cache.get(&account_id) {
                (
                    metadata.provider.clone(),
                    metadata.refresh_count + 1,
                )
            } else {
                // 缓存未命中，从 Keyring 加载
                let metadata = self.refresh_cache(account_id).await?;
                (metadata.provider, metadata.refresh_count + 1)
            }
        };

        // 2. 存储新 token
        self.store_oauth_token(account_id, &provider, new_refresh_token, new_expires_at)
            .await?;

        // 3. 更新刷新计数
        self.cache.write().await.entry(account_id).and_modify(|m| {
            m.refresh_count = refresh_count;
            m.last_refresh_at = Utc::now().timestamp();
        });

        tracing::info!(
            "更新 OAuth token 成功: account_id={}, refresh_count={}",
            account_id,
            refresh_count
        );

        Ok(())
    }

    /// 获取所有即将过期的账号（用于批量刷新）
    ///
    /// # 参数
    ///
    /// * `within_seconds` - 时间范围（秒）
    ///
    /// # 返回
    ///
    /// 返回即将过期账号 ID 的列表
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 获取 5 分钟内即将过期的账号
    /// let expiring = manager.get_expiring_accounts(300).await?;
    /// ```
    pub async fn get_expiring_accounts(&self, within_seconds: i64) -> Result<Vec<i32>> {
        let now = Utc::now().timestamp();
        let mut expiring = Vec::new();

        // 检查缓存中的所有账号
        let cache = self.cache.read().await;
        for (&account_id, metadata) in cache.iter() {
            let remaining = metadata.expires_at - now;
            if remaining > 0 && remaining <= within_seconds {
                expiring.push(account_id);
            }
        }

        tracing::debug!(
            "找到 {} 个即将过期的账号（{} 秒内）",
            expiring.len(),
            within_seconds
        );

        Ok(expiring)
    }

    /// 删除 Token（账号删除时调用）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    pub async fn delete_token(&self, account_id: i32) -> Result<()> {
        // 1. 从 Keyring 删除
        let username = oauth_username(account_id);
        let keyring = self.app_handle.keyring();
        keyring
            .delete_password(KEYRING_SERVICE, &username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?;

        // 2. 从缓存删除
        self.cache.write().await.remove(&account_id);

        tracing::info!("删除 OAuth token 成功: account_id={}", account_id);

        Ok(())
    }

    /// 刷新内存缓存（从 Keyring 重新加载）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// 返回更新后的 TokenMetadata
    pub async fn refresh_cache(&self, account_id: i32) -> Result<TokenMetadata> {
        // 1. 从 Keyring 读取 token
        let token = self.get_oauth_token(account_id).await?;

        // 2. 获取 provider（从缓存或使用默认值）
        let provider = {
            let cache = self.cache.read().await;
            cache.get(&account_id).map(|m| m.provider.clone())
        };

        // 3. 构造元数据
        let metadata = TokenMetadata {
            account_id,
            provider: provider.unwrap_or_else(|| "unknown".to_string()),
            expires_at: token.expires_at,
            refresh_count: 0,
            last_refresh_at: Utc::now().timestamp(),
        };

        // 4. 更新缓存
        self.cache.write().await.insert(account_id, metadata.clone());

        tracing::debug!("刷新缓存成功: account_id={}", account_id);

        Ok(metadata)
    }

    /// 获取 Token 元数据（优先从缓存）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// - `Some(metadata)` - 找到元数据
    /// - `None` - 账号不存在
    pub async fn get_metadata(&self, account_id: i32) -> Result<Option<TokenMetadata>> {
        // 优先从缓存检查
        if let Some(metadata) = self.cache.read().await.get(&account_id) {
            return Ok(Some(metadata.clone()));
        }

        // 缓存未命中，尝试从 Keyring 加载
        match self.refresh_cache(account_id).await {
            Ok(metadata) => Ok(Some(metadata)),
            Err(MailError::Storage(StorageError::NotFound(_))) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 设置过期阈值
    ///
    /// # 参数
    ///
    /// * `threshold` - 新的阈值（秒）
    pub fn set_expiry_threshold(&mut self, threshold: i64) {
        self.expiry_threshold = threshold;
        tracing::info!("设置过期阈值: {} 秒", threshold);
    }

    /// 获取过期阈值
    pub fn expiry_threshold(&self) -> i64 {
        self.expiry_threshold
    }

    /// 获取应用句柄
    pub fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_metadata_creation() {
        let metadata = TokenMetadata {
            account_id: 1,
            provider: "microsoft".to_string(),
            expires_at: 9999999999,
            refresh_count: 0,
            last_refresh_at: 1234567890,
        };

        assert_eq!(metadata.account_id, 1);
        assert_eq!(metadata.provider, "microsoft");
        assert_eq!(metadata.expires_at, 9999999999);
        assert_eq!(metadata.refresh_count, 0);
        assert_eq!(metadata.last_refresh_at, 1234567890);
    }

    #[test]
    fn test_expiry_threshold_default() {
        assert_eq!(TokenManager::DEFAULT_EXPIRY_THRESHOLD, 300);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let cache = Arc::new(RwLock::new(HashMap::new()));
        let now = Utc::now().timestamp();

        // 插入缓存
        let metadata = TokenMetadata {
            account_id: 1,
            provider: "google".to_string(),
            expires_at: now + 3600,
            refresh_count: 0,
            last_refresh_at: now,
        };

        cache.write().await.insert(1, metadata.clone());

        // 读取缓存
        let cache_guard = cache.read().await;
        let retrieved = cache_guard.get(&1).unwrap();

        assert_eq!(retrieved.account_id, 1);
        assert_eq!(retrieved.provider, "google");
        assert_eq!(retrieved.expires_at, now + 3600);
    }
}
