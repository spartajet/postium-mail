use crate::domain::auth::AuthManager;
use crate::domain::sync::folder_sync_dispatcher::SyncOrchestrator;
use crate::domain::sync::SyncProgressEmitter;
use crate::domain::sync::{FolderStat, SyncProgress, SyncStage};
use crate::error::MailError;
use crate::infrastructure::storage::repository::email_repo;
use crate::infrastructure::storage::DbConn;
use std::sync::Arc;

pub struct SyncService {
    db: DbConn,
    auth: Arc<AuthManager>,
}

impl SyncService {
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self { db, auth }
    }

    /// 同步账号 — 带 Tauri 事件进度通知
    pub async fn sync_account_with_progress(
        &self,
        app_handle: tauri::AppHandle,
        account_id: i32,
    ) -> Result<(), MailError> {
        tracing::info!(account_id, "开始同步（带进度）");

        let emitter = SyncProgressEmitter::new(app_handle);

        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::Connecting,
            folder: None,
            current: 0,
            total: 0,
            message: "正在连接...".into(),
        });

        let orchestrator = SyncOrchestrator::new(self.db.clone(), self.auth.clone());

        emitter.emit(SyncProgress {
            account_id,
            stage: SyncStage::SyncingFolders,
            folder: None,
            current: 0,
            total: 0,
            message: "正在同步文件夹...".into(),
        });

        match orchestrator.sync_account(account_id).await {
            Ok(result) => {
                tracing::info!(
                    account_id,
                    new_emails = result.new_emails,
                    updated_emails = result.updated_emails,
                    duration_ms = result.duration_ms,
                    "同步完成"
                );
                emitter.emit(SyncProgress {
                    account_id,
                    stage: SyncStage::Completed,
                    folder: None,
                    current: result.new_emails,
                    total: result.new_emails + result.updated_emails,
                    message: format!(
                        "同步完成: {} 封新邮件, {} 封更新",
                        result.new_emails, result.updated_emails
                    ),
                });
                Ok(())
            }
            Err(e) => {
                emitter.emit(SyncProgress {
                    account_id,
                    stage: SyncStage::Error,
                    folder: None,
                    current: 0,
                    total: 0,
                    message: format!("同步失败: {e}"),
                });
                Err(e)
            }
        }
    }

    // /// 同步账号 — 静默模式
    // pub async fn sync_account(&self, account_id: i32) -> Result<SyncResult, MailError> {
    //     let orchestrator = SyncOrchestrator::new(self.db.clone(), self.auth.clone());
    //     orchestrator.sync_account(account_id).await
    // }

    /// 获取文件夹统计 — 单条 GROUP BY SQL，替代 N+1 查询
    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>, MailError> {
        email_repo::folder_stats_by_account(&self.db, account_id).await
    }
}
