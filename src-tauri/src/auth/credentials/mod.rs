//! 认证器模块
//!
//! 包含各种认证方式的处理器：
//! - 密码认证

mod password_auth;

pub use password_auth::PasswordAuth;
