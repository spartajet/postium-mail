use crate::domain::sync::SyncEventTx;
use crate::engine::commands::EngineCommand;
use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::accounts;
use crate::infrastructure::storage::repository::account_repo;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 后台定时同步调度器：每周期对所有启用同步的账号向引擎发 `SyncNow`。
///
/// 同步执行由每账号 `AccountEngine` 承担（三档策略），本调度器只负责
/// 定时触发与失败日志。
pub struct SyncScheduler {
    db: DbConn,
    engine: Arc<dyn crate::engine::EngineAccess>,
    event_tx: Option<SyncEventTx>,
    interval_minutes: u64,
    running: Arc<AtomicBool>,
}

impl SyncScheduler {
    pub fn new(
        db: DbConn,
        engine: Arc<dyn crate::engine::EngineAccess>,
        event_tx: Option<SyncEventTx>,
    ) -> Self {
        Self {
            db,
            engine,
            event_tx,
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
        let engine = self.engine.clone();
        let event_tx = self.event_tx.clone();
        let interval = self.interval_minutes;
        let running = self.running.clone();

        crate::rt::runtime().spawn(async move {
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

                for account in &accounts {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    if !account.sync_enabled.unwrap_or(true) {
                        continue;
                    }
                    if let Err(e) =
                        sync_account_via_engine(&engine, account, event_tx.as_ref()).await
                    {
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

/// 向账号引擎发 SyncNow（`event_tx` 为 Some 时本次进度发往该 channel）。
async fn sync_account_via_engine(
    engine: &Arc<dyn crate::engine::EngineAccess>,
    account: &accounts::Model,
    event_tx: Option<&SyncEventTx>,
) -> Result<(), MailError> {
    let handle = engine.imap_engine(account).await?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    handle
        .send(EngineCommand::SyncNow {
            folder: None,
            events: event_tx.cloned(),
            reply: reply_tx,
        })
        .await?;
    reply_rx
        .await
        .map_err(|_| MailError::ImapConnectionFailed("引擎已关闭".into()))?
}

impl Drop for SyncScheduler {
    fn drop(&mut self) {
        self.stop();
    }
}
