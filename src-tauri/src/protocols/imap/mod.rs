//! IMAP 协议实现
//!
//! 提供完整的 IMAP 客户端功能，支持：
//! - 传统密码认证
//! - OAuth2/XOAUTH2 认证
//! - IDLE 实时通知
//!
//! # 模块结构
//!
//! - `client` - IMAP 客户端核心实现
//! - `types` - 数据类型定义
//! - `error` - 错误类型定义
//! - `auth` - 认证类型定义
//! - `idle` - IDLE 实时通知支持
//! - `parser` - 邮件解析工具
//! - `service` - IMAP 服务包装
//! - `tests` - 测试工具

mod auth;
mod client;
mod error;
mod parser;
mod service;
mod tests;
mod types;

// ========== 重新导出所有公共接口 ==========

// 认证相关
pub use auth::ImapAuth;

// 客户端
pub use client::{AsyncImapClient, three_months_ago_imap_format};

// 类型定义
pub use types::*;

// 错误类型
pub use error::{ImapError, Result as ImapResult};

// IMAP 服务
pub use service::ImapService;

// 测试工具
pub use tests::{ConnectionTestResult, test_connection};

// 向后兼容别名
pub use AsyncImapClient as ImapClient;
