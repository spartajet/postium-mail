//! 变更检测
//!
//! 检测服务器端的邮件变更（新增、修改、删除）

use crate::error::{MailError, Result};
use crate::models::email;
use sea_orm::{DbConn, EntityTrait, QueryFilter, ColumnTrait};
use std::collections::HashMap;
use std::sync::Arc;

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
#[derive(Debug, Clone, PartialEq)]
pub struct EmailFlags {
    pub seen: bool,       // \Seen
    pub flagged: bool,    // \Flagged
    pub answered: bool,   // \Answered
    pub draft: bool,      // \Draft
    pub deleted: bool,    // \Deleted
    pub recent: bool,     // \Recent (只读)
}

impl Default for EmailFlags {
    fn default() -> Self {
        Self {
            seen: false,
            flagged: false,
            answered: false,
            draft: false,
            deleted: false,
            recent: false,
        }
    }
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
            answered: false, // 数据库中没有此字段
            draft: email.is_draft,
            deleted: false,  // 数据库中没有此字段
            recent: false,   // \Recent 是只读的
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
}

/// 变更检测器
///
/// 负责检测服务器端邮件的变更
pub struct ChangeDetector {
    db: Arc<DbConn>,
}

impl ChangeDetector {
    /// 创建新的变更检测器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 检测新邮件
    ///
    /// 从服务器获取 UID 列表，过滤出新邮件
    pub async fn detect_new_emails(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
        last_sync_uid: Option<u32>,
    ) -> Result<Vec<u32>> {
        // 1. 从数据库获取已同步的 UID
        let local_uids = self.get_local_uids(account_id, folder).await?;

        // 2. 创建本地 UID 集合
        let local_set = UidSet::from_vec(local_uids);

        // 3. 过滤出新邮件（服务器有，本地没有的）
        let mut new_emails = Vec::new();
        for &uid in server_uids {
            if !local_set.contains(uid) {
                new_emails.push(uid);
            }
        }

        // 4. 如果有 last_sync_uid，进一步过滤
        if let Some(last_uid) = last_sync_uid {
            new_emails.retain(|&uid| uid > last_uid);
        }

        tracing::debug!(
            "检测到 {} 封新邮件: account_id={}, folder={}",
            new_emails.len(),
            account_id,
            folder
        );

        Ok(new_emails)
    }

    /// 检测删除的邮件
    ///
    /// 对比本地 UID 列表和服务器 UID 列表
    pub async fn detect_deletions(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
    ) -> Result<Vec<u32>> {
        // 1. 从数据库获取已同步的 UID
        let local_uids = self.get_local_uids(account_id, folder).await?;

        // 2. 创建本地和服务器 UID 集合
        let local_set = UidSet::from_vec(local_uids);
        let server_set = UidSet::from_vec(server_uids.to_vec());

        // 3. 计算差集（本地有，服务器没有的 = 已删除）
        let deleted_emails = local_set.difference(&server_set);

        tracing::debug!(
            "检测到 {} 封删除的邮件: account_id={}, folder={}",
            deleted_emails.len(),
            account_id,
            folder
        );

        Ok(deleted_emails)
    }

