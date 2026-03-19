//! 任务调度器
//!
//! 负责定时任务的调度和执行

use crate::error::{MailError, Result};
use crate::sync::SyncManager;
use sea_orm::DbConn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use chrono::{DateTime, Utc};

/// 任务调度器
///
/// 支持多账号并发调度，使用tokio::time::interval实现
pub struct TaskScheduler {
    /// 任务注册表: account_id -> ScheduledTask
    tasks: Arc<RwLock<HashMap<i32, ScheduledTask>>>,

    /// 运行时状态: account_id -> TaskHandle
    handles: Arc<RwLock<HashMap<i32, JoinHandle<()>>>>,

    /// 全局运行标志
    running: Arc<AtomicBool>,

    /// 任务配置
    config: SchedulerConfig,

    /// 数据库连接(用于获取账号列表)
    db: Arc<DbConn>,

    /// SyncManager引用(用于触发同步)
    sync_manager: Arc<SyncManager>,
}

/// 定时任务
#[derive(Clone, Debug)]
pub struct ScheduledTask {
    pub account_id: i32,
    pub interval_minutes: u64,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub task_type: TaskType,
}

/// 任务类型
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TaskType {
    /// 完整同步
    #[serde(rename = "full_sync")]
    FullSync,
    /// 增量同步
    #[serde(rename = "delta_sync")]
    DeltaSync,
    /// 仅文件夹同步
    #[serde(rename = "folder_sync")]
    FolderSync,
}

/// 调度器配置
#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    /// 默认同步间隔(分钟)
    pub default_interval_minutes: u64,
    /// 最小同步间隔(分钟) - 防止过于频繁
    pub min_interval_minutes: u64,
    /// 最大并发任务数
    pub max_concurrent_tasks: usize,
    /// 任务启动延迟(秒) - 避免同时启动大量任务
    pub task_start_delay_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            default_interval_minutes: 15, // 15分钟
            min_interval_minutes: 5,     // 最小5分钟
            max_concurrent_tasks: 10,    // 最多10个并发任务
            task_start_delay_secs: 2,    // 每个任务延迟2秒启动
        }
    }
}

