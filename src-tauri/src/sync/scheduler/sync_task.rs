//! 同步任务定义
//!
//! 定义同步任务的数据结构和状态

use chrono::{DateTime, Utc};
use rand;

/// 同步任务
#[derive(Debug, Clone)]
pub struct SyncTask {
    /// 任务 ID
    pub id: String,
    /// 账号 ID
    pub account_id: i32,
    /// 文件夹列表（空表示全部文件夹）
    pub folders: Vec<String>,
    /// 任务优先级
    pub priority: SyncTaskPriority,
    /// 任务状态
    pub status: SyncTaskStatus,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 开始时间
    pub started_at: Option<DateTime<Utc>>,
    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

impl SyncTask {
    /// 创建新的同步任务
    pub fn new(account_id: i32, folders: Vec<String>) -> Self {
        // 使用时间戳和随机数生成唯一 ID
        let timestamp = Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let random = rand::random::<u64>();
        let id = format!("{}-{}", timestamp, random);

        Self {
            id,
            account_id,
            folders,
            priority: SyncTaskPriority::Normal,
            status: SyncTaskStatus::Pending,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
        }
    }

    /// 是否已完成
    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            SyncTaskStatus::Completed | SyncTaskStatus::Failed | SyncTaskStatus::Cancelled
        )
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        matches!(self.status, SyncTaskStatus::Running)
    }

    /// 标记开始
    pub fn mark_started(&mut self) {
        self.status = SyncTaskStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// 标记完成
    pub fn mark_completed(&mut self) {
        self.status = SyncTaskStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// 标记失败
    pub fn mark_failed(&mut self, error: String) {
        self.status = SyncTaskStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
    }

    /// 标记取消
    pub fn mark_cancelled(&mut self) {
        self.status = SyncTaskStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }
}

/// 同步任务优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum SyncTaskPriority {
    /// 低优先级
    Low = 0,
    /// 普通优先级
    #[default]
    Normal = 1,
    /// 高优先级
    High = 2,
    /// 紧急优先级
    Urgent = 3,
}

/// 同步任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTaskStatus {
    /// 等待中
    Pending,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_task() {
        let mut task = SyncTask::new(1, vec!["INBOX".to_string()]);
        assert_eq!(task.account_id, 1);
        assert!(!task.is_completed());
        assert!(!task.is_running());

        task.mark_started();
        assert!(task.is_running());

        task.mark_completed();
        assert!(task.is_completed());
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_sync_task_failure() {
        let mut task = SyncTask::new(1, vec![]);
        task.mark_failed("Test error".to_string());
        assert!(task.is_completed());
        assert_eq!(task.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_priority_ordering() {
        assert!(SyncTaskPriority::High > SyncTaskPriority::Normal);
        assert!(SyncTaskPriority::Urgent > SyncTaskPriority::High);
        assert!(SyncTaskPriority::Low < SyncTaskPriority::Normal);
    }
}