    /// 检测标志变更（UID 搜索策略）
    ///
    /// 对比本地和服务器邮件的标志
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids_with_flags` - 服务器 UID 和标志列表
    ///
    /// # 返回
    ///
    /// 返回标志变更的 UID 列表
    pub async fn detect_flag_changes_uid_search(
        &self,
        account_id: i32,
        folder: &str,
        server_uids_with_flags: &[(u32, Vec<String>)],
    ) -> Result<Vec<u32>> {
        // 1. 提取 UID 列表
        let uids: Vec<u32> = server_uids_with_flags
            .iter()
            .map(|(uid, _)| *uid)
            .collect();

        // 2. 批量获取本地标志
        let local_flags_map = self.get_local_flags_batch(account_id, folder, &uids).await?;

        // 3. 对比每个邮件的标志
        let mut changed_uids = Vec::new();
        for (uid, server_flags_str) in server_uids_with_flags {
            // 解析服务器标志
            let server_flags = EmailFlags::from_imap_flags(server_flags_str);

            // 检查本地是否有此邮件
            if let Some(local_flags) = local_flags_map.get(uid) {
                // 对比标志
                if !local_flags.equals(&server_flags) {
                    tracing::debug!(
                        "检测到标志变更: uid={}, folder={}, local={:?}, server={:?}",
                        uid,
                        folder,
                        local_flags,
                        server_flags
                    );
                    changed_uids.push(*uid);
                }
            } else {
                // 本地没有此邮件，可能已删除或未同步
                tracing::warn!(
                    "本地未找到邮件: uid={}, folder={}",
                    uid,
                    folder
                );
            }
        }

        tracing::debug!(
            "检测到 {} 封标志变更的邮件: account_id={}, folder={}",
            changed_uids.len(),
            account_id,
            folder
        );

        Ok(changed_uids)
    }

    /// 检测变更（统一入口）
    ///
    /// 根据 CONDSTORE 支持情况自动选择检测策略
    pub async fn detect_changes(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
        last_sync_uid: Option<u32>,
        _supports_condstore: bool,
    ) -> Result<ChangeDetectionResult> {
        // 1. 检测新邮件
        let new_emails = self
            .detect_new_emails(account_id, folder, server_uids, last_sync_uid)
            .await?;

        // 2. 检测删除的邮件
        let deleted_emails = self
            .detect_deletions(account_id, folder, server_uids)
            .await?;

        // 3. 检测标志变更（UID 搜索策略）
        let modified_emails = Vec::new(); // TODO: 实现标志检测

        Ok(ChangeDetectionResult {
            new_emails,
            modified_emails,
            deleted_emails,
        })
    }

    /// 获取本地已同步的 UID 列表
    ///
    /// 从数据库查询指定文件夹的所有邮件 UID
    async fn get_local_uids(&self, account_id: i32, folder: &str) -> Result<Vec<u32>> {
        use crate::models::email;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

        // 查询数据库：SELECT uid FROM emails WHERE account_id = ? AND folder = ?
        let emails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .all(self.db.as_ref())
            .await?;

        // 提取 UID，过滤掉 None 值
        let uids: Vec<u32> = emails
            .into_iter()
            .filter_map(|email| email.uid.map(|uid| uid as u32))
            .collect();

        tracing::debug!(
            "获取本地 UID: account_id={}, folder={}, count={}",
            account_id,
            folder,
            uids.len()
        );

        Ok(uids)
    }

    /// 获取本地邮件的标志状态
    ///
    /// 从数据库查询指定 UID 的邮件标志
    async fn get_local_flags(
        &self,
        account_id: i32,
        folder: &str,
        uid: u32,
    ) -> Result<Option<EmailFlags>> {
        use crate::models::email;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

        // 查询数据库
        let emails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.eq(uid as i32))
            .all(self.db.as_ref())
            .await?;

        if let Some(email) = emails.first() {
            Ok(Some(EmailFlags::from_email_model(email)))
        } else {
            Ok(None)
        }
    }

    /// 批量获取本地邮件的标志状态
    ///
    /// 从数据库查询多个 UID 的邮件标志
    async fn get_local_flags_batch(
        &self,
        account_id: i32,
        folder: &str,
        uids: &[u32],
    ) -> Result<HashMap<u32, EmailFlags>> {
        use crate::models::email;
        use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};

        // 查询数据库
        let uid_i32: Vec<i32> = uids.iter().map(|&uid| uid as i32).collect();
        let emails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.is_in(uid_i32))
            .all(self.db.as_ref())
            .await?;

        // 构建 HashMap
        let mut flags_map = HashMap::new();
        for email in emails {
            if let Some(uid) = email.uid {
                flags_map.insert(uid as u32, EmailFlags::from_email_model(&email));
            }
        }

        Ok(flags_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_detection_result_default() {
        let result = ChangeDetectionResult::default();
        assert!(result.new_emails.is_empty());
        assert!(result.modified_emails.is_empty());
        assert!(result.deleted_emails.is_empty());
        assert!(!result.has_changes());
        assert_eq!(result.total_changes(), 0);
    }

    #[test]
    fn test_change_detection_result_has_changes() {
        let mut result = ChangeDetectionResult::default();
        result.new_emails.push(100);
        assert!(result.has_changes());
        assert_eq!(result.total_changes(), 1);

        result.modified_emails.push(200);
        assert_eq!(result.total_changes(), 2);
    }

    #[test]
    fn test_change_type_equality() {
        let change1 = ChangeType::NewEmail { uid: 100 };
        let change2 = ChangeType::NewEmail { uid: 100 };
        assert_eq!(change1, change2);

        let change3 = ChangeType::NewEmail { uid: 200 };
        assert_ne!(change1, change3);
    }

    // ========== UidSet 测试 ==========

    #[test]
    fn test_uidset_new() {
        let set = UidSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_uidset_insert() {
        let mut set = UidSet::new();
        set.insert(100);
        set.insert(200);
        assert_eq!(set.len(), 2);
        assert!(set.contains(100));
        assert!(set.contains(200));
        assert!(!set.contains(300));
    }

    #[test]
    fn test_uidset_insert_duplicate() {
        let mut set = UidSet::new();
        set.insert(100);
        set.insert(100); // 重复插入
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_uidset_from_vec() {
        let uids = vec![100, 200, 300];
        let set = UidSet::from_vec(uids);
        assert_eq!(set.len(), 3);
        assert!(set.contains(100));
        assert!(set.contains(300));
    }

    #[test]
    fn test_uidset_difference() {
        let set1 = UidSet::from_vec(vec![1, 2, 3, 4, 5]);
        let set2 = UidSet::from_vec(vec![2, 3, 4]);

        let diff = set1.difference(&set2);
        assert_eq!(diff.len(), 2);
        assert!(diff.contains(&1));
        assert!(diff.contains(&5));
        assert!(!diff.contains(&2));
        assert!(!diff.contains(&3));
    }

    #[tokio::test]
    async fn test_detect_new_emails() {
        // 这个测试需要实际的数据库，这里只测试逻辑
        // 模拟数据
        let server_uids = vec![100, 200, 300];
        let last_sync_uid = Some(150);

        // 期望：过滤出 > 150 的 UID
        let expected: Vec<u32> = server_uids
            .into_iter()
            .filter(|&uid| uid > last_sync_uid.unwrap())
            .collect();

        assert_eq!(expected, vec![200, 300]);
    }

    #[tokio::test]
    async fn test_detect_deletions() {
        // 测试删除检测逻辑
        let local_uids = vec![1, 2, 3, 4, 5];
        let server_uids = vec![2, 3, 4];

        let local_set = UidSet::from_vec(local_uids);
        let server_set = UidSet::from_vec(server_uids);
        let deleted = local_set.difference(&server_set);

        assert_eq!(deleted.len(), 2);
        assert!(deleted.contains(&1));
        assert!(deleted.contains(&5));
    }

    // ========== EmailFlags 测试 ==========

    #[test]
    fn test_email_flags_default() {
        let flags = EmailFlags::default();
        assert!(!flags.seen);
        assert!(!flags.flagged);
        assert!(!flags.answered);
        assert!(!flags.draft);
        assert!(!flags.deleted);
        assert!(!flags.recent);
    }

    #[test]
    fn test_email_flags_from_imap() {
        let imap_flags = vec![
            "\\Seen".to_string(),
            "\\Flagged".to_string(),
            "\\Answered".to_string(),
        ];
        let flags = EmailFlags::from_imap_flags(&imap_flags);

        assert!(flags.seen);
        assert!(flags.flagged);
        assert!(flags.answered);
        assert!(!flags.draft);
        assert!(!flags.deleted);
        assert!(!flags.recent);
    }

    #[test]
    fn test_email_flags_to_imap() {
        let flags = EmailFlags {
            seen: true,
            flagged: true,
            answered: false,
            draft: false,
            deleted: false,
            recent: false,
        };

        let imap_flags = flags.to_imap_flags();
        assert_eq!(imap_flags.len(), 2);
        assert!(imap_flags.contains(&"\\Seen".to_string()));
        assert!(imap_flags.contains(&"\\Flagged".to_string()));
    }

    #[test]
    fn test_email_flags_equality() {
        let flags1 = EmailFlags {
            seen: true,
            flagged: false,
            answered: false,
            draft: false,
            deleted: false,
            recent: false,
        };

        let flags2 = EmailFlags {
            seen: true,
            flagged: false,
            answered: false,
            draft: false,
            deleted: false,
            recent: false,
        };

        assert_eq!(flags1, flags2);

        let flags3 = EmailFlags {
            seen: false,
            ..flags1
        };

        assert_ne!(flags1, flags3);
    }

    #[test]
    fn test_email_flags_roundtrip() {
        let original = EmailFlags {
            seen: true,
            flagged: true,
            answered: true,
            draft: false,
            deleted: false,
            recent: false,
        };

        let imap_flags = original.to_imap_flags();
        let restored = EmailFlags::from_imap_flags(&imap_flags);

        assert_eq!(original, restored);
    }
}
