//! SMTP 协议实现
//!
//! 提供邮件发送功能，支持密码和 OAuth2 认证

pub mod auth;
pub mod client;
pub mod error;
pub mod types;

// 导出主要类型
pub use auth::SmtpAuth;
pub use client::SmtpClient;
pub use error::{SmtpError, SmtpResult};
pub use types::{EmailAttachment, EmailAddress, SendEmailRequest, SendEmailResult};
