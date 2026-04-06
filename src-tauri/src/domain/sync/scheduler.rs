use crate::domain::auth::AuthManager;
use crate::domain::sync::folder_sync_dispatcher::SyncOrchestrator;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::account_repo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 后台定时同步调度器
pub struct SyncScheduler {
    db: DbConn,
    auth: Arc<AuthManager>,
    interval_minutes: u64,
    running: Arc<AtomicBool>,
}

impl SyncScheduler {
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self {
            db,
            auth,
            interval_minutes: 5,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_interval(mut self, minutes: u64) -> Self {
        self.interval_minutes = minutes.max(1);
        self
    }

    /// 启动后台同步循环
    pub fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let db = self.db.clone();
        let auth = self.auth.clone();
        let interval = self.interval_minutes;
        let running = self.running.clone();

        tauri::async_runtime::spawn(async move {
            let sleep_duration = tokio::time::Duration::from_secs(interval * 60);
            loop {
                tokio::time::sleep(sleep_duration).await;

                if !running.load(Ordering::Relaxed) {
                    return;
                }

                let accounts = match account_repo::list(&db).await {
                    Ok(accounts) => {
                        tracing::info!("开始定时同步，共 {} 个账号", accounts.len());
                        accounts
                    }
                    Err(e) => {
                        tracing::error!("SyncScheduler: 获取账号列表失败: {e}");
                        continue;
                    }
                };

                let orchestrator = SyncOrchestrator::new(db.clone(), auth.clone());

                for account in &accounts {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    if !account.sync_enabled.unwrap_or(true) {
                        continue;
                    }
                    if let Err(e) = orchestrator.sync_account(account.id).await {
                        tracing::warn!(
                            "SyncScheduler: 同步账号 {} ({}) 失败: {e}",
                            account.id,
                            account.email
                        );
                    }
                }
            }
        });

        tracing::info!("SyncScheduler 已启动，间隔 {} 分钟", interval);
    }

    /// 停止后台同步
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for SyncScheduler {
    fn drop(&mut self) {
        self.stop();
    }
}
