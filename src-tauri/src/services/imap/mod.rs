mod client;
mod error;
mod parser;
mod service;
mod tests;
mod types;

pub use client::{AsyncImapClient, one_year_ago_imap_format};
pub use error::{ImapError, Result};
pub use service::ImapService;
pub use tests::{test_connection, ConnectionTestResult};
pub use types::{EmailData, EmailFlags, FolderInfo, ImapAuth, SpecialUse};
