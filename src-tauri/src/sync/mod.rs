//! 同步模块
//!
//! 提供邮件同步功能

mod change_detector;
mod delta_sync;
mod folder_manager;
mod mail_processor;
mod sync_manager;
mod sync_state;

// 重新导出主要类型
pub use sync_manager::{SyncManager, SyncProgress, SyncStage, SyncResult};
pub use delta_sync::{DeltaSync, SyncStrategy, DeltaSyncResult};
pub use change_detector::{ChangeDetector, ChangeType, ChangeDetectionResult, EmailFlags, UidSet};
pub use folder_manager::{FolderManager, SpecialUse, ImapFolder, FolderSyncResult};
pub use mail_processor::{MailProcessor, MailData, MailProcessResult};
