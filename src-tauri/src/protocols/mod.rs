//! 协议层模块
//!
//! 提供各种邮件协议的客户端实现。
//!
//! # 核心协议
//!
//! - **IMAP**: 邮件接收协议（RFC 3501）
//! - **SMTP**: 邮件发送协议（RFC 5321）
//!
//! # 架构设计
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │              应用层                      │
//! │  (同步、发送、邮件操作)                  │
//! └────────────────┬────────────────────────┘
//!                  │
//! ┌────────────────▼────────────────────────┐
//! │            协议层                         │
//! │  ┌───────────────┐  ┌──────────────┐  │
//! │  │  IMAP Client  │  │  SMTP Client │  │
//! │  │  async-imap   │  │   lettre     │  │
//! │  └───────────────┘  └──────────────┘  │
//! └────────────────┬───────────────────────┘
//!                  │
//! ┌────────────────▼────────────────────────┐
//! │         邮件服务器                        │
//! │  (Gmail, Outlook, QQ, 163, ...)         │
//! └────────────────────────────────────────┘
//! ```
//!
//! # IMAP 模块
//!
//! ## 核心功能
//!
//! - **连接管理**: TLS 加密连接、认证
//! - **文件夹操作**: 列出、创建、删除、重命名文件夹
//! - **邮件操作**: 获取、搜索、标志管理、删除
//! - **增量同步**: CONDSTORE 支持（RFC 4551）
//! - **实时推送**: IDLE 支持（RFC 2177）
//!
//! ## 主要类型
//!
//! - [`AsyncImapClient`][]: 异步 IMAP 客户端
//! - [`ImapAuth`][]: 认证方式（密码/OAuth2）
//! - [`ImapFolder`][]: 文件夹元数据
//!
//! # SMTP 模块
//!
//! ## 核心功能
//!
//! - **连接类型**: SSL/TLS (465)、STARTTLS (587)
//! - **认证方式**: 密码、OAuth2/XOAUTH2
//! - **邮件构建**: 纯文本、HTML、多部分邮件
//! - **附件支持**: MIME 多部分附件
//!
//! ## 主要类型
//!
//! - [`SmtpClient`][]: SMTP 客户端
//! - [`SmtpAuth`][]: 认证方式
//! - [`SendEmailRequest`][]: 发送邮件请求
//! - [`SendEmailResult`][]: 发送结果
//!
//! # 连接配置
//!
//! ## 常用服务器配置
//!
//! | 服务商 | IMAP 服务器 | SMTP 服务器 | 端口 |
//! |--------|-------------|-------------|------|
//! | Gmail | imap.gmail.com | smtp.gmail.com | 993/587 |
//! | Outlook | outlook.office365.com | smtp-mail.outlook.com | 993/587 |
//! | iCloud | imap.mail.me.com | smtp.mail.me.com | 993/587 |
//! | QQ | imap.qq.com | smtp.qq.com | 993/587 |
//! | 163 | imap.163.com | smtp.163.com | 993/465 |
//!
//! # 使用示例
//!
//! ## IMAP 连接
//!
//! ```rust,no_run
//! use crate::protocols::imap::{AsyncImapClient, ImapAuth};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut client = AsyncImapClient::new();
//!
//! let auth = ImapAuth::Password("password".to_string());
//! client.connect("imap.gmail.com", 993, "user@gmail.com", auth).await?;
//!
//! // 获取文件夹列表
//! let folders = client.list_folders().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## SMTP 发送
//!
//! ```rust,no_run
//! use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let mut client = SmtpClient::new();
//!
//! let auth = SmtpAuth::Password("app_password".to_string());
//! client.connect("smtp.gmail.com", 587, "user@gmail.com", auth).await?;
//!
//! let request = SendEmailRequest {
//!     from: "sender@example.com".to_string(),
//!     to: vec!["recipient@example.com".to_string()],
//!     subject: "测试".to_string(),
//!     html_body: "<h1>测试</h1>".to_string(),
//!     ..Default::default()
//! };
//!
//! let result = client.send_email(request).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # 协议扩展
//!
//! ## CONDSTORE (RFC 4551)
//!
//! 用于高效的增量同步：
//!
//! ```text
//! - 使用 MODSEQ 跟踪邮件变更
//! - 仅获取变更的邮件
//! - 减少网络流量和同步时间
//! ```
//!
//! ## IDLE (RFC 2177)
//!
//! 实时新邮件推送：
//!
//! ```text
//! 1. 发送 IDLE 命令
//! 2. 等待服务器推送
//! 3. 收到新邮件通知
//! 4. 完成 IDLE
//! ```
//!
//! # 错误处理
//!
//! 协议层错误分类：
//!
//! - **连接错误**: 网络连接失败
//! - **认证错误**: 用户名/密码错误
//! - **协议错误**: IMAP/SMTP 协议错误
//! - **解析错误**: 邮件解析失败

pub mod imap;
pub mod smtp;

// 重新导出常用类型
pub use imap::{AsyncImapClient, ImapAuth, ImapClient};
pub use smtp::{EmailAddress, EmailAttachment, SendEmailRequest, SendEmailResult, SmtpAuth, SmtpClient, SmtpError, SmtpResult};
