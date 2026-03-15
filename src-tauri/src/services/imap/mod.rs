mod client;
mod error;
mod parser;
mod service;
mod tests;
mod types;

pub use client::{AsyncImapClient, three_months_ago_imap_format};
// 向后兼容别名
pub use AsyncImapClient as ImapClient;
pub use error::{ImapError, Result};
pub use service::ImapService;
pub use tests::{test_connection, ConnectionTestResult};
pub use types::{EmailAttachment, EmailData, EmailFlags, EmailHeader, FolderInfo, FolderMetadata, ImapAuth, SpecialUse};
