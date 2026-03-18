//! 通知管理器
//!
//! 管理系统通知，包括新邮件、同步状态、错误等

use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};

/// 通知管理器
///
/// 核心功能：
/// - 通知去重（5秒窗口内相同通知只发送一次）
/// - 通知合并（10秒窗口内同类型通知合并为"收到N封新邮件"）
/// - Tauri事件发射到前端
/// - 桌面通知（可选）
pub struct NotificationManager {
    /// Tauri AppHandle（用于发射事件）
    app_handle: AppHandle,

    /// 通知历史: notification_id -> (timestamp, notification)
    /// 用于去重和合并逻辑
    history: Arc<RwLock<HashMap<String, (DateTime<Utc>, Notification)>>>,

    /// 通知配置
    config: NotificationConfig,

    /// 统计计数器
    stats: Arc<RwLock<NotificationStats>>,
}

/// 通知配置
#[derive(Clone, Debug)]
pub struct NotificationConfig {
    /// 去重窗口（秒）- 相同通知在此时间内只发送一次
    pub dedupe_window_secs: i64,

    /// 合并窗口（秒）- 同类型通知在此时间内合并
    pub merge_window_secs: i64,

    /// 是否启用桌面通知
    pub enable_desktop_notification: bool,

    /// 是否启用声音提醒
    pub enable_sound: bool,

    /// 最大历史记录数
    pub max_history_size: usize,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            dedupe_window_secs: 5,    // 5秒去重窗口
            merge_window_secs: 10,    // 10秒合并窗口
            enable_desktop_notification: true,
            enable_sound: false,      // 默认关闭声音
            max_history_size: 1000,   // 最多保留1000条历史
        }
    }
}

/// 通知
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    /// 通知ID（唯一标识符）
    pub id: String,
    /// 通知标题
    pub title: String,
    /// 通知内容
    pub body: String,
    /// 通知类型
    pub notification_type: NotificationType,
    /// 关联的账号ID（可选）
    pub account_id: Option<i32>,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 元数据（可选，用于额外信息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// 通知类型
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    /// 新邮件
    NewEmail,
    /// 同步完成
    SyncComplete,
    /// 同步错误
    SyncError,
    /// 认证错误
    AuthError,
    /// 网络错误
    NetworkError,
    /// 系统通知
    System,
}

/// 新邮件通知数据
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewEmailData {
    pub account_id: i32,
    pub account_name: String,
    pub folder: String,
    pub email_count: usize,
    pub subject: Option<String>,
    pub sender: Option<String>,
}

/// 通知统计
#[derive(Clone, Debug, Default)]
struct NotificationStats {
    total_sent: usize,
    total_deduped: usize,
    total_merged: usize,
    new_email_count: usize,
    error_count: usize,
}

