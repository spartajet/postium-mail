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
pub use sync_manager::SyncManager;
