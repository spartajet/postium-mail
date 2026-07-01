use crate::service::email_service::EmailCategory;
use serde::{Deserialize, Serialize};

/// 标准文件夹类别（引擎内部统一用此类型，垃圾邮件为 Junk）
///
/// 注意：对外 API 的 [`EmailCategory`] 里垃圾邮件叫 `Spam`，二者通过
/// `from_email_category` / `to_email_category` 互转（Junk ↔ Spam）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FolderCategory {
    Inbox,
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
}

impl FolderCategory {
    /// 类别固定优先级（同分时按此序，Inbox 最高）
    pub fn priority(self) -> u8 {
        match self {
            Self::Inbox => 0,
            Self::Sent => 1,
            Self::Drafts => 2,
            Self::Junk => 3,
            Self::Trash => 4,
            Self::Archive => 5,
        }
    }

    /// 从对外 EmailCategory 转换。Starred 无对应文件夹，返回 None。
    pub fn from_email_category(cat: &EmailCategory) -> Option<Self> {
        match cat {
            EmailCategory::Inbox => Some(Self::Inbox),
            EmailCategory::Sent => Some(Self::Sent),
            EmailCategory::Drafts => Some(Self::Drafts),
            EmailCategory::Spam => Some(Self::Junk),
            EmailCategory::Trash => Some(Self::Trash),
            EmailCategory::Archive => Some(Self::Archive),
            EmailCategory::Starred => None,
        }
    }

    /// 转回对外 EmailCategory。
    pub fn to_email_category(self) -> EmailCategory {
        match self {
            Self::Inbox => EmailCategory::Inbox,
            Self::Sent => EmailCategory::Sent,
            Self::Drafts => EmailCategory::Drafts,
            Self::Junk => EmailCategory::Spam,
            Self::Trash => EmailCategory::Trash,
            Self::Archive => EmailCategory::Archive,
        }
    }

    /// 数据库存储用稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Sent => "sent",
            Self::Drafts => "drafts",
            Self::Junk => "spam",
            Self::Trash => "trash",
            Self::Archive => "archive",
        }
    }

    /// 从数据库/API 字符串恢复分类。
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "inbox" => Some(Self::Inbox),
            "sent" => Some(Self::Sent),
            "drafts" => Some(Self::Drafts),
            "spam" | "junk" => Some(Self::Junk),
            "trash" => Some(Self::Trash),
            "archive" => Some(Self::Archive),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junk_maps_to_spam_and_back() {
        let cat = FolderCategory::Junk;
        assert_eq!(cat.to_email_category(), EmailCategory::Spam);
        assert_eq!(
            FolderCategory::from_email_category(&EmailCategory::Spam),
            Some(FolderCategory::Junk)
        );
    }

    #[test]
    fn starred_has_no_folder_category() {
        assert_eq!(
            FolderCategory::from_email_category(&EmailCategory::Starred),
            None
        );
    }

    #[test]
    fn inbox_has_highest_priority() {
        assert!(FolderCategory::Inbox.priority() < FolderCategory::Archive.priority());
    }
}
