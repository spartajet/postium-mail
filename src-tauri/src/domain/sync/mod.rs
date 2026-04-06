pub mod folder_sync_dispatcher;
pub mod folder_sync_full;
pub mod folder_sync_increment;
pub mod scheduler;

pub use folder_sync_dispatcher::SyncOrchestrator;
pub use scheduler::SyncScheduler;
use serde::{Deserialize, Serialize};
use specta::Type;

use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
pub struct SyncProgressEvent {
    pub progress: SyncProgress,
}

/// 同步进度发射器 — 独立于 SyncOrchestrator
#[derive(Clone)]
pub struct SyncProgressEmitter {
    app_handle: AppHandle,
}

impl SyncProgressEmitter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    pub fn emit(&self, progress: SyncProgress) {
        let event = SyncProgressEvent { progress };
        let _ = self.app_handle.emit("sync-progress-event", &event);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncProgress {
    pub account_id: i32,
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SyncResult {
    pub new_emails: usize,
    pub updated_emails: usize,
    pub deleted_emails: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FolderStat {
    pub folder: String,
    pub total: usize,
    pub unread: usize,
}

/// 同步元数据
///
/// 根据同步策略包含不同的元数据
#[derive(Debug, Clone)]
pub enum SyncMode {
    /// 全量同步元数据
    Full { uidvalidity: u64 },
    /// 增量同步元数据
    Incremental {
        /// 上次同步的最高 UID
        last_sync_uid: u32,
    },
}
