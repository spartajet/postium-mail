//! 缓存管理模块
//!
//! 提供多级内存缓存策略，用于减少数据库查询和提高响应速度。
//!
//! # 核心功能
//!
//! - **通用缓存**: 键值对内存缓存，支持 TTL 和容量限制
//! - **文件夹缓存**: 缓存账号的文件夹列表
//! - **邮件内容缓存**: 缓存邮件正文内容
//! - **计数器缓存**: 缓存未读数等统计数据
//!
//! # 缓存策略
//!
//! ## TTL 配置
//!
//! | 缓存类型 | TTL | 容量 | 用途 |
//! |----------|-----|------|------|
//! | FolderListCache | 5 分钟 | 100 项 | 文件夹列表 |
//! | EmailContentCache | 10 分钟 | 500 项 | 邮件内容 |
//! | CounterCache | 1 分钟 | 100 项 | 未读数等统计 |
//!
//! ## 淘汰策略
//!
//! 当缓存达到容量上限时，采用简单的 FIFO 淘汰策略：
//!
//! ```text
//! 插入新项时：
//! 1. 检查容量
//! 2. 如果已满且键不存在，移除一个旧项
//! 3. 插入新项
//! ```
//!
//! # 数据结构
//!
//! ## CacheItem
//!
//! 每个缓存项包含：
//!
//! ```rust
//! struct CacheItem<T> {
//!     value: T,                // 缓存值
//!     expires_at: Option<Instant>,  // 过期时间
//! }
//! ```
//!
//! # 使用示例
//!
//! ## 基本使用
//!
//! ```rust,no_run
//! # use crate::storage::cache::MemoryCache;
//! # async fn example() {
//! let cache = MemoryCache::new();
//!
//! // 设置缓存
//! cache.set("key1".to_string(), "value1".to_string()).await;
//!
//! // 获取缓存
//! if let Some(value) = cache.get(&"key1".to_string()).await {
//!     println!("缓存命中: {}", value);
//! }
//!
//! // 删除缓存
//! cache.remove(&"key1".to_string()).await;
//! # }
//! ```
//!
//! ## 带配置的缓存
//!
//! ```rust,no_run
//! # use crate::storage::cache::MemoryCache;
//! # use std::time::Duration;
//! # async fn example() {
//! // 10 秒 TTL，最多 100 项
//! let cache = MemoryCache::with_config(
//!     Some(Duration::from_secs(10)),
//!     Some(100),
//! );
//! # }
//! ```
//!
//! ## CacheManager
//!
//! ```rust,no_run
//! # use crate::storage::cache::CacheManager;
//! # async fn example() {
//! let manager = CacheManager::new();
//!
//! // 缓存文件夹列表
//! manager.folders.set(1, folders_vec).await;
//!
//! // 缓存邮件内容
//! manager.email_contents.set("email:123", body_html).await;
//!
//! // 缓存未读数
//! manager.counters.set("unread:1", 10).await;
//!
//! // 定期清理过期项
//! manager.cleanup_all().await;
//! # }
//! ```
//!
//! # 缓存失效
//!
//! ## 主动失效
//!
//! ```rust,no_run
//! # use crate::storage::cache::CacheManager;
//! # async fn example(manager: &CacheManager, account_id: i32) {
//! // 新邮件到达时，清除相关缓存
//! manager.counters.remove(&format!("unread:{}", account_id)).await;
//!
//! // 文件夹变更时，清除文件夹缓存
//! manager.folders.remove(&account_id).await;
//! # }
//! ```
//!
//! ## 自动过期
//!
//! 缓存在读取时自动检查过期：
//!
//! ```text
//! 读取 → 检查过期时间 → 已过期返回 None → 未过期返回值
//! ```
//!
//! # 性能考虑
//!
//! - **内存占用**: 缓存项占用内存，应根据实际情况调整容量
//! - **并发访问**: 使用 `Arc<RwLock<>>` 保证线程安全
//! - **清理频率**: 建议定期调用 `cleanup_expired` 清理过期项
//! - **命中率**: 监控缓存命中率以优化 TTL 和容量配置

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 缓存项
#[derive(Clone)]
struct CacheItem<T> {
    value: T,
    expires_at: Option<Instant>,
}

impl<T> CacheItem<T> {
    /// 创建新的缓存项
    fn new(value: T, ttl: Option<Duration>) -> Self {
        Self {
            value,
            expires_at: ttl.map(|d| Instant::now() + d),
        }
    }

    /// 检查是否过期
    fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => Instant::now() > exp,
            None => false,
        }
    }
}

/// 内存缓存
pub struct MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    data: Arc<RwLock<HashMap<K, CacheItem<V>>>>,
    ttl: Option<Duration>,
    max_size: usize,
}

