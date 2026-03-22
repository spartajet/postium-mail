//! 文件夹服务类型
//!
//! 定义文件夹相关的数据传输对象和请求/响应类型。

use crate::storage::models::folder_sync_state;

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
    fn test_folder_sync_state_dto_conversion() {
        let model = folder_sync_state::Model {
            id: 1,
            account_id: 1,
            imap_name: "INBOX".to_string(),
            uidvalidity: Some(12345),
            uidnext: Some(101),
            highest_modseq: Some(67890),
            synced_at: Some(1234567890),
            created_at: Some(1234567890),
            updated_at: Some(1234567890),
            folder_type: Some("".to_string()),
        };

        let dto = FolderSyncStateDto::from(model);
        assert_eq!(dto.id, 1);
        assert_eq!(dto.imap_name, "INBOX");
    }

    #[test]
    fn test_standard_folder() {
        assert_eq!(StandardFolder::Inbox.as_str(), "inbox");
        assert_eq!(
            StandardFolder::from_folder_name("inbox"),
            StandardFolder::Inbox
        );
        assert_eq!(
            StandardFolder::from_folder_name("custom"),
            StandardFolder::Custom("custom".to_string())
        );
    }
}