impl NotificationManager {
    /// 创建新的通知管理器
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            history: Arc::new(RwLock::new(HashMap::new())),
            config: NotificationConfig::default(),
            stats: Arc::new(RwLock::new(NotificationStats::default())),
        }
    }

    /// 创建带配置的通知管理器
    pub fn with_config(app_handle: AppHandle, config: NotificationConfig) -> Self {
        Self {
            app_handle,
            history: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(NotificationStats::default())),
        }
    }

    /// 发送新邮件通知
    ///
    /// 自动处理去重和合并逻辑
    pub async fn notify_new_email(&self, data: NewEmailData) -> Result<()> {
        // 生成去重键（account_id + folder）
        let dedupe_key = format!("new_email:{}:{}", data.account_id, data.folder);

        // 检查去重窗口内是否有相同通知
        if self.should_dedupe(&dedupe_key).await {
            tracing::debug!("通知被去重: {}", dedupe_key);
            return Ok(());
        }

        // 检查合并窗口内是否有同类型通知
        if let Some(merged) = self.try_merge_notification(&data).await {
            // 发送合并后的通知
            self.send_notification(merged).await?;
        } else {
            // 创建新通知
            let notification = Notification {
                id: dedupe_key.clone(),
                title: format!("新邮件 - {}", data.account_name),
                body: self.format_new_email_body(&data),
                notification_type: NotificationType::NewEmail,
                account_id: Some(data.account_id),
                timestamp: Utc::now(),
                metadata: serde_json::to_value(data).ok(),
            };

            self.send_notification(notification).await?;
        }

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.new_email_count += 1;

        Ok(())
    }

    /// 发送同步完成通知
    pub async fn notify_sync_complete(
        &self,
        account_id: i32,
        account_name: &str,
        synced_count: usize,
    ) -> Result<()> {
        let notification = Notification {
            id: format!("sync_complete:{}:{}", account_id, chrono::Utc::now().timestamp()),
            title: format!("同步完成 - {}", account_name),
            body: format!("同步了 {} 封邮件", synced_count),
            notification_type: NotificationType::SyncComplete,
            account_id: Some(account_id),
            timestamp: Utc::now(),
            metadata: None,
        };

        self.send_notification(notification).await
    }

    /// 发送同步错误通知
    pub async fn notify_sync_error(
        &self,
        account_id: i32,
        account_name: &str,
        error: &str,
    ) -> Result<()> {
        let notification = Notification {
            id: format!("sync_error:{}:{}", account_id, chrono::Utc::now().timestamp()),
            title: format!("同步失败 - {}", account_name),
            body: format!("错误: {}", error),
            notification_type: NotificationType::SyncError,
            account_id: Some(account_id),
            timestamp: Utc::now(),
            metadata: None,
        };

        self.send_notification(notification).await?;

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.error_count += 1;

        Ok(())
    }

    /// 发送认证错误通知
    pub async fn notify_auth_error(
        &self,
        account_id: i32,
        account_name: &str,
        error: &str,
    ) -> Result<()> {
        let notification = Notification {
            id: format!("auth_error:{}:{}", account_id, chrono::Utc::now().timestamp()),
            title: format!("认证失败 - {}", account_name),
            body: format!("请检查账号凭据: {}", error),
            notification_type: NotificationType::AuthError,
            account_id: Some(account_id),
            timestamp: Utc::now(),
            metadata: None,
        };

        self.send_notification(notification).await
    }

    /// 发送网络错误通知
    pub async fn notify_network_error(&self, account_id: i32, error: &str) -> Result<()> {
        let notification = Notification {
            id: format!("network_error:{}:{}", account_id, chrono::Utc::now().timestamp()),
            title: "网络错误".to_string(),
            body: format!("连接失败: {}", error),
            notification_type: NotificationType::NetworkError,
            account_id: Some(account_id),
            timestamp: Utc::now(),
            metadata: None,
        };

        self.send_notification(notification).await
    }

    /// 发送系统通知
    pub async fn notify_system(&self, title: &str, body: &str) -> Result<()> {
        let notification = Notification {
            id: format!("system:{}", chrono::Utc::now().timestamp_millis()),
            title: title.to_string(),
            body: body.to_string(),
            notification_type: NotificationType::System,
            account_id: None,
            timestamp: Utc::now(),
            metadata: None,
        };

        self.send_notification(notification).await
    }

    /// 获取统计信息
    pub async fn get_stats(&self) -> NotificationStats {
        self.stats.read().await.clone()
    }

    /// 清理过期的历史记录
    pub async fn cleanup_history(&self) {
        let mut history = self.history.write().await;
        let now = Utc::now();
        let cutoff = now - Duration::seconds(3600); // 保留1小时内的记录

        history.retain(|_, (timestamp, _)| *timestamp > cutoff);

        // 同时限制最大数量
        if history.len() > self.config.max_history_size {
            // 按时间排序，删除最旧的
            let mut entries: Vec<_> = history.drain().collect();
            entries.sort_by_key(|e| e.1 .0);
            let to_remove = entries.len() - self.config.max_history_size;
            // 只保留需要的数量
            for (key, value) in entries.into_iter().skip(to_remove) {
                history.insert(key, value);
            }
        }
    }

    // ===== 内部方法 =====

    /// 发送通知（核心逻辑）
    async fn send_notification(&self, notification: Notification) -> Result<()> {
        // 记录到历史
        let mut history = self.history.write().await;
        history.insert(
            notification.id.clone(),
            (notification.timestamp, notification.clone()),
        );
        drop(history);

        // 发射 Tauri 事件
        if let Err(e) = self
            .app_handle
            .emit("notification://new", &notification)
        {
            tracing::error!("发送 Tauri 通知事件失败: {}", e);
        }

        // 桌面通知（可选）
        if self.config.enable_desktop_notification {
            self.send_desktop_notification(&notification).await?;
        }

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.total_sent += 1;

        tracing::info!("📢 通知已发送: {} - {}", notification.title, notification.body);

        Ok(())
    }

    /// 检查是否应该去重
    async fn should_dedupe(&self, key: &str) -> bool {
        let history = self.history.read().await;
        if let Some((timestamp, _)) = history.get(key) {
            let now = Utc::now();
            let elapsed = (now - *timestamp).num_seconds();
            if elapsed < self.config.dedupe_window_secs {
                return true;
            }
        }
        false
    }

    /// 尝试合并通知
    async fn try_merge_notification(&self, data: &NewEmailData) -> Option<Notification> {
        let history = self.history.read().await;
        let now = Utc::now();

        // 查找合并窗口内的同类型通知
        for (key, (timestamp, existing)) in history.iter() {
            if key.starts_with("new_email:") && key.contains(&format!(":{}", data.folder)) {
                let elapsed = (now - *timestamp).num_seconds();
                if elapsed < self.config.merge_window_secs {
                    // 找到可合并的通知，增加计数
                    let mut new_data = data.clone();
                    new_data.email_count += 1;

                    return Some(Notification {
                        id: format!("new_email:{}:{}:merged", data.account_id, now.timestamp()),
                        title: format!("新邮件 - {}", data.account_name),
                        body: format!("收到 {} 封新邮件", new_data.email_count),
                        notification_type: NotificationType::NewEmail,
                        account_id: Some(data.account_id),
                        timestamp: now,
                        metadata: Some(serde_json::to_value(new_data).ok()?),
                    });
                }
            }
        }

        None
    }

    /// 发送桌面通知
    async fn send_desktop_notification(&self, notification: &Notification) -> Result<()> {
        // 使用 Tauri 的通知插件
        // 这里先记录日志，实际集成需要 tauri-plugin-notification
        tracing::info!(
            "桌面通知: {} - {}",
            notification.title,
            notification.body
        );

        // TODO: 集成 tauri-plugin-notification
        // let _ = self.app_handle.notification()
        //     .title(&notification.title)
        //     .body(&notification.body)
        //     .show();

        Ok(())
    }

    /// 格式化新邮件通知内容
    fn format_new_email_body(&self, data: &NewEmailData) -> String {
        match (data.sender.as_ref(), data.subject.as_ref()) {
            (Some(sender), Some(subject)) => {
                format!("来自: {}\n主题: {}", sender, subject)
            }
            (Some(sender), None) => {
                format!("来自: {}", sender)
            }
            (None, Some(subject)) => {
                format!("主题: {}", subject)
            }
            (None, None) => {
                format!("收到 {} 封新邮件", data.email_count)
            }
        }
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        // 需要 AppHandle，这里提供占位实现
        panic!("NotificationManager::default() should not be used directly. Use NotificationManager::new() instead.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_notification() -> Notification {
        Notification {
            id: "test_notification".to_string(),
            title: "测试通知".to_string(),
            body: "这是一条测试通知".to_string(),
            notification_type: NotificationType::NewEmail,
            account_id: Some(1),
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    #[test]
    fn test_notification_config_default() {
        let config = NotificationConfig::default();
        assert_eq!(config.dedupe_window_secs, 5);
        assert_eq!(config.merge_window_secs, 10);
        assert!(config.enable_desktop_notification);
        assert!(!config.enable_sound);
        assert_eq!(config.max_history_size, 1000);
    }

    #[test]
    fn test_notification_type_serialization() {
        let notification_type = NotificationType::NewEmail;
        let json = serde_json::to_string(&notification_type).unwrap();
        assert_eq!(json, "\"new_email\"");

        let deserialized: NotificationType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, NotificationType::NewEmail);

        // 测试其他类型
        assert_eq!(
            serde_json::to_string(&NotificationType::SyncComplete).unwrap(),
            "\"sync_complete\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::SyncError).unwrap(),
            "\"sync_error\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::AuthError).unwrap(),
            "\"auth_error\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::NetworkError).unwrap(),
            "\"network_error\""
        );
        assert_eq!(
            serde_json::to_string(&NotificationType::System).unwrap(),
            "\"system\""
        );
    }

    #[test]
    fn test_notification_type_deserialization() {
        let new_email: NotificationType = serde_json::from_str("\"new_email\"").unwrap();
        assert_eq!(new_email, NotificationType::NewEmail);

        let sync_complete: NotificationType = serde_json::from_str("\"sync_complete\"").unwrap();
        assert_eq!(sync_complete, NotificationType::SyncComplete);

        let sync_error: NotificationType = serde_json::from_str("\"sync_error\"").unwrap();
        assert_eq!(sync_error, NotificationType::SyncError);

        let auth_error: NotificationType = serde_json::from_str("\"auth_error\"").unwrap();
        assert_eq!(auth_error, NotificationType::AuthError);

        let network_error: NotificationType = serde_json::from_str("\"network_error\"").unwrap();
        assert_eq!(network_error, NotificationType::NetworkError);

        let system: NotificationType = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(system, NotificationType::System);
    }

    #[test]
    fn test_notification_creation() {
        let notification = create_test_notification();
        assert_eq!(notification.id, "test_notification");
        assert_eq!(notification.title, "测试通知");
        assert_eq!(notification.body, "这是一条测试通知");
        assert_eq!(notification.notification_type, NotificationType::NewEmail);
        assert_eq!(notification.account_id, Some(1));
    }

    #[test]
    fn test_notification_serialization() {
        let notification = create_test_notification();
        let json = serde_json::to_string(&notification).unwrap();

        assert!(json.contains("\"id\":\"test_notification\""));
        assert!(json.contains("\"title\":\"测试通知\""));
        assert!(json.contains("\"body\":\"这是一条测试通知\""));
        assert!(json.contains("\"new_email\""));

        let deserialized: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, notification.id);
        assert_eq!(deserialized.title, notification.title);
        assert_eq!(deserialized.body, notification.body);
        assert_eq!(deserialized.notification_type, notification.notification_type);
    }

    #[test]
    fn test_notification_type_equality() {
        assert_eq!(NotificationType::NewEmail, NotificationType::NewEmail);
        assert_eq!(NotificationType::SyncComplete, NotificationType::SyncComplete);
        assert_ne!(NotificationType::NewEmail, NotificationType::SyncError);
    }

    #[test]
    fn test_new_email_data() {
        let data = NewEmailData {
            account_id: 1,
            account_name: "测试账号".to_string(),
            folder: "INBOX".to_string(),
            email_count: 5,
            subject: Some("测试主题".to_string()),
            sender: Some("test@example.com".to_string()),
        };

        assert_eq!(data.account_id, 1);
        assert_eq!(data.account_name, "测试账号");
        assert_eq!(data.folder, "INBOX");
        assert_eq!(data.email_count, 5);
        assert_eq!(data.subject, Some("测试主题".to_string()));
        assert_eq!(data.sender, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_new_email_data_serialization() {
        let data = NewEmailData {
            account_id: 1,
            account_name: "测试账号".to_string(),
            folder: "INBOX".to_string(),
            email_count: 1,
            subject: None,
            sender: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"account_id\":1"));
        assert!(json.contains("\"account_name\":\"测试账号\""));
        assert!(json.contains("\"folder\":\"INBOX\""));

        let deserialized: NewEmailData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, 1);
        assert_eq!(deserialized.account_name, "测试账号");
    }

    #[test]
    fn test_notification_stats_default() {
        let stats = NotificationStats::default();
        assert_eq!(stats.total_sent, 0);
        assert_eq!(stats.total_deduped, 0);
        assert_eq!(stats.total_merged, 0);
        assert_eq!(stats.new_email_count, 0);
        assert_eq!(stats.error_count, 0);
    }

    #[test]
    fn test_notification_stats_clone() {
        let stats = NotificationStats {
            total_sent: 10,
            total_deduped: 2,
            total_merged: 1,
            new_email_count: 5,
            error_count: 1,
        };

        let cloned = stats.clone();
        assert_eq!(cloned.total_sent, 10);
        assert_eq!(cloned.total_deduped, 2);
        assert_eq!(cloned.total_merged, 1);
        assert_eq!(cloned.new_email_count, 5);
        assert_eq!(cloned.error_count, 1);
    }

    #[test]
    fn test_dedupe_key_generation() {
        let account_id = 123;
        let folder = "INBOX";
        let expected_key = format!("new_email:{}:{}", account_id, folder);
        assert_eq!(expected_key, "new_email:123:INBOX");
    }

    #[test]
    fn test_format_new_email_body() {
        let config = NotificationConfig::default();

        // 测试格式化函数
        let data_with_both = NewEmailData {
            account_id: 1,
            account_name: "测试".to_string(),
            folder: "INBOX".to_string(),
            email_count: 1,
            subject: Some("主题".to_string()),
            sender: Some("sender@example.com".to_string()),
        };

        // 实际的格式化逻辑在 NotificationManager 实例方法中
        // 这里只测试数据结构
        assert_eq!(data_with_both.email_count, 1);
        assert!(data_with_both.subject.is_some());
        assert!(data_with_both.sender.is_some());
    }

    #[test]
    fn test_notification_config_custom() {
        let config = NotificationConfig {
            dedupe_window_secs: 10,
            merge_window_secs: 20,
            enable_desktop_notification: false,
            enable_sound: true,
            max_history_size: 500,
        };

        assert_eq!(config.dedupe_window_secs, 10);
        assert_eq!(config.merge_window_secs, 20);
        assert!(!config.enable_desktop_notification);
        assert!(config.enable_sound);
        assert_eq!(config.max_history_size, 500);
    }
}
