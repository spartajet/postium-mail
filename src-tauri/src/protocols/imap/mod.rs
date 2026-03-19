//! IMAP 协议实现
//!
//! 提供完整的 IMAP 客户端功能，支持：
//! - 传统密码认证
//! - OAuth2/XOAUTH2 认证
//! - CONDSTORE 增量同步
//! - IDLE 实时通知
//!
//! # 模块结构
//!
//! - `client` - IMAP 客户端核心实现
//! - `types` - 数据类型定义
//! - `error` - 错误类型定义
//! - `auth` - 认证类型定义
//! - `condstore` - CONDSTORE 增量同步支持
//! - `idle` - IDLE 实时通知支持
//! - `parser` - 邮件解析工具
//! - `raw_commands` - 原始 IMAP 命令
//! - `service` - IMAP 服务包装
//! - `tests` - 测试工具

mod auth;
mod client;
mod condstore_helpers;
mod error;
pub mod idle_manager;
mod parser;
mod raw_commands;
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

// CONDSTORE 支持
pub use condstore_helpers::CondstoreCommands;

// IDLE 管理
pub use idle_manager::ImapIdleManager;

// IMAP 服务
pub use service::ImapService;

// 测试工具
pub use tests::{test_connection, ConnectionTestResult};

// 向后兼容别名
pub use AsyncImapClient as ImapClient;
