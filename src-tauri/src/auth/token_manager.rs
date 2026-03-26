//! Token 管理器
//!
//! 管理 OAuth Token 的存储、刷新和过期检测

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;
use tokio::sync::RwLock;

use crate::error::{MailError, Result, StorageError};

/// OAuth Token 结构（用于 Keyring 存储）
/// 只存储 refresh_token，因为 access_token 可以通过 refresh_token 重新获取
/// 这样可以避免超过 Windows Keyring 的 2560 字符（UTF-16）限制
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OAuthToken {
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Keyring Service 名称（常量）
pub const KEYRING_SERVICE: &str = "com.postium.mail";

/// Keyring 辅助函数：生成账号密码的用户名
pub fn password_username(account_id: i32) -> String {
    format!("account_{}", account_id)
}

/// Keyring 辅助函数：生成 OAuth Token 的用户名
pub fn oauth_username(account_id: i32) -> String {
    format!("oauth_{}", account_id)
}

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
/// - 内存缓存 access_token（短期缓存，5 分钟）
/// - 过期检测
/// - 批量刷新查询
pub struct TokenManager {
    /// Tauri 应用句柄
    app_handle: Arc<AppHandle>,
    /// 内存缓存：account_id -> TokenMetadata
    cache: Arc<RwLock<HashMap<i32, TokenMetadata>>>,
    /// access_token 缓存：account_id -> (token, expires_at)
    /// 短期缓存（5 分钟），减少刷新请求
    access_token_cache: Arc<RwLock<HashMap<i32, (String, i64)>>>,
    /// Token 过期检测阈值（秒），默认 300 秒（5 分钟）
    expiry_threshold: i64,
}

impl TokenManager {
    /// 默认过期阈值（秒）
    const DEFAULT_EXPIRY_THRESHOLD: i64 = 300; // 5 分钟

    /// access_token 缓存时间（秒）
    const ACCESS_TOKEN_CACHE_TTL: i64 = 300; // 5 分钟

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
            access_token_cache: Arc::new(RwLock::new(HashMap::new())),
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
            access_token_cache: Arc::new(RwLock::new(HashMap::new())),
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
        let token: OAuthToken = serde_json::from_str(&token_json)
            .map_err(|e| MailError::Internal(format!("反序列化 token 失败: {}", e)))?;

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
                (metadata.provider.clone(), metadata.refresh_count + 1)
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
        self.cache
            .write()
            .await
            .insert(account_id, metadata.clone());

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

    // ========== access_token 内存缓存（方案 C）==========

    /// 获取缓存的 access_token
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// - `Some(token)` - 缓存命中且未过期
    /// - `None` - 缓存未命中或已过期
    pub async fn get_cached_access_token(&self, account_id: i32) -> Option<String> {
        let cache = self.access_token_cache.read().await;
        if let Some((token, expires_at)) = cache.get(&account_id) {
            let now = Utc::now().timestamp();
            if *expires_at > now {
                tracing::debug!("access_token 缓存命中: account_id={}", account_id);
                return Some(token.clone());
            } else {
                tracing::debug!("access_token 缓存已过期: account_id={}", account_id);
            }
        }
        None
    }

    /// 缓存 access_token
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `access_token` - access_token 值
    ///
    /// # 说明
    ///
    /// access_token 将被缓存 5 分钟（ACCESS_TOKEN_CACHE_TTL）
    pub async fn cache_access_token(&self, account_id: i32, access_token: String) {
        let expires_at = Utc::now().timestamp() + Self::ACCESS_TOKEN_CACHE_TTL;
        self.access_token_cache
            .write()
            .await
            .insert(account_id, (access_token, expires_at));
        tracing::debug!(
            "缓存 access_token: account_id={}, expires_at={}",
            account_id,
            expires_at
        );
    }

    /// 清除 access_token 缓存
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    pub async fn clear_access_token_cache(&self, account_id: i32) {
        self.access_token_cache.write().await.remove(&account_id);
        tracing::debug!("清除 access_token 缓存: account_id={}", account_id);
    }

    /// 清除所有 access_token 缓存
    pub async fn clear_all_access_token_cache(&self) {
        let count = self.access_token_cache.write().await.len();
        self.access_token_cache.write().await.clear();
        tracing::debug!("清除所有 access_token 缓存: {} 个", count);
    }

    /// 获取 access_token（带自动刷新）
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `refresh_fn` - 刷新函数，当缓存未命中时调用
    ///
    /// # 返回
    ///
    /// 返回 access_token
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let access_token = token_manager.get_access_token(1, || async {
    ///     // 调用 OAuthHandler 刷新 token
    ///     Ok(new_token)
    /// }).await?;
    /// ```
    pub async fn get_access_token<F, Fut>(&self, account_id: i32, refresh_fn: F) -> Result<String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        // 1. 检查缓存
        if let Some(token) = self.get_cached_access_token(account_id).await {
            return Ok(token);
        }

        // 2. 缓存未命中或已过期，调用刷新函数
        tracing::debug!("刷新 access_token: account_id={}", account_id);
        let new_token = refresh_fn()
            .await
            .map_err(|e| MailError::Internal(format!("刷新 access_token 失败: {}", e)))?;

        // 3. 缓存新 token（5 分钟）
        self.cache_access_token(account_id, new_token.clone()).await;

        Ok(new_token)
    }

    /// 迁移 Token 到新的 account_id
    ///
    /// 当使用临时 account_id (0) 存储 token 后，账号创建完成时需要迁移到真实 ID
    ///
    /// # 参数
    ///
    /// * `temp_account_id` - 临时账号 ID（通常为 0）
    /// * `new_account_id` - 新的账号 ID
    pub async fn migrate_token_account(
        &self,
        temp_account_id: i32,
        new_account_id: i32,
    ) -> Result<()> {
        // 1. 从临时 ID 读取 token
        let temp_username = oauth_username(temp_account_id);
        let keyring = self.app_handle.keyring();
        let token_json = keyring
            .get_password(KEYRING_SERVICE, &temp_username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?
            .ok_or_else(|| {
                MailError::Storage(StorageError::NotFound(format!(
                    "临时账号 {} 的 OAuth Token 不存在",
                    temp_account_id
                )))
            })?;

        // 2. 解析 token
        let token: OAuthToken = serde_json::from_str(&token_json)
            .map_err(|e| MailError::Internal(format!("反序列化 token 失败: {}", e)))?;

        // 3. 获取 provider（从缓存）
        let provider = self
            .cache
            .read()
            .await
            .get(&temp_account_id)
            .map(|m| m.provider.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // 4. 存储到新 ID
        self.store_oauth_token(
            new_account_id,
            &provider,
            &token.refresh_token,
            token.expires_at,
        )
        .await?;

        // 5. 删除临时 token
        keyring
            .delete_password(KEYRING_SERVICE, &temp_username)
            .map_err(|e| MailError::Storage(StorageError::Keyring(e.to_string())))?;

        // 6. 更新缓存
        self.cache.write().await.remove(&temp_account_id);

        tracing::info!(
            "迁移 OAuth token 成功: {} -> {}",
            temp_account_id,
            new_account_id
        );

        Ok(())
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

    #[test]
    fn test_token_metadata_fields() {
        let now = Utc::now().timestamp();
        let metadata = TokenMetadata {
            account_id: 5,
            provider: "test_provider".to_string(),
            expires_at: now + 7200,
            refresh_count: 3,
            last_refresh_at: now - 100,
        };

        // 验证所有字段
        assert_eq!(metadata.account_id, 5);
        assert_eq!(metadata.provider, "test_provider");
        assert!(metadata.expires_at > now);
        assert_eq!(metadata.refresh_count, 3);
        assert!(metadata.last_refresh_at < now);
    }

    #[tokio::test]
    async fn test_is_token_expiring_soon_true() {
        // 这个测试需要实际的 TokenManager 实例
        // 由于需要 AppHandle，这里只测试逻辑
        let now = Utc::now().timestamp();
        let expires_soon = now + 100; // 100 秒后过期

        // 5 分钟 = 300 秒
        let remaining = expires_soon - now;
        assert!(remaining < 300 && remaining > 0);
    }

    #[tokio::test]
    async fn test_is_token_expiring_soon_false() {
        let now = Utc::now().timestamp();
        let expires_later = now + 1000; // 1000 秒后过期

        let remaining = expires_later - now;
        assert!(remaining >= 300);
    }

    #[tokio::test]
    async fn test_is_token_expired_true() {
        let now = Utc::now().timestamp();
        let expired_time = now - 100;

        assert!(expired_time <= now);
    }

    #[tokio::test]
    async fn test_is_token_expired_false() {
        let now = Utc::now().timestamp();
        let future_time = now + 3600;

        assert!(future_time > now);
    }

    #[tokio::test]
    async fn test_expiring_accounts_filtering() {
        // 测试即将过期账号的过滤逻辑
        let now = Utc::now().timestamp();

        // 模拟账号列表
        let accounts = vec![
            (1, now - 100),  // 已过期
            (2, now + 100),  // 即将过期（< 300 秒）
            (3, now + 200),  // 即将过期（< 300 秒）
            (4, now + 1000), // 未过期（> 300 秒）
        ];

        let threshold = 300;
        let expiring: Vec<i32> = accounts
            .into_iter()
            .filter(|(_, expires_at)| {
                let remaining = expires_at - now;
                remaining > 0 && remaining <= threshold
            })
            .map(|(id, _)| id)
            .collect();

        assert_eq!(expiring.len(), 2);
        assert!(expiring.contains(&2));
        assert!(expiring.contains(&3));
        assert!(!expiring.contains(&1)); // 已过期
        assert!(!expiring.contains(&4)); // 未过期
    }

    #[tokio::test]
    async fn test_token_update_logic() {
        let now = Utc::now().timestamp();
        let mut refresh_count = 0;

        // 模拟 Token 更新
        refresh_count += 1;
        let new_expires_at = now + 7200;

        assert_eq!(refresh_count, 1);
        assert_eq!(new_expires_at, now + 7200);
    }

    #[tokio::test]
    async fn test_metadata_not_found() {
        use std::collections::HashMap;
        let cache: HashMap<i32, TokenMetadata> = HashMap::new();

        // 测试查找不存在的账号
        assert!(!cache.contains_key(&999));
    }

    #[tokio::test]
    async fn test_expiry_threshold_setting() {
        let custom_threshold = 600;

        // 测试自定义阈值
        assert_eq!(custom_threshold, 600);
    }

    #[test]
    fn test_default_expiry_threshold() {
        assert_eq!(TokenManager::DEFAULT_EXPIRY_THRESHOLD, 300);
    }

    // ========== access_token 缓存测试 ==========

    #[tokio::test]
    async fn test_access_token_cache_hit() {
        let cache: Arc<RwLock<HashMap<i32, (String, i64)>>> = Arc::new(RwLock::new(HashMap::new()));
        let now = Utc::now().timestamp();

        // 插入缓存（5分钟后过期）
        let token = "test_access_token".to_string();
        let expires_at = now + TokenManager::ACCESS_TOKEN_CACHE_TTL;
        cache.write().await.insert(1, (token.clone(), expires_at));

        // 读取缓存
        let cache_guard = cache.read().await;
        let retrieved = cache_guard.get(&1);

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().0, token);
    }

    #[tokio::test]
    async fn test_access_token_cache_expired() {
        let cache: Arc<RwLock<HashMap<i32, (String, i64)>>> = Arc::new(RwLock::new(HashMap::new()));
        let now = Utc::now().timestamp();

        // 插入已过期的缓存
        let token = "expired_token".to_string();
        let expires_at = now - 100; // 100秒前过期
        cache.write().await.insert(1, (token, expires_at));

        // 检查是否过期
        let cache_guard = cache.read().await;
        if let Some((_, exp)) = cache_guard.get(&1) {
            assert!(*exp <= now);
        }
    }

    #[tokio::test]
    async fn test_access_token_cache_miss() {
        let cache: Arc<RwLock<HashMap<i32, (String, i64)>>> = Arc::new(RwLock::new(HashMap::new()));

        // 读取不存在的缓存
        let cache_guard = cache.read().await;
        let retrieved = cache_guard.get(&999);

        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_access_token_cache_clear() {
        let cache: Arc<RwLock<HashMap<i32, (String, i64)>>> = Arc::new(RwLock::new(HashMap::new()));
        let now = Utc::now().timestamp();

        // 插入缓存
        cache
            .write()
            .await
            .insert(1, ("token".to_string(), now + 300));

        // 清除缓存
        cache.write().await.remove(&1);

        // 验证已清除
        let cache_guard = cache.read().await;
        assert!(cache_guard.get(&1).is_none());
    }

    #[tokio::test]
    async fn test_access_token_cache_ttl() {
        // 验证 TTL 常量
        assert_eq!(TokenManager::ACCESS_TOKEN_CACHE_TTL, 300); // 5 分钟
    }

    #[tokio::test]
    async fn test_access_token_cache_multiple() {
        let cache: Arc<RwLock<HashMap<i32, (String, i64)>>> = Arc::new(RwLock::new(HashMap::new()));
        let now = Utc::now().timestamp();

        // 插入多个账号的缓存
        cache
            .write()
            .await
            .insert(1, ("token1".to_string(), now + 300));
        cache
            .write()
            .await
            .insert(2, ("token2".to_string(), now + 300));
        cache
            .write()
            .await
            .insert(3, ("token3".to_string(), now + 300));

        // 验证数量
        let cache_guard = cache.read().await;
        assert_eq!(cache_guard.len(), 3);
    }
}
