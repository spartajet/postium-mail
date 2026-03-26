//! 变化检测类型定义
//!
//! 包含 IMAP 标志、UID 集合、变更类型等核心数据结构

use crate::storage::models::email;

// ========== IMAP Flags 常量 ==========

/// IMAP 系统标志
pub mod imap_flags {
    /// 已读标志 (\Seen)
    pub const SEEN: &str = "\\Seen";
    /// 已删除标志 (\Deleted)
    pub const DELETED: &str = "\\Deleted";
    /// 已标记标志 (\Flagged)
    pub const FLAGGED: &str = "\\Flagged";
    /// 已回复标志 (\Answered)
    pub const ANSWERED: &str = "\\Answered";
    /// 草稿标志 (\Draft)
    pub const DRAFT: &str = "\\Draft";
    /// 最近标志 (\Recent) - 只读
    pub const RECENT: &str = "\\Recent";
}

/// 邮件标志状态
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EmailFlags {
    pub seen: bool,     // \Seen
    pub flagged: bool,  // \Flagged
    pub answered: bool, // \Answered
    pub draft: bool,    // \Draft
    pub deleted: bool,  // \Deleted
    pub recent: bool,   // \Recent (只读)
}

impl EmailFlags {
    /// 从 IMAP flag 字符串列表解析
    pub fn from_imap_flags(flags: &[String]) -> Self {
        let mut result = EmailFlags::default();
        for flag in flags {
            match flag.as_str() {
                imap_flags::SEEN => result.seen = true,
                imap_flags::FLAGGED => result.flagged = true,
                imap_flags::ANSWERED => result.answered = true,
                imap_flags::DRAFT => result.draft = true,
                imap_flags::DELETED => result.deleted = true,
                imap_flags::RECENT => result.recent = true,
                _ => {
                    // 忽略未知标志
                    tracing::debug!("未知的 IMAP 标志: {}", flag);
                }
            }
        }
        result
    }

    /// 转换为 IMAP flag 字符串列表
    pub fn to_imap_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if self.seen {
            flags.push(imap_flags::SEEN.to_string());
        }
        if self.flagged {
            flags.push(imap_flags::FLAGGED.to_string());
        }
        if self.answered {
            flags.push(imap_flags::ANSWERED.to_string());
        }
        if self.draft {
            flags.push(imap_flags::DRAFT.to_string());
        }
        if self.deleted {
            flags.push(imap_flags::DELETED.to_string());
        }
        if self.recent {
            flags.push(imap_flags::RECENT.to_string());
        }
        flags
    }

    /// 从数据库模型转换
    pub fn from_email_model(email: &email::Model) -> Self {
        Self {
            seen: email.is_read,
            flagged: email.is_starred,
            answered: email.is_answered,
            draft: email.is_draft,
            deleted: email.is_deleted,
            recent: false, // \Recent 是只读的
        }
    }

    /// 比较两个标志状态是否相同
    pub fn equals(&self, other: &Self) -> bool {
        self.seen == other.seen
            && self.flagged == other.flagged
            && self.answered == other.answered
            && self.draft == other.draft
            && self.deleted == other.deleted
        // recent 不参与比较（只读标志）
    }
}

/// UID 集合辅助类型
#[derive(Debug, Clone)]
pub struct UidSet {
    uids: Vec<u32>,
}

impl UidSet {
    /// 创建新的 UID 集合
    pub fn new() -> Self {
        Self { uids: Vec::new() }
    }

    /// 从 Vec 创建
    pub fn from_vec(uids: Vec<u32>) -> Self {
        Self { uids }
    }

    /// 添加 UID
    pub fn insert(&mut self, uid: u32) {
        if !self.uids.contains(&uid) {
            self.uids.push(uid);
        }
    }

    /// 检查是否包含 UID
    pub fn contains(&self, uid: u32) -> bool {
        self.uids.contains(&uid)
    }

    /// 获取所有 UID
    pub fn as_slice(&self) -> &[u32] {
        &self.uids
    }

    /// 计算差集（self - other）
    pub fn difference(&self, other: &UidSet) -> Vec<u32> {
        self.uids
            .iter()
            .filter(|uid| !other.contains(**uid))
            .copied()
            .collect()
    }

    /// 获取大小
    pub fn len(&self) -> usize {
        self.uids.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.uids.is_empty()
    }
}

impl Default for UidSet {
    fn default() -> Self {
        Self::new()
    }
}

/// 变更类型
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// 新邮件
    NewEmail { uid: u32 },

    /// 标志变更
    FlagsChanged {
        uid: u32,
        old_flags: Vec<String>,
        new_flags: Vec<String>,
    },

    /// 邮件删除
    EmailDeleted { uid: u32 },
}

/// 变更检测结果
#[derive(Debug, Clone, Default)]
pub struct ChangeDetectionResult {
    /// 新邮件 UID 列表
    pub new_emails: Vec<u32>,

    /// 修改邮件 UID 列表
    pub modified_emails: Vec<u32>,

    /// 删除邮件 UID 列表
    pub deleted_emails: Vec<u32>,
}

impl ChangeDetectionResult {
    /// 是否有变更
    pub fn has_changes(&self) -> bool {
        !self.new_emails.is_empty()
            || !self.modified_emails.is_empty()
            || !self.deleted_emails.is_empty()
    }

    /// 变更总数
    pub fn total_changes(&self) -> usize {
        self.new_emails.len() + self.modified_emails.len() + self.deleted_emails.len()
    }

    /// 创建空的变更结果
    pub fn empty() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_flags_from_imap() {
        let flags = vec!["\\Seen".to_string(), "\\Flagged".to_string()];
        let email_flags = EmailFlags::from_imap_flags(&flags);
        assert!(email_flags.seen);
        assert!(email_flags.flagged);
        assert!(!email_flags.answered);
    }

    #[test]
    fn test_email_flags_to_imap() {
        let flags = EmailFlags {
            seen: true,
            flagged: false,
            answered: true,
            draft: false,
            deleted: false,
            recent: false,
        };
        let imap_flags = flags.to_imap_flags();
        assert_eq!(imap_flags.len(), 2);
        assert!(imap_flags.contains(&"\\Seen".to_string()));
        assert!(imap_flags.contains(&"\\Answered".to_string()));
    }

    #[test]
    fn test_uid_set() {
        let mut set = UidSet::new();
        set.insert(1);
        set.insert(2);
        set.insert(1); // 重复
        assert_eq!(set.len(), 2);
        assert!(set.contains(1));
        assert!(!set.contains(3));
    }

    #[test]
    fn test_uid_set_difference() {
        let set1 = UidSet::from_vec(vec![1, 2, 3, 4]);
        let set2 = UidSet::from_vec(vec![2, 3]);
        let diff = set1.difference(&set2);
        assert_eq!(diff, vec![1, 4]);
    }

    #[test]
    fn test_change_detection_result() {
        let result = ChangeDetectionResult {
            new_emails: vec![1, 2],
            modified_emails: vec![3],
            deleted_emails: vec![],
        };
        assert!(result.has_changes());
        assert_eq!(result.total_changes(), 3);
    }

    #[test]
    fn test_change_detection_result_empty() {
        let result = ChangeDetectionResult::empty();
        assert!(!result.has_changes());
        assert_eq!(result.total_changes(), 0);
    }
}
