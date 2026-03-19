//! 协议层模块
//!
//! 提供各种邮件协议的实现：
//! - IMAP - 邮件接收协议
//! - SMTP - 邮件发送协议
//!
//! # 模块结构
//!
//! - `imap` - IMAP 协议实现
//! - `smtp` - SMTP 协议实现

pub mod imap;
pub mod smtp;

// 重新导出常用类型
pub use imap::{AsyncImapClient, ImapAuth, ImapClient};
pub use smtp::{EmailAddress, EmailAttachment, SendEmailRequest, SendEmailResult, SmtpAuth, SmtpClient, SmtpError, SmtpResult};
