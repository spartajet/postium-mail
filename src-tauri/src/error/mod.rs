//! 错误处理模块
//!
//! 提供统一的错误类型定义和处理机制

mod retry;
mod types;

// 重新导出主要类型
pub use types::MailError;

/// 结果类型别名
pub type Result<T> = std::result::Result<T, MailError>;
