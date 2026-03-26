//! OAuth 会话管理模块
//!
//! 管理 OAuth2 授权流程的会话状态，包括会话创建、验证、更新和清理。
//!
//! # 会话生命周期
//!
//! ```text
//! Pending → Authorized/Failed → Expired (自动清理)
//!    │            │              │
//!    │            │              └── 10分钟后自动过期
//!    │            └── 授权成功或失败
//!    └── 等待用户授权
//! ```
//!
//! # 使用示例
//!
//! ```rust,no_run
//! # use postium_mail::auth::oauth2::OAuthSessionManager;
//! # async fn example() -> anyhow::Result<()> {
//! let manager = OAuthSessionManager::new(600); // 10分钟超时
//!
//! // 创建会话
//! let session_id = manager.create_session("gmail", "random_state").await?;
//!
//! // 验证会话
//! let session = manager.verify_and_get_session("random_state").await?;
//!
//! // 更新状态
//! manager.update_session_status(&session_id, OAuthSessionStatus::Authorized).await?;
//! # Ok(())
//! # }
//! ```

use crate::error::{MailError, Result};
use sea_orm::prelude::Uuid;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 生成新的 UUID v4
fn generate_uuid() -> String {
    // 使用 rand 创建随机 UUID
    let rand1 = rand::random::<u32>();
    let rand2 = rand::random::<u16>();
    let rand3 = rand::random::<u16>();
    let rand4a = rand::random::<u16>();
    let rand4b = rand::random::<u32>();

    // UUID v4 格式: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    // 其中 y 是 8, 9, A, 或 B (表示版本和变体)
    let mut d4 = [0u8; 8];
    d4[0..2].copy_from_slice(&rand4a.to_be_bytes());
    d4[2..6].copy_from_slice(&rand4b.to_be_bytes());

    // 设置版本和变体位
    let mut d3 = rand3;
    d3 = (d3 & 0x0FFF) | 0x4000; // 版本 4
    let mut d4_0 = d4[0];
    d4_0 = (d4_0 & 0x3F) | 0x80; // 变体
    d4[0] = d4_0;

    let uuid = Uuid::from_fields(rand1, rand2, d3, &d4);
    uuid.to_string()
}

/// OAuth 会话状态
///
/// 表示授权流程中的不同阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OAuthSessionStatus {
    /// 等待用户授权
    Pending,
    /// 授权成功
    Authorized,
    /// 授权失败
    Failed,
    /// 会话已过期
    Expired,
}

/// OAuth 会话信息
///
/// 跟踪单个 OAuth 授权流程的状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSession {
    /// 会话唯一标识（UUID）
    pub session_id: String,
    /// 服务商标识（如 "gmail", "outlook"）
    pub provider: String,
    /// OAuth state 参数（用于 CSRF 防护）
    pub state: String,
    /// 邮箱地址
    pub email: String,
    /// 创建时间（Unix 时间戳，秒）
    pub created_at: i64,
    /// 过期时间（Unix 时间戳，秒）
    pub expires_at: i64,
    /// 当前状态
    pub status: OAuthSessionStatus,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 创建的账号 ID（授权成功后）
    pub account_id: Option<i32>,
}

impl OAuthSession {
    /// 检查会话是否已过期
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() >= self.expires_at
    }
}

/// OAuth 会话管理器
///
/// 管理所有活动的 OAuth 授权会话
pub struct OAuthSessionManager {
    /// 会话存储（内存存储，不持久化）
    sessions: Arc<RwLock<HashMap<String, OAuthSession>>>,
    /// 会话超时时间（秒）
    session_timeout: i64,
}

impl Default for OAuthSessionManager {
    fn default() -> Self {
        Self::new(600)
    }
}

