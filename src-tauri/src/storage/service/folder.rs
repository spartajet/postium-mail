//! 文件夹服务类型
//!
//! 定义文件夹相关的数据传输对象和请求/响应类型。

use crate::storage::models::{folder, folder_sync_state};

/// 文件夹数据传输对象
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FolderDto {
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub imap_name: String,
    pub parent_id: Option<i32>,
    pub email_count: i32,
    pub unread_count: i32,
    pub synced_at: Option<i64>,
    // IMAP 元数据
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub highest_modseq: Option<i64>,
}

impl From<folder::Model> for FolderDto {
    fn from(model: folder::Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            name: model.name,
            imap_name: model.imap_name,
            parent_id: model.parent_id,
            email_count: model.email_count,
            unread_count: model.unread_count,
            synced_at: model.synced_at,
            uidvalidity: model.uidvalidity,
            uidnext: model.uidnext,
            highest_modseq: model.highest_modseq,
        }
    }
}

/// 文件夹同步状态传输对象
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FolderSyncStateDto {
    pub id: i32,
    pub account_id: i32,
    pub imap_name: String,
    pub uidvalidity: Option<i64>,
    pub uidnext: Option<i64>,
    pub highest_modseq: Option<i64>,
    pub synced_at: Option<i64>,
}

impl From<folder_sync_state::Model> for FolderSyncStateDto {
    fn from(model: folder_sync_state::Model) -> Self {
        Self {
            id: model.id,
            account_id: model.account_id,
            imap_name: model.imap_name,
            uidvalidity: model.uidvalidity,
            uidnext: model.uidnext,
            highest_modseq: model.highest_modseq,
            synced_at: model.synced_at,
        }
    }
}

/// 标准文件夹名称枚举
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum StandardFolder {
    Inbox,
    Starred,
    Sent,
    Drafts,
    Spam,
    Trash,
    Archive,
    Custom(String),
}

impl StandardFolder {
    pub fn as_str(&self) -> &str {
        match self {
            StandardFolder::Inbox => "inbox",
            StandardFolder::Starred => "starred",
            StandardFolder::Sent => "sent",
            StandardFolder::Drafts => "drafts",
            StandardFolder::Spam => "spam",
            StandardFolder::Trash => "trash",
            StandardFolder::Archive => "archive",
            StandardFolder::Custom(name) => name,
        }
    }

    pub fn from_folder_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "inbox" => StandardFolder::Inbox,
            "starred" => StandardFolder::Starred,
            "sent" => StandardFolder::Sent,
            "drafts" => StandardFolder::Drafts,
            "spam" => StandardFolder::Spam,
            "trash" => StandardFolder::Trash,
            "archive" => StandardFolder::Archive,
            other => StandardFolder::Custom(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folder_dto_conversion() {
        let model = folder::Model {
            id: 1,
            account_id: 1,
            name: "inbox".to_string(),
            imap_name: "INBOX".to_string(),
            parent_id: None,
            attributes: None,
            email_count: 100,
            unread_count: 5,
            synced_at: Some(1234567890),
            uidvalidity: Some(12345),
            uidnext: Some(101),
            highest_modseq: Some(67890),
        };

        let dto = FolderDto::from(model);
        assert_eq!(dto.id, 1);
        assert_eq!(dto.name, "inbox");
        assert_eq!(dto.imap_name, "INBOX");
    }

    #[test]
    fn test_standard_folder() {
        assert_eq!(StandardFolder::Inbox.as_str(), "inbox");
        assert_eq!(StandardFolder::from_folder_name("inbox"), StandardFolder::Inbox);
        assert_eq!(StandardFolder::from_folder_name("custom"), StandardFolder::Custom("custom".to_string()));
    }
}
