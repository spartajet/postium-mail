//! 流程引擎
//!
//! 协调整个邮件客户端的工作流程，整合定时任务、通知管理和实时监听

use crate::error::{MailError, Result};
use crate::engine::notification_manager::NotificationManager;
use crate::engine::task_scheduler::TaskScheduler;
use crate::protocols::imap::idle_manager::ImapIdleManager;
use crate::protocols::imap::AsyncImapClient;
use crate::sync::SyncManager;
use sea_orm::DbConn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tauri::AppHandle;
use tracing::instrument;

/// 流程引擎
///
/// 整合 TaskScheduler、NotificationManager、ImapIdleManager 和 SyncManager
pub struct FlowEngine {
    /// 任务调度器
    task_scheduler: Arc<TaskScheduler>,

    /// 通知管理器
    notification_manager: Arc<NotificationManager>,

    /// 同步管理器
    sync_manager: Arc<SyncManager>,

    /// IDLE 管理器集合: account_id -> ImapIdleManager
    idle_managers: Arc<RwLock<HashMap<i32, Arc<ImapIdleManager>>>>,

    /// 全局运行标志
    running: Arc<AtomicBool>,

    /// 数据库连接
    db: Arc<DbConn>,

    /// Tauri AppHandle
    app_handle: AppHandle,

    /// 引擎配置
    config: FlowEngineConfig,
}

/// 流程引擎配置
#[derive(Clone, Debug)]
pub struct FlowEngineConfig {
    /// 是否启用任务调度器
    pub enable_task_scheduler: bool,
    /// 是否启用通知管理器
    pub enable_notification_manager: bool,
    /// 是否启用 IDLE 监听
    pub enable_idle_monitoring: bool,
    /// 默认同步间隔（分钟）
    pub default_sync_interval_minutes: u64,
    /// IDLE 轮询间隔（秒）
    pub idle_polling_interval_secs: u64,
}

impl Default for FlowEngineConfig {
    fn default() -> Self {
        Self {
            enable_task_scheduler: true,
            enable_notification_manager: true,
            enable_idle_monitoring: true,
            default_sync_interval_minutes: 15,
            idle_polling_interval_secs: 300, // 5分钟
        }
    }
}

/// 引擎状态
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    /// 已停止
    Stopped,
    /// 启动中
    Starting,
    /// 运行中
    Running,
    /// 停止中
    Stopping,
    /// 错误状态
    Error(String),
}

/// 引擎状态报告
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineStatusReport {
    pub state: EngineState,
    pub running_tasks: usize,
    pub active_idle_monitors: usize,
    pub total_notifications_sent: usize,
    pub uptime_seconds: u64,
}

impl FlowEngine {
    /// 创建新的流程引擎
    pub fn new(
        db: Arc<DbConn>,
        sync_manager: Arc<SyncManager>,
        app_handle: AppHandle,
    ) -> Self {
        let config = FlowEngineConfig::default();

        // 创建任务调度器
        let task_scheduler = Arc::new(TaskScheduler::with_config(
            db.clone(),
            sync_manager.clone(),
            crate::engine::task_scheduler::SchedulerConfig::default(),
        ));

        // 创建通知管理器
        let notification_manager = Arc::new(NotificationManager::new(app_handle.clone()));

        Self {
            task_scheduler,
            notification_manager,
            sync_manager,
            idle_managers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            db,
            app_handle,
            config,
        }
    }

    /// 创建带配置的流程引擎
    pub fn with_config(
        db: Arc<DbConn>,
        sync_manager: Arc<SyncManager>,
        app_handle: AppHandle,
        config: FlowEngineConfig,
    ) -> Self {
        // 创建任务调度器配置
        let scheduler_config = crate::engine::task_scheduler::SchedulerConfig {
            default_interval_minutes: config.default_sync_interval_minutes,
            ..Default::default()
        };

        let task_scheduler = Arc::new(TaskScheduler::with_config(
            db.clone(),
            sync_manager.clone(),
            scheduler_config,
        ));

        let notification_manager = Arc::new(NotificationManager::new(app_handle.clone()));

        Self {
            task_scheduler,
            notification_manager,
            sync_manager,
            idle_managers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
            db,
            app_handle,
            config,
        }
    }

