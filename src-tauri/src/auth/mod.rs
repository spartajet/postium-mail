//! 认证模块
//!
//! 提供统一的认证管理功能

mod auth_manager;
mod enterprise_auth;
mod oauth_handler;
mod password_auth;
mod token_manager;

// 重新导出主要类型
pub use auth_manager::{AuthManager, ImapAuthInfo};
