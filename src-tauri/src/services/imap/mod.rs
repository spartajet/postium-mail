//! IMAP 客户端实现
//!
//! # 🚧 迁移状态
//!
//! 此模块将迁移到 `protocols::imap`，遵循新架构设计。
//!
//! ## 迁移进度
//!
//! - ✅ CONDSTORE 支持已提取为独立模块 (`condstore_helpers`)
//! - ✅ OAuth2 认证已改用 `providers::generate_xoauth2_string`
//! - ⏳ 主客户端逻辑待迁移到 `protocols::imap::client`
//! - ⏳ `service.rs` 待迁移到 `protocols::imap::session_manager`
//!
//! ## 使用说明
//!
//! 当前模块暂时保留，供 `services` 层其他模块使用。
//! 迁移完成后，此模块将被标记为 deprecated。

mod client;
mod condstore_helpers;
mod error;
pub mod idle_manager;
mod parser;
mod service;
mod tests;
mod types;

pub use client::{AsyncImapClient, three_months_ago_imap_format};
pub use condstore_helpers::CondstoreCommands;
// 向后兼容别名
pub use AsyncImapClient as ImapClient;
pub use error::{ImapError, Result};
pub use idle_manager::ImapIdleManager;
pub use service::ImapService;
pub use tests::{test_connection, ConnectionTestResult};
pub use types::{
    EmailAttachment, EmailData, EmailFlags, EmailHeader, FolderInfo, FolderMetadata, ImapAuth,
    IdleEvent, IdleHandle, IdleState, ReconnectConfig, SpecialUse,
};
