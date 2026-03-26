//! 同步调度器
//!
//! 负责调度和管理同步任务

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::sync_task::SyncTask;

/// 同步调度器
///
/// 负责调度和管理同步任务
pub struct SyncScheduler {
    /// 任务队列
    task_queue: Arc<RwLock<VecDeque<SyncTask>>>,
    /// 运行中的任务 (task_id -> task)
    running_tasks: Arc<RwLock<HashMap<String, SyncTask>>>,
    /// 已完成的任务 (account_id -> tasks)
    completed_tasks: Arc<RwLock<HashMap<i32, Vec<SyncTask>>>>,
}

impl SyncScheduler {
    /// 创建新的同步调度器
    pub fn new() -> Self {
        Self {
            task_queue: Arc::new(RwLock::new(VecDeque::new())),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            completed_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加任务
    pub async fn schedule_task(&self, task: SyncTask) {
        let mut queue = self.task_queue.write().await;
        queue.push_back(task);
    }

    /// 获取下一个待执行任务
    pub async fn next_task(&self) -> Option<SyncTask> {
        let mut queue = self.task_queue.write().await;
        queue.pop_front()
    }

    /// 标记任务开始
    pub async fn mark_task_started(&self, task_id: &str, mut task: SyncTask) {
        task.mark_started();
        let mut running = self.running_tasks.write().await;
        running.insert(task_id.to_string(), task);
    }

    /// 标记任务完成
    pub async fn mark_task_completed(&self, task_id: &str, mut task: SyncTask) {
        task.mark_completed();
        let mut running = self.running_tasks.write().await;
        running.remove(task_id);

        let mut completed = self.completed_tasks.write().await;
        let account_tasks = completed.entry(task.account_id).or_default();
        account_tasks.push(task);
    }

    /// 标记任务失败
    pub async fn mark_task_failed(&self, task_id: &str, mut task: SyncTask, error: String) {
        task.mark_failed(error);
        let mut running = self.running_tasks.write().await;
        running.remove(task_id);

        let mut completed = self.completed_tasks.write().await;
        let account_tasks = completed.entry(task.account_id).or_default();
        account_tasks.push(task);
    }

    /// 获取运行中的任务
    pub async fn get_running_tasks(&self) -> Vec<SyncTask> {
        let running = self.running_tasks.read().await;
        running.values().cloned().collect()
    }

    /// 获取账号的已完成任务
    pub async fn get_completed_tasks(&self, account_id: i32) -> Vec<SyncTask> {
        let completed = self.completed_tasks.read().await;
        completed.get(&account_id).cloned().unwrap_or_default()
    }

    /// 清除已完成的任务
    pub async fn clear_completed_tasks(&self, account_id: i32) {
        let mut completed = self.completed_tasks.write().await;
        completed.remove(&account_id);
    }

    /// 获取队列大小
    pub async fn queue_size(&self) -> usize {
        let queue = self.task_queue.read().await;
        queue.len()
    }

    /// 获取运行中的任务数
    pub async fn running_count(&self) -> usize {
        let running = self.running_tasks.read().await;
        running.len()
    }
}

impl Default for SyncScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sync_scheduler() {
        let scheduler = SyncScheduler::new();

        // 添加任务
        let task1 = SyncTask::new(1, vec!["INBOX".to_string()]);
        let task2 = SyncTask::new(2, vec!["SENT".to_string()]);

        scheduler.schedule_task(task1.clone()).await;
        scheduler.schedule_task(task2.clone()).await;

        assert_eq!(scheduler.queue_size().await, 2);

        // 获取下一个任务
        let next = scheduler.next_task().await;
        assert!(next.is_some());
        assert_eq!(next.unwrap().account_id, 1);
        assert_eq!(scheduler.queue_size().await, 1);
    }

    #[tokio::test]
    async fn test_task_lifecycle() {
        let scheduler = SyncScheduler::new();

        let task = SyncTask::new(1, vec![]);
        let task_id = task.id.clone();

        // 标记开始
        scheduler.mark_task_started(&task_id, task.clone()).await;
        assert_eq!(scheduler.running_count().await, 1);

        // 标记完成
        scheduler.mark_task_completed(&task_id, task.clone()).await;
        assert_eq!(scheduler.running_count().await, 0);

        let completed = scheduler.get_completed_tasks(1).await;
        assert_eq!(completed.len(), 1);
        assert!(completed[0].is_completed());
    }
}