    /// 启动引擎
    ///
    /// 启动所有子组件：任务调度器、通知管理器、IDLE 监听
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::Relaxed) {
            return Err(MailError::Internal("引擎已在运行".to_string()));
        }

        self.running.store(true, Ordering::Relaxed);

        tracing::info!("🚀 启动流程引擎...");

        // 1. 启动任务调度器
        if self.config.enable_task_scheduler {
            self.task_scheduler.start().await?;
            tracing::info!("✅ 任务调度器已启动");
        }

        // 2. 从数据库加载账号并启动 IDLE 监听
        if self.config.enable_idle_monitoring {
            self.start_idle_monitors().await?;
            tracing::info!("✅ IDLE 监听已启动");
        }

        tracing::info!("🎉 流程引擎启动完成");

        Ok(())
    }

    /// 停止引擎
    ///
    /// 优雅关闭所有子组件
    #[instrument(skip(self))]
    pub async fn stop(&self) -> Result<()> {
        if !self.running.load(Ordering::Relaxed) {
            return Ok(());
        }

        tracing::info!("🛑 停止流程引擎...");

        self.running.store(false, Ordering::Relaxed);

        // 1. 停止任务调度器
        if self.config.enable_task_scheduler {
            self.task_scheduler.stop().await?;
            tracing::info!("✅ 任务调度器已停止");
        }

        // 2. 停止所有 IDLE 监听
        if self.config.enable_idle_monitoring {
            let idle_managers = self.idle_managers.read().await;
            for (account_id, manager) in idle_managers.iter() {
                manager.stop().await;
                tracing::info!("✅ 已停止账号 {} 的 IDLE 监听", account_id);
            }
            self.idle_managers.write().await.clear();
        }

        tracing::info!("🏁 流程引擎已停止");

        Ok(())
    }

    /// 获取引擎状态报告
    pub async fn get_status(&self) -> EngineStatusReport {
        let running = self.running.load(Ordering::Relaxed);
        let state = if running {
            EngineState::Running
        } else {
            EngineState::Stopped
        };

        // 获取任务调度器状态
        let task_status = self.task_scheduler.get_task_status().await;
        let running_tasks = task_status.len();

        // 获取 IDLE 管理器数量
        let idle_managers = self.idle_managers.read().await;
        let active_idle_monitors = idle_managers.len();

        // 获取通知统计
        let notification_stats = self.notification_manager.get_stats().await;
        let total_notifications_sent = notification_stats.total_sent;

        EngineStatusReport {
            state,
            running_tasks,
            active_idle_monitors,
            total_notifications_sent,
            uptime_seconds: 0, // TODO: 实现运行时间追踪
        }
    }

    /// 检查引擎是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 为账号添加定时同步任务
    pub async fn add_sync_task(&self, account_id: i32, interval_minutes: u64) -> Result<()> {
        self.task_scheduler.add_sync_task(account_id, interval_minutes).await
    }

    /// 移除账号的同步任务
    pub async fn remove_sync_task(&self, account_id: i32) -> Result<()> {
        self.task_scheduler.remove_task(account_id).await
    }

    /// 暂停账号的同步任务
    pub async fn pause_sync_task(&self, account_id: i32) -> Result<()> {
        self.task_scheduler.pause_task(account_id).await
    }

    /// 恢复账号的同步任务
    pub async fn resume_sync_task(&self, account_id: i32) -> Result<()> {
        self.task_scheduler.resume_task(account_id).await
    }

    /// 手动触发账号同步
    pub async fn trigger_sync(&self, account_id: i32) -> Result<crate::sync::SyncResult> {
        self.sync_manager.sync_account(account_id).await
    }

    // ===== 内部方法 =====

    /// 启动 IDLE 监听（为所有账号）
    async fn start_idle_monitors(&self) -> Result<()> {
        // TODO: 从数据库加载账号列表
        // 目前暂时只记录日志
        tracing::info!("📡 IDLE 监听启动（待集成账号加载）");
        Ok(())
    }

    /// 为单个账号启动 IDLE 监听
    async fn start_idle_monitor_for_account(
        &self,
        account_id: i32,
        imap_client: Arc<tokio::sync::Mutex<AsyncImapClient>>,
        folder: String,
    ) -> Result<()> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let idle_manager = Arc::new(ImapIdleManager::new(
            imap_client,
            account_id,
            folder,
            event_tx,
            self.config.idle_polling_interval_secs,
        ));

        // 启动 IDLE 监听
        idle_manager
            .start()
            .await
            .map_err(|e| MailError::Internal(format!("IDLE 启动失败: {}", e)))?;

        // 注册到管理器集合
        let mut managers = self.idle_managers.write().await;
        managers.insert(account_id, idle_manager);

        tracing::info!("📡 已为账号 {} 启动 IDLE 监听", account_id);

        // 启动事件处理任务
        let notification_manager = self.notification_manager.clone();
        let sync_manager = self.sync_manager.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            while running.load(Ordering::Relaxed) {
                match event_rx.recv().await {
                    Some(event) => {
                        // 处理 IDLE 事件
                        match event {
                            crate::protocols::imap::IdleEvent::NewEmail { folder, uid } => {
                                tracing::info!(
                                    "📬 IDLE 事件: 新邮件 account_id={}, folder={}, uid={}",
                                    account_id,
                                    folder,
                                    uid
                                );

                                // 发送通知
                                let _ = notification_manager.notify_new_email(
                                    crate::engine::notification_manager::NewEmailData {
                                        account_id,
                                        account_name: format!("账号 {}", account_id),
                                        folder: folder.clone(),
                                        email_count: 1,
                                        subject: None,
                                        sender: None,
                                    },
                                ).await;

                                // 触发同步
                                let _ = sync_manager.sync_account(account_id).await;
                            }
                            crate::protocols::imap::IdleEvent::Disconnected => {
                                tracing::warn!("⚠️  IDLE 连接断开: account_id={}", account_id);
                            }
                            crate::protocols::imap::IdleEvent::Error(err) => {
                                tracing::error!("❌ IDLE 错误: account_id={}, error={}", account_id, err);
                            }
                            _ => {}
                        }
                    }
                    None => {
                        // 通道关闭，退出循环
                        break;
                    }
                }
            }

            tracing::info!("⏹ IDLE 事件处理已停止: account_id={}", account_id);
        });

        Ok(())
    }
}