impl<K, V> MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    /// 创建新的内存缓存
    pub fn new() -> Self {
        Self::with_config(None, None)
    }

    /// 创建带配置的内存缓存
    pub fn with_config(ttl: Option<Duration>, max_size: Option<usize>) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            ttl,
            max_size: max_size.unwrap_or(1000),
        }
    }

    /// 获取缓存值
    pub async fn get(&self, key: &K) -> Option<V> {
        let data = self.data.read().await;
        data.get(key).and_then(|item| {
            if item.is_expired() {
                None
            } else {
                Some(item.value.clone())
            }
        })
    }

    /// 设置缓存值
    pub async fn set(&self, key: K, value: V) {
        let mut data = self.data.write().await;

        // 如果超过最大大小，删除最旧的项（简单策略：删除第一个）
        if data.len() >= self.max_size && !data.contains_key(&key) {
            // 简单的清理策略：移除一个项
            let key_to_remove = data.keys().next().cloned();
            if let Some(k) = key_to_remove {
                data.remove(&k);
            }
        }

        data.insert(key, CacheItem::new(value, self.ttl));
    }

    /// 删除缓存值
    pub async fn remove(&self, key: &K) {
        let mut data = self.data.write().await;
        data.remove(key);
    }

    /// 清空所有缓存
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        data.clear();
    }

    /// 获取缓存大小
    pub async fn size(&self) -> usize {
        let data = self.data.read().await;
        data.len()
    }

    /// 清理过期项
    pub async fn cleanup_expired(&self) {
        let mut data = self.data.write().await;
        data.retain(|_, item| !item.is_expired());
    }
}

impl<K, V> Default for MemoryCache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

/// 邮箱列表缓存
pub type FolderListCache = MemoryCache<i32, Vec<crate::protocols::imap::FolderInfo>>;

/// 邮件内容缓存
pub type EmailContentCache = MemoryCache<String, String>;

/// 计数器缓存（用于未读邮件数等）
pub type CounterCache = MemoryCache<String, i64>;

/// 缓存管理器
pub struct CacheManager {
    pub folders: FolderListCache,
    pub email_contents: EmailContentCache,
    pub counters: CounterCache,
}

impl CacheManager {
    /// 创建新的缓存管理器
    pub fn new() -> Self {
        // 文件夹列表缓存：5分钟 TTL
        let folders = MemoryCache::with_config(
            Some(Duration::from_secs(300)),
            Some(100),
        );

        // 邮件内容缓存：10分钟 TTL
        let email_contents = MemoryCache::with_config(
            Some(Duration::from_secs(600)),
            Some(500),
        );

        // 计数器缓存：1分钟 TTL
        let counters = MemoryCache::with_config(
            Some(Duration::from_secs(60)),
            Some(100),
        );

        Self {
            folders,
            email_contents,
            counters,
        }
    }

    /// 清理所有过期缓存
    pub async fn cleanup_all(&self) {
        self.folders.cleanup_expired().await;
        self.email_contents.cleanup_expired().await;
        self.counters.cleanup_expired().await;
    }

    /// 清空所有缓存
    pub async fn clear_all(&self) {
        self.folders.clear().await;
        self.email_contents.clear().await;
        self.counters.clear().await;
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_cache_basic() {
        let cache = MemoryCache::new();

        cache.set("key1".to_string(), "value1".to_string()).await;
        let value = cache.get(&"key1".to_string()).await;

        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_memory_cache_expiration() {
        // 100ms TTL
        let cache = MemoryCache::with_config(Some(Duration::from_millis(100)), None);

        cache.set("key1".to_string(), "value1".to_string()).await;

        // 立即获取应该成功
        assert!(cache.get(&"key1".to_string()).await.is_some());

        // 等待过期
        tokio::time::sleep(Duration::from_millis(150)).await;

        // 过期后应该返回 None
        assert!(cache.get(&"key1".to_string()).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_cache_remove() {
        let cache = MemoryCache::new();

        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.remove(&"key1".to_string()).await;

        assert!(cache.get(&"key1".to_string()).await.is_none());
    }

    #[tokio::test]
    async fn test_memory_cache_clear() {
        let cache = MemoryCache::new();

        cache.set("key1".to_string(), "value1".to_string()).await;
        cache.set("key2".to_string(), "value2".to_string()).await;

        cache.clear().await;

        assert_eq!(cache.size().await, 0);
    }

    #[tokio::test]
    async fn test_cache_manager() {
        let manager = CacheManager::new();

        // 测试计数器缓存
        manager.counters.set("unread:1".to_string(), 10).await;
        let count = manager.counters.get(&"unread:1".to_string()).await;

        assert_eq!(count, Some(10));
    }
}
