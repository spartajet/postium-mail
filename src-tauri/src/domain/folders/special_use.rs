use crate::domain::folders::FolderCategory;
use async_imap::imap_proto::NameAttribute;
use serde::{Deserialize, Serialize};
use specta::Type;

/// RFC 6154 SPECIAL-USE 标记（从 LIST attributes 提取）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum SpecialUseFlag {
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
    /// \All —— 通常对应归档视图（如 Gmail All Mail）
    All,
    /// \Flagged —— 星标虚拟邮箱，不映射到单一物理文件夹
    Flagged,
}

impl SpecialUseFlag {
    /// 从 IMAP NameAttribute 列表提取所有 SPECIAL-USE 标记。
    /// `async-imap`/`imap-proto` 已把 RFC 6154 标记解析为强类型变体，直接 match。
    pub fn from_attributes(attrs: &[NameAttribute<'_>]) -> Vec<Self> {
        attrs
            .iter()
            .filter_map(|attr| match attr {
                NameAttribute::Sent => Some(Self::Sent),
                NameAttribute::Drafts => Some(Self::Drafts),
                NameAttribute::Junk => Some(Self::Junk),
                NameAttribute::Trash => Some(Self::Trash),
                NameAttribute::Archive => Some(Self::Archive),
                NameAttribute::All => Some(Self::All),
                NameAttribute::Flagged => Some(Self::Flagged),
                _ => None,
            })
            .collect()
    }

    /// 转为标准类别。All 归 Archive，Flagged 不映射（None）。
    pub fn to_category(&self) -> Option<FolderCategory> {
        match self {
            Self::Sent => Some(FolderCategory::Sent),
            Self::Drafts => Some(FolderCategory::Drafts),
            Self::Junk => Some(FolderCategory::Junk),
            Self::Trash => Some(FolderCategory::Trash),
            Self::Archive => Some(FolderCategory::Archive),
            // Gmail 的 \All Mail 用 \All 标记，按归档处理
            Self::All => Some(FolderCategory::Archive),
            // 星标是跨文件夹虚拟视图，不映射单一文件夹
            Self::Flagged => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_typed_special_use_variants() {
        let attrs = vec![NameAttribute::Sent, NameAttribute::NoSelect];
        let flags = SpecialUseFlag::from_attributes(&attrs);
        assert_eq!(flags, vec![SpecialUseFlag::Sent]);
    }

    #[test]
    fn all_maps_to_archive() {
        assert_eq!(
            SpecialUseFlag::All.to_category(),
            Some(FolderCategory::Archive)
        );
    }

    #[test]
    fn flagged_does_not_map_to_folder() {
        assert_eq!(SpecialUseFlag::Flagged.to_category(), None);
    }

    #[test]
    fn plain_attributes_yield_no_flags() {
        // 注意：imap-proto 0.16.7 无 HasChildren 变体，用 NoInferiors（RFC 3501）。
        let attrs = vec![NameAttribute::Marked, NameAttribute::NoInferiors];
        assert!(SpecialUseFlag::from_attributes(&attrs).is_empty());
    }
}