impl Default for FlowEngine {
    fn default() -> Self {
        panic!("FlowEngine::default() should not be used directly. Use FlowEngine::new() instead.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_state_serialization() {
        let state = EngineState::Running;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"running\"");

        let deserialized: EngineState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, EngineState::Running);
    }

    #[test]
    fn test_engine_state_error() {
        let error = EngineState::Error("测试错误".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, "{\"error\":\"测试错误\"}");

        let deserialized: EngineState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, EngineState::Error("测试错误".to_string()));
    }

    #[test]
    fn test_engine_state_equality() {
        assert_eq!(EngineState::Stopped, EngineState::Stopped);
        assert_eq!(EngineState::Running, EngineState::Running);
        assert_ne!(EngineState::Stopped, EngineState::Running);
    }

    #[test]
    fn test_flow_engine_config_default() {
        let config = FlowEngineConfig::default();
        assert!(config.enable_task_scheduler);
        assert!(config.enable_notification_manager);
        assert!(config.enable_idle_monitoring);
        assert_eq!(config.default_sync_interval_minutes, 15);
        assert_eq!(config.idle_polling_interval_secs, 300);
    }

    #[test]
    fn test_flow_engine_config_custom() {
        let config = FlowEngineConfig {
            enable_task_scheduler: false,
            enable_notification_manager: true,
            enable_idle_monitoring: false,
            default_sync_interval_minutes: 30,
            idle_polling_interval_secs: 600,
        };

        assert!(!config.enable_task_scheduler);
        assert!(config.enable_notification_manager);
        assert!(!config.enable_idle_monitoring);
        assert_eq!(config.default_sync_interval_minutes, 30);
        assert_eq!(config.idle_polling_interval_secs, 600);
    }

    #[test]
    fn test_engine_status_report() {
        let report = EngineStatusReport {
            state: EngineState::Running,
            running_tasks: 3,
            active_idle_monitors: 2,
            total_notifications_sent: 10,
            uptime_seconds: 3600,
        };

        assert_eq!(report.state, EngineState::Running);
        assert_eq!(report.running_tasks, 3);
        assert_eq!(report.active_idle_monitors, 2);
        assert_eq!(report.total_notifications_sent, 10);
        assert_eq!(report.uptime_seconds, 3600);
    }

    #[test]
    fn test_engine_status_report_serialization() {
        let report = EngineStatusReport {
            state: EngineState::Running,
            running_tasks: 1,
            active_idle_monitors: 1,
            total_notifications_sent: 5,
            uptime_seconds: 300,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"running_tasks\":1"));
        assert!(json.contains("\"active_idle_monitors\":1"));

        let deserialized: EngineStatusReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.state, EngineState::Running);
        assert_eq!(deserialized.running_tasks, 1);
    }
}
