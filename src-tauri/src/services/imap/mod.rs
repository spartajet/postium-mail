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
