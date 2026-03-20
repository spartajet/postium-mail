//! SMTP 协议实现
//!
//! 提供邮件发送功能，支持 TLS 加密连接、密码和 OAuth2 认证。
//!
//! # 核心功能
//!
//! - **邮件发送**: RFC 5321 SMTP 协议实现
//! - **TLS 支持**: STARTTLS 和隐式 SSL/TLS
//! - **认证方式**: LOGIN、PLAIN 和 OAuth2/XOAUTH2
//! - **附件支持**: MIME 多部分邮件
//! - **错误处理**: 结构化的错误类型和消息
//!
//! # 与 IMAP 模块的关系
//!
//! SMTP 模块用于**发送**邮件，IMAP 模块用于**接收**邮件。
//! 两者共享认证方式（`SmtpAuth` 和 `ImapAuth` 结构类似）。
//!
//! # 使用示例
//!
//! ## 发送纯文本邮件
//!
//! ```rust,no_run
//! use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut client = SmtpClient::new();
//!
//! // 连接并认证
//! let auth = SmtpAuth::Password("app_password".to_string());
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//!
//! // 构建邮件
//! let request = SendEmailRequest {
//!     from: "sender@example.com".to_string(),
//!     to: vec!["recipient@example.com".to_string()],
//!     subject: "测试邮件".to_string(),
//!     body_text: "这是一封测试邮件。".to_string(),
//!     body_html: None,
//!     attachments: vec![],
//! };
//!
//! // 发送
//! let result = client.send_email(&request).await?;
//! println!("邮件已发送: {}", result.message_id);
//! # Ok(())
//! # }
//! ```
//!
//! ## 发送 HTML 邮件
//!
//! ```rust,no_run
//! # use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = SmtpClient::new();
//! # let auth = SmtpAuth::Password("app_password".to_string());
//! # client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//! let request = SendEmailRequest {
//!     from: "sender@example.com".to_string(),
//!     to: vec!["recipient@example.com".to_string()],
//!     subject: "HTML 邮件".to_string(),
//!     body_text: "纯文本版本".to_string(),
//!     body_html: Some("<h1>HTML 版本</h1>".to_string()),
//!     attachments: vec![],
//! };
//! # let result = client.send_email(&request).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## OAuth2 认证
//!
//! ```rust,no_run
//! # use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//! # async fn example() -> anyhow::Result<()> {
//! # let mut client = SmtpClient::new();
//! let auth = SmtpAuth::OAuth2 {
//!     email: "user@gmail.com".to_string(),
//!     access_token: "ya29.a0AfH6...".to_string(),
//! };
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # 常用 SMTP 服务器配置
//!
//! | 服务商 | SMTP 服务器 | 端口 | TLS | STARTTLS |
//! |--------|-----------|------|-----|----------|
//! | Gmail | smtp.gmail.com | 587 | - | ✅ |
//! | Gmail | smtp.gmail.com | 465 | ✅ | - |
//! | Outlook | smtp.office365.com | 587 | - | ✅ |
//! | QQ 邮箱 | smtp.qq.com | 587 | - | ✅ |
//! | 163 邮箱 | smtp.163.com | 465 | ✅ | - |
//!
//! # 模块结构
//!
//! - [`auth`] - SMTP 认证类型定义
//! - [`client`] - SMTP 客户端实现
//! - [`error`] - 错误类型定义
//! - [`types`] - 数据类型定义
//!
//! # 参考资料
//!
//! - [RFC 5321 - Simple Mail Transfer Protocol](https://datatracker.ietf.org/doc/html/rfc5321)
//! - [RFC 6409 - Submission Port (587)](https://datatracker.ietf.org/doc/html/rfc6409)
//! - [RFC 4616 - PLAIN SASL Mechanism](https://datatracker.ietf.org/doc/html/rfc4616)
//! - [RFC 4954 - SMTP AUTH](https://datatracker.ietf.org/doc/html/rfc4954)
//! - [RFC 3207 - STARTTLS](https://datatracker.ietf.org/doc/html/rfc3207)
//!
//! # 注意事项
//!
//! - 端口 587 通常用于 STARTTLS（提交端口）
//! - 端口 465 通常用于隐式 SSL/TLS
//! - 大多数服务商要求使用应用专用密码
//! - OAuth2 令牌需要定期刷新

pub mod auth;
pub mod client;
pub mod error;
pub mod types;

// 导出主要类型
pub use auth::SmtpAuth;
pub use client::SmtpClient;
pub use error::{SmtpError, SmtpResult};
pub use types::{EmailAttachment, EmailAddress, SendEmailRequest, SendEmailResult};