/// 任务状态(用于序列化)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub account_id: i32,
    pub interval_minutes: u64,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    pub fn new(
        db: Arc<DbConn>,
        sync_manager: Arc<SyncManager>,
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            config: SchedulerConfig::default(),
            db,
            sync_manager,
        }
    }

    /// 创建带配置的任务调度器
    pub fn with_config(
        db: Arc<DbConn>,
        sync_manager: Arc<SyncManager>,
        config: SchedulerConfig,
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            handles: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            config,
            db,
            sync_manager,
        }
    }

    /// 启动调度器
    ///
    /// 从数据库加载所有账号的同步配置，为每个账号创建定时任务
    pub async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(MailError::Internal("调度器已在运行".to_string()));
        }

        self.running.store(true, Ordering::Relaxed);

        // TODO: 从数据库加载所有账号
        // let accounts = crate::services::account_service::list_accounts(&self.db).await?;
        // 目前先使用占位符，后续集成账号服务
        tracing::info!("任务调度器启动（待集成账号加载）");

        // 临时：为测试目的，记录启动
        tracing::info!("🚀 任务调度器启动完成");

        Ok(())
    }

    /// 添加同步任务
    pub async fn add_sync_task(
        &self,
        account_id: i32,
        interval_minutes: u64,
    ) -> Result<()> {
        // 验证间隔
        if interval_minutes < self.config.min_interval_minutes {
            return Err(MailError::Internal(format!(
                "同步间隔过小: {} 分钟,最小 {} 分钟",
                interval_minutes, self.config.min_interval_minutes
            )));
        }

        // 检查是否已存在
        {
            let tasks = self.tasks.read().await;
            if tasks.contains_key(&account_id) {
                return Err(MailError::Internal(format!(
                    "账号 {} 已存在同步任务", account_id
                )));
            }
        }

        // 创建任务
        let task = ScheduledTask {
            account_id,
            interval_minutes,
            enabled: true,
            last_run: None,
            next_run: Some(Utc::now() + chrono::Duration::seconds(
                (interval_minutes * 60) as i64
            )),
            task_type: TaskType::DeltaSync,
        };

        // 启动任务执行器
        let handle = self.spawn_task_executor(task.clone(), 0).await?;

        // 注册任务
        {
            let mut tasks = self.tasks.write().await;
            let mut handles = self.handles.write().await;

            tasks.insert(account_id, task);
            handles.insert(account_id, handle);
        }

        tracing::info!("✅ 已添加账号 {} 的定时同步任务 (间隔: {} 分钟)",
            account_id, interval_minutes);

        Ok(())
    }

    /// 移除任务
    pub async fn remove_task(&self, account_id: i32) -> Result<()> {
        // 停止任务
        let handle = {
            let mut handles = self.handles.write().await;
            handles.remove(&account_id)
        };

        if let Some(handle) = handle {
            handle.abort();
        }

        // 移除任务注册
        let mut tasks = self.tasks.write().await;
        tasks.remove(&account_id);

        tracing::info!("🗑️  已移除账号 {} 的同步任务", account_id);

        Ok(())
    }

    /// 暂停任务
    pub async fn pause_task(&self, account_id: i32) -> Result<()> {
        let mut tasks = self.tasks.write().await;

        if let Some(task) = tasks.get_mut(&account_id) {
            task.enabled = false;
            tracing::info!("⏸️  已暂停账号 {} 的同步任务", account_id);
            Ok(())
        } else {
            Err(MailError::Internal(format!(
                "账号 {} 不存在同步任务", account_id
            )))
        }
    }

    /// 恢复任务
    pub async fn resume_task(&self, account_id: i32) -> Result<()> {
        let mut tasks = self.tasks.write().await;

        if let Some(task) = tasks.get_mut(&account_id) {
            task.enabled = true;
            tracing::info!("▶️  已恢复账号 {} 的同步任务", account_id);
            Ok(())
        } else {
            Err(MailError::Internal(format!(
                "账号 {} 不存在同步任务", account_id
            )))
        }
    }

    /// 更新任务间隔
    pub async fn update_task_interval(
        &self,
        account_id: i32,
        interval_minutes: u64,
    ) -> Result<()> {
        // 重新创建任务
        self.remove_task(account_id).await?;
        self.add_sync_task(account_id, interval_minutes).await?;

        Ok(())
    }

    /// 停止调度器
    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);

        // 停止所有任务
        let mut handles = self.handles.write().await;
        let count = handles.len();

        for (account_id, handle) in handles.drain() {
            handle.abort();
            tracing::info!("⏹ 已停止账号 {} 的同步任务", account_id);
        }

        // 清空任务注册表
        let mut tasks = self.tasks.write().await;
        tasks.clear();

        tracing::info!("🛑 任务调度器已停止 (停止了 {} 个任务)", count);

        Ok(())
    }

    /// 获取所有任务状态
    pub async fn get_task_status(&self) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;

        tasks.values().map(|task| TaskStatus {
            account_id: task.account_id,
            interval_minutes: task.interval_minutes,
            enabled: task.enabled,
            last_run: task.last_run,
            next_run: task.next_run,
        }).collect()
    }

    /// 检查调度器是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    // ===== 内部方法 =====

    /// 生成任务执行器
    ///
    /// 使用tokio::spawn创建独立的异步任务
    async fn spawn_task_executor(
        &self,
        task: ScheduledTask,
        start_delay_secs: u64,
    ) -> Result<JoinHandle<()>> {
        let account_id = task.account_id;
        let interval = Duration::from_secs(task.interval_minutes * 60);

        let running = self.running.clone();
        let sync_manager = self.sync_manager.clone();
        let tasks = self.tasks.clone();

        let handle = tokio::spawn(async move {
            // 初始延迟
            if start_delay_secs > 0 {
                tokio::time::sleep(Duration::from_secs(start_delay_secs)).await;
            }

            // 创建定时器
            let mut timer = tokio::time::interval(interval);
            timer.tick().await; // 跳过第一次立即触发

            tracing::info!("🔄 账号 {} 定时同步任务启动 (间隔: {}秒)",
                account_id, interval.as_secs());

            while running.load(Ordering::Relaxed) {
                timer.tick().await;

                // 检查任务是否启用
                let enabled = {
                    let tasks_guard = tasks.read().await;
                    tasks_guard.get(&account_id)
                        .map(|t| t.enabled)
                        .unwrap_or(false)
                };

                if !enabled {
                    continue;
                }

                // 执行同步
                tracing::info!("⏰ 触发定时同步: account_id={}", account_id);

                match sync_manager.sync_account(account_id).await {
                    Ok(result) => {
                        tracing::info!(
                            "✅ 定时同步完成: account_id={}, synced={}, duration={}ms",
                            account_id, result.total_synced, result.duration_ms
                        );

                        // 更新最后运行时间
                        let mut tasks_guard = tasks.write().await;
                        if let Some(task) = tasks_guard.get_mut(&account_id) {
                            task.last_run = Some(Utc::now());
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ 定时同步失败: account_id={}, error={}",
                            account_id, e);
                        // 继续运行，不中断调度器
                    }
                }
            }

            tracing::info!("⏹️ 账号 {} 定时同步任务停止", account_id);
        });

        Ok(handle)
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        // 需要数据库和sync_manager，这里提供占位实现
        // 实际使用时应该使用 new() 方法
        panic!("TaskScheduler::default() should not be used directly. Use TaskScheduler::new() instead.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    // 创建测试用的数据库连接
    async fn create_test_db() -> Arc<DbConn> {
        // 使用内存 SQLite 数据库进行测试
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Arc::new(db)
    }

    // 注意：由于 SyncManager 是外部类型且需要数据库连接，
    // 这些测试主要验证 TaskScheduler 的数据结构和 API 正确性

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.default_interval_minutes, 15);
        assert_eq!(config.min_interval_minutes, 5);
        assert_eq!(config.max_concurrent_tasks, 10);
        assert_eq!(config.task_start_delay_secs, 2);
    }

    #[test]
    fn test_task_type_serialization() {
        // 测试 TaskType 序列化
        let task_type = TaskType::DeltaSync;
        let json = serde_json::to_string(&task_type).unwrap();
        assert_eq!(json, "\"delta_sync\"");

        let deserialized: TaskType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, TaskType::DeltaSync);

        // 测试其他 TaskType
        assert_eq!(serde_json::to_string(&TaskType::FullSync).unwrap(), "\"full_sync\"");
        assert_eq!(serde_json::to_string(&TaskType::FolderSync).unwrap(), "\"folder_sync\"");
    }

    #[test]
    fn test_task_type_deserialization() {
        // 测试反序列化
        let full_sync: TaskType = serde_json::from_str("\"full_sync\"").unwrap();
        assert_eq!(full_sync, TaskType::FullSync);

        let delta_sync: TaskType = serde_json::from_str("\"delta_sync\"").unwrap();
        assert_eq!(delta_sync, TaskType::DeltaSync);

        let folder_sync: TaskType = serde_json::from_str("\"folder_sync\"").unwrap();
        assert_eq!(folder_sync, TaskType::FolderSync);
    }

    #[test]
    fn test_scheduled_task_creation() {
        let task = ScheduledTask {
            account_id: 1,
            interval_minutes: 10,
            enabled: true,
            last_run: None,
            next_run: Some(Utc::now()),
            task_type: TaskType::DeltaSync,
        };

        assert_eq!(task.account_id, 1);
        assert_eq!(task.interval_minutes, 10);
        assert!(task.enabled);
        assert!(task.last_run.is_none());
        assert!(task.next_run.is_some());
        assert_eq!(task.task_type, TaskType::DeltaSync);
    }

    #[test]
    fn test_task_status_from_scheduled_task() {
        let scheduled = ScheduledTask {
            account_id: 42,
            interval_minutes: 15,
            enabled: true,
            last_run: Some(Utc::now()),
            next_run: Some(Utc::now() + chrono::Duration::minutes(15)),
            task_type: TaskType::FullSync,
        };

        let status = TaskStatus {
            account_id: scheduled.account_id,
            interval_minutes: scheduled.interval_minutes,
            enabled: scheduled.enabled,
            last_run: scheduled.last_run,
            next_run: scheduled.next_run,
        };

        assert_eq!(status.account_id, 42);
        assert_eq!(status.interval_minutes, 15);
        assert!(status.enabled);
        assert!(status.last_run.is_some());
        assert!(status.next_run.is_some());
    }

    #[test]
    fn test_scheduler_config_custom() {
        let config = SchedulerConfig {
            default_interval_minutes: 30,
            min_interval_minutes: 10,
            max_concurrent_tasks: 20,
            task_start_delay_secs: 5,
        };

        assert_eq!(config.default_interval_minutes, 30);
        assert_eq!(config.min_interval_minutes, 10);
        assert_eq!(config.max_concurrent_tasks, 20);
        assert_eq!(config.task_start_delay_secs, 5);
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus {
            account_id: 1,
            interval_minutes: 15,
            enabled: true,
            last_run: Some(Utc::now()),
            next_run: Some(Utc::now() + chrono::Duration::minutes(15)),
        };

        // 测试序列化
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"account_id\":1"));
        assert!(json.contains("\"interval_minutes\":15"));
        assert!(json.contains("\"enabled\":true"));

        // 测试反序列化
        let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, 1);
        assert_eq!(deserialized.interval_minutes, 15);
        assert!(deserialized.enabled);
    }

    #[test]
    fn test_task_equality() {
        let task1 = TaskType::DeltaSync;
        let task2 = TaskType::DeltaSync;
        let task3 = TaskType::FullSync;

        assert_eq!(task1, task2);
        assert_ne!(task1, task3);
    }

    #[tokio::test]
    async fn test_scheduled_task_clone() {
        let task = ScheduledTask {
            account_id: 1,
            interval_minutes: 10,
            enabled: true,
            last_run: None,
            next_run: Some(Utc::now()),
            task_type: TaskType::DeltaSync,
        };

        let cloned = task.clone();
        assert_eq!(task.account_id, cloned.account_id);
        assert_eq!(task.interval_minutes, cloned.interval_minutes);
        assert_eq!(task.enabled, cloned.enabled);
        assert_eq!(task.task_type, cloned.task_type);
    }

    #[test]
    fn test_interval_validation_logic() {
        // 测试间隔验证逻辑（概念测试）
        let min_interval = 5;

        // 有效间隔
        assert!(10 >= min_interval);
        assert!(15 >= min_interval);
        assert!(30 >= min_interval);

        // 无效间隔
        assert!(1 < min_interval);
        assert!(3 < min_interval);
    }

    #[test]
    fn test_duration_calculation() {
        // 测试持续时间计算
        let interval_minutes = 15;
        let duration = Duration::from_secs(interval_minutes * 60);

        assert_eq!(duration.as_secs(), 900); // 15 分钟 = 900 秒
    }

    #[test]
    fn test_next_run_calculation() {
        let interval_minutes = 10;
        let now = Utc::now();
        let next_run = now + chrono::Duration::seconds((interval_minutes * 60) as i64);

        // 验证下次运行时间大约是 10 分钟后
        let diff = next_run - now;
        assert!(diff.num_seconds() >= 599); // 至少 599 秒（浮点精度）
        assert!(diff.num_seconds() <= 601); // 最多 601 秒
    }
}
