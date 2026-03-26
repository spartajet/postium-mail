//! 同步进度追踪器
//!
//! 跟踪同步操作的进度和状态

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 同步进度
#[derive(Debug, Clone)]
pub struct SyncProgress {
    /// 账号 ID
    pub account_id: i32,
    /// 文件夹名称
    pub folder: String,
    /// 总邮件数
    pub total_emails: usize,
    /// 已处理邮件数
    pub processed_emails: usize,
    /// 新邮件数
    pub new_emails: usize,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 当前状态
    pub status: SyncStatus,
}

impl SyncProgress {
    /// 创建新的进度记录
    pub fn new(account_id: i32, folder: String) -> Self {
        Self {
            account_id,
            folder,
            total_emails: 0,
            processed_emails: 0,
            new_emails: 0,
            started_at: Utc::now(),
            status: SyncStatus::InProgress,
        }
    }

    /// 计算完成百分比
    pub fn progress_percent(&self) -> f64 {
        if self.total_emails == 0 {
            return 0.0;
        }
        (self.processed_emails as f64 / self.total_emails as f64) * 100.0
    }

    /// 是否完成
    pub fn is_completed(&self) -> bool {
        matches!(self.status, SyncStatus::Completed)
    }

    /// 是否失败
    pub fn is_failed(&self) -> bool {
        matches!(self.status, SyncStatus::Failed)
    }
}

/// 同步状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 同步进度追踪器
///
/// 负责跟踪同步操作的进度
pub struct ProgressTracker {
    /// 进度记录 (account_id -> folder -> progress)
    progress: Arc<RwLock<HashMap<i32, HashMap<String, SyncProgress>>>>,
}

impl ProgressTracker {
    /// 创建新的进度追踪器
    pub fn new() -> Self {
        Self {
            progress: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 开始跟踪同步
    pub async fn start_tracking(&self, account_id: i32, folder: String) {
        let mut progress = self.progress.write().await;
        let account_progress = progress.entry(account_id).or_default();
        account_progress.insert(folder.clone(), SyncProgress::new(account_id, folder));
    }

    /// 更新进度
    pub async fn update_progress(
        &self,
        account_id: i32,
        folder: &str,
        processed_emails: usize,
        total_emails: usize,
        new_emails: usize,
    ) {
        let mut progress = self.progress.write().await;
        if let Some(account_progress) = progress.get_mut(&account_id)
            && let Some(folder_progress) = account_progress.get_mut(folder)
        {
            folder_progress.processed_emails = processed_emails;
            folder_progress.total_emails = total_emails;
            folder_progress.new_emails = new_emails;
        }
    }

    /// 标记完成
    pub async fn mark_completed(&self, account_id: i32, folder: &str) {
        let mut progress = self.progress.write().await;
        if let Some(account_progress) = progress.get_mut(&account_id)
            && let Some(folder_progress) = account_progress.get_mut(folder)
        {
            folder_progress.status = SyncStatus::Completed;
        }
    }

    /// 标记失败
    pub async fn mark_failed(&self, account_id: i32, folder: &str) {
        let mut progress = self.progress.write().await;
        if let Some(account_progress) = progress.get_mut(&account_id)
            && let Some(folder_progress) = account_progress.get_mut(folder)
        {
            folder_progress.status = SyncStatus::Failed;
        }
    }

    /// 获取进度
    pub async fn get_progress(&self, account_id: i32, folder: &str) -> Option<SyncProgress> {
        let progress = self.progress.read().await;
        progress
            .get(&account_id)
            .and_then(|account_progress| account_progress.get(folder).cloned())
    }

    /// 获取账号的所有进度
    pub async fn get_account_progress(&self, account_id: i32) -> Vec<SyncProgress> {
        let progress = self.progress.read().await;
        progress
            .get(&account_id)
            .map(|account_progress| account_progress.values().cloned().collect())
            .unwrap_or_default()
    }

    /// 清除进度
    pub async fn clear_progress(&self, account_id: i32, folder: &str) {
        let mut progress = self.progress.write().await;
        if let Some(account_progress) = progress.get_mut(&account_id) {
            account_progress.remove(folder);
        }
    }

    /// 清除账号的所有进度
    pub async fn clear_account_progress(&self, account_id: i32) {
        let mut progress = self.progress.write().await;
        progress.remove(&account_id);
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker() {
        let tracker = ProgressTracker::new();
        let account_id = 1;
        let folder = "INBOX";

        // 开始跟踪
        tracker.start_tracking(account_id, folder.to_string()).await;

        // 更新进度
        tracker
            .update_progress(account_id, folder, 50, 100, 10)
            .await;

        // 获取进度
        let progress = tracker.get_progress(account_id, folder).await;
        assert!(progress.is_some());
        let progress = progress.unwrap();
        assert_eq!(progress.processed_emails, 50);
        assert_eq!(progress.total_emails, 100);
        assert_eq!(progress.progress_percent(), 50.0);

        // 标记完成
        tracker.mark_completed(account_id, folder).await;
        let progress = tracker.get_progress(account_id, folder).await;
        assert!(progress.unwrap().is_completed());
    }

    #[tokio::test]
    async fn test_clear_progress() {
        let tracker = ProgressTracker::new();
        let account_id = 1;
        let folder = "INBOX";

        tracker
            .start_tracking(account_id, folder.to_string())
            .await;

        // 清除进度
        tracker.clear_progress(account_id, folder).await;

        let progress = tracker.get_progress(account_id, folder).await;
        assert!(progress.is_none());
    }
}