impl OAuthSessionManager {
    /// 创建新的会话管理器
    ///
    /// # 参数
    ///
    /// - `session_timeout`: 会话超时时间（秒），默认 600 秒（10 分钟）
    pub fn new(session_timeout: i64) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_timeout,
        }
    }

    /// 创建新的 OAuth 会话
    ///
    /// # 参数
    ///
    /// - `provider`: 服务商标识（如 "gmail", "outlook"）
    /// - `email`: 邮箱地址
    /// - `state`: OAuth state 参数
    ///
    /// # 返回
    ///
    /// 返回会话 ID（UUID）
    pub async fn create_session(&self, provider: &str, email: &str, state: &str) -> Result<String> {
        let session_id = generate_uuid();
        let now = chrono::Utc::now().timestamp();

        let session = OAuthSession {
            session_id: session_id.clone(),
            provider: provider.to_string(),
            state: state.to_string(),
            email: email.to_string(),
            created_at: now,
            expires_at: now + self.session_timeout,
            status: OAuthSessionStatus::Pending,
            error: None,
            account_id: None,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);

        tracing::info!(
            "创建 OAuth 会话: session_id={}, provider={}",
            session_id,
            provider
        );

        Ok(session_id)
    }

    /// 验证 state 并获取会话
    ///
    /// # 参数
    ///
    /// - `state`: OAuth state 参数
    ///
    /// # 返回
    ///
    /// 返回会话信息，如果会话不存在或已过期则返回错误
    pub async fn verify_and_get_session(&self, state: &str) -> Result<OAuthSession> {
        let sessions = self.sessions.read().await;

        // 查找匹配的会话
        let session = sessions
            .values()
            .find(|s| s.state == state)
            .ok_or_else(|| MailError::Internal(format!("无效的 OAuth state: {}", state)))?;

        // 检查是否过期
        if session.is_expired() {
            return Err(MailError::Internal("OAuth 会话已过期".to_string()));
        }

        Ok(session.clone())
    }

    /// 根据 session_id 获取会话
    ///
    /// # 参数
    ///
    /// - `session_id`: 会话 ID
    pub async fn get_session(&self, session_id: &str) -> Result<OAuthSession> {
        let sessions = self.sessions.read().await;

        sessions
            .get(session_id)
            .cloned()
            .ok_or_else(|| MailError::Internal(format!("会话不存在: {}", session_id)))
    }

    /// 更新会话状态
    ///
    /// # 参数
    ///
    /// - `session_id`: 会话 ID
    /// - `status`: 新状态
    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: OAuthSessionStatus,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(session_id) {
            session.status = status;
            tracing::info!(
                "更新 OAuth 会话状态: session_id={}, status={:?}",
                session_id,
                status
            );
            Ok(())
        } else {
            Err(MailError::Internal(format!("会话不存在: {}", session_id)))
        }
    }

    /// 设置会话错误
    ///
    /// # 参数
    ///
    /// - `session_id`: 会话 ID
    /// - `error`: 错误信息
    pub async fn set_session_error(&self, session_id: &str, error: String) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(session_id) {
            session.error = Some(error);
            session.status = OAuthSessionStatus::Failed;
            Ok(())
        } else {
            Err(MailError::Internal(format!("会话不存在: {}", session_id)))
        }
    }

    /// 设置会话账号 ID
    ///
    /// # 参数
    ///
    /// - `session_id`: 会话 ID
    /// - `account_id`: 创建的账号 ID
    pub async fn set_session_account_id(&self, session_id: &str, account_id: i32) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        if let Some(session) = sessions.get_mut(session_id) {
            session.account_id = Some(account_id);
            session.status = OAuthSessionStatus::Authorized;
            Ok(())
        } else {
            Err(MailError::Internal(format!("会话不存在: {}", session_id)))
        }
    }

    /// 清理过期的会话
    ///
    /// # 返回
    ///
    /// 返回清理的会话数量
    pub async fn cleanup_expired(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let now = chrono::Utc::now().timestamp();

        let before_count = sessions.len();
        sessions.retain(|_, session| session.expires_at > now);
        let after_count = sessions.len();

        let cleaned = before_count - after_count;
        if cleaned > 0 {
            tracing::info!("清理过期 OAuth 会话: {} 个", cleaned);
        }

        cleaned
    }

    /// 删除指定会话
    ///
    /// # 参数
    ///
    /// - `session_id`: 会话 ID
    pub async fn remove_session(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;

        sessions
            .remove(session_id)
            .ok_or_else(|| MailError::Internal(format!("会话不存在: {}", session_id)))
            .map(|_| ())
    }

    /// 获取所有活动会话数量
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// 启动后台清理任务
    ///
    /// 每分钟清理一次过期会话
    pub fn spawn_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                self.cleanup_expired().await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_verify_session() {
        let manager = OAuthSessionManager::new(600);

        // 创建会话
        let session_id = manager
            .create_session("gmail", "test@example.com", "test_state_123")
            .await
            .unwrap();

        // 验证会话
        let session = manager
            .verify_and_get_session("test_state_123")
            .await
            .unwrap();

        assert_eq!(session.provider, "gmail");
        assert_eq!(session.state, "test_state_123");
        assert_eq!(session.status, OAuthSessionStatus::Pending);
        assert!(!session.is_expired());
    }

    #[tokio::test]
    async fn test_invalid_state() {
        let manager = OAuthSessionManager::new(600);

        let result = manager.verify_and_get_session("invalid_state").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_session_status() {
        let manager = OAuthSessionManager::new(600);

        let session_id = manager
            .create_session("outlook", "test@example.com", "state_456")
            .await
            .unwrap();

        // 更新为已授权
        manager
            .update_session_status(&session_id, OAuthSessionStatus::Authorized)
            .await
            .unwrap();

        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, OAuthSessionStatus::Authorized);
    }

    #[tokio::test]
    async fn test_set_session_error() {
        let manager = OAuthSessionManager::new(600);

        let session_id = manager
            .create_session("gmail", "test@example.com", "state_789")
            .await
            .unwrap();

        manager
            .set_session_error(&session_id, "用户拒绝授权".to_string())
            .await
            .unwrap();

        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, OAuthSessionStatus::Failed);
        assert_eq!(session.error, Some("用户拒绝授权".to_string()));
    }

    #[tokio::test]
    async fn test_set_session_account_id() {
        let manager = OAuthSessionManager::new(600);

        let session_id = manager
            .create_session("gmail", "test@example.com", "state_abc")
            .await
            .unwrap();

        manager
            .set_session_account_id(&session_id, 123)
            .await
            .unwrap();

        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.status, OAuthSessionStatus::Authorized);
        assert_eq!(session.account_id, Some(123));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let manager = OAuthSessionManager::new(1); // 1秒超时

        let session_id = manager
            .create_session("gmail", "test@example.com", "state_exp")
            .await
            .unwrap();

        // 等待过期
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 清理过期会话
        let cleaned = manager.cleanup_expired().await;
        assert_eq!(cleaned, 1);

        // 会话应该不存在
        let result = manager.get_session(&session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_session() {
        let manager = OAuthSessionManager::new(600);

        let session_id = manager
            .create_session("gmail", "test@example.com", "state_remove")
            .await
            .unwrap();

        manager.remove_session(&session_id).await.unwrap();

        let result = manager.get_session(&session_id).await;
        assert!(result.is_err());
    }
}
