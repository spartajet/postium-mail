//! 变化检测模块
//!
//! 包含 IMAP 邮件变化检测的类型定义和核心逻辑

mod detector;
mod types;

pub use detector::ChangeDetector;
pub use types::{
    ChangeDetectionResult, ChangeType, EmailFlags, UidSet,
    imap_flags,
};
