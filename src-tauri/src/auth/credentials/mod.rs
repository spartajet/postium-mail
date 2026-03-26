//! 认证器模块
//!
//! 包含各种认证方式的处理器：
//! - 密码认证
//! - OAuth2 认证

mod password_auth;
mod oauth_auth;

pub use password_auth::PasswordAuth;
pub use oauth_auth::OAuthAuth;
