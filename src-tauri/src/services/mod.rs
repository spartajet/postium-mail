#![allow(deprecated)]

pub mod account_service;
pub mod email_service;
pub mod imap;
pub mod smtp_service;
pub mod search_service;
pub mod oauth_service;
pub mod folder_service;
pub mod sync_state_service;
pub mod sync_error_service;
pub mod sync_manager;

// 暂时排除旧的 imap_service
// pub mod imap_service_old;

pub use account_service::*;
pub use email_service::*;
pub use imap::*;
pub use smtp_service::*;
pub use search_service::*;
pub use oauth_service::*;
pub use folder_service::*;
pub use sync_state_service::*;
pub use sync_error_service::*;
pub use sync_manager::*;
