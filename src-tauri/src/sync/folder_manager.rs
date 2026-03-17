//! 文件夹管理器
//!
//! 管理邮件文件夹，支持 RFC 6154 Special-Use 文件夹

use crate::error::{MailError, Result};
use crate::models::folder;
use sea_orm::{DbConn, EntityTrait, ActiveModelTrait, Set};
use std::sync::Arc;
use std::collections::HashMap;

/// RFC 6154 Special-Use 文件夹类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialUse {
    /// 收件箱 (\Inbox)
    Inbox,
    /// 草稿箱 (\Drafts)
    Drafts,
    /// 已发送 (\Sent)
    Sent,
    /// 垃圾箱 (\Trash)
    Trash,
    /// 垃圾邮件 (\Junk)
    Junk,
    /// 重要邮件 (\Important)
    Important,
    /// 归档 (\Archive)
    Archive,
    /// 全部邮件 (\All)
    All,
    /// 标记邮件 (\Flagged)
    Flagged,
    /// 普通文件夹（无特殊用途）
    Normal,
}

impl SpecialUse {
    /// 从 IMAP flags 解析 Special-Use 类型
    pub fn from_imap_flags(flags: &[String]) -> Self {
        for flag in flags {
            match flag.as_str() {
                "\\Inbox" => return SpecialUse::Inbox,
                "\\Drafts" => return SpecialUse::Drafts,
                "\\Sent" => return SpecialUse::Sent,
                "\\Trash" => return SpecialUse::Trash,
                "\\Junk" => return SpecialUse::Junk,
                "\\Important" => return SpecialUse::Important,
                "\\Archive" => return SpecialUse::Archive,
                "\\All" => return SpecialUse::All,
                "\\Flagged" => return SpecialUse::Flagged,
                _ => continue,
            }
        }
        SpecialUse::Normal
    }

    /// 获取标准文件夹名称
    pub fn standard_name(&self) -> &str {
        match self {
            SpecialUse::Inbox => "inbox",
            SpecialUse::Drafts => "drafts",
            SpecialUse::Sent => "sent",
            SpecialUse::Trash => "trash",
            SpecialUse::Junk => "junk",
            SpecialUse::Important => "important",
            SpecialUse::Archive => "archive",
            SpecialUse::All => "all",
            SpecialUse::Flagged => "flagged",
            SpecialUse::Normal => "other",
        }
    }

    /// 判断是否为系统文件夹
    pub fn is_system_folder(&self) -> bool {
        !matches!(self, SpecialUse::Normal)
    }
}

/// IMAP 文件夹信息
#[derive(Debug, Clone)]
pub struct ImapFolder {
    /// IMAP 服务器原始名称
    pub imap_name: String,
    /// Special-Use 类型
    pub special_use: SpecialUse,
    /// 文件夹属性（RFC 3501）
    pub attributes: Vec<String>,
    /// UIDVALIDITY
    pub uidvalidity: Option<u64>,
    /// 预期的下一个 UID
    pub uidnext: Option<u64>,
    /// 最高 MODSEQ（CONDSTORE）
    pub highest_modseq: Option<u64>,
}

/// 文件夹同步结果
#[derive(Debug, Clone)]
pub struct FolderSyncResult {
    /// 新增文件夹数量
    pub new_folders: usize,
    /// 更新文件夹数量
    pub updated_folders: usize,
    /// 删除文件夹数量
    pub deleted_folders: usize,
    /// 总文件夹数量
    pub total_folders: usize,
}

/// 文件夹管理器
///
/// 负责管理邮件文件夹的同步和更新
pub struct FolderManager {
    db: Arc<DbConn>,
}

impl FolderManager {
    /// 创建新的文件夹管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 同步文件夹列表
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `imap_folders` - 从 IMAP 服务器获取的文件夹列表
    ///
    /// # 返回
    ///
    /// 返回同步结果
    pub async fn sync_folders(
        &self,
        account_id: i32,
        imap_folders: Vec<ImapFolder>,
    ) -> Result<FolderSyncResult> {
        tracing::info!(
            "开始同步文件夹: account_id={}, count={}",
            account_id,
            imap_folders.len()
        );

        // 1. 获取本地文件夹列表
        let local_folders = self.get_local_folders(account_id).await?;
        let local_folder_map: HashMap<String, folder::Model> = local_folders
            .into_iter()
            .map(|f| (f.imap_name.clone(), f))
            .collect();

        let mut new_count = 0;
        let mut updated_count = 0;

        // 2. 同步每个文件夹
        for imap_folder in imap_folders {
            if let Some(local_folder) = local_folder_map.get(&imap_folder.imap_name) {
                // 更新现有文件夹
                self.update_folder(local_folder, &imap_folder).await?;
                updated_count += 1;
            } else {
                // 创建新文件夹
                self.create_folder(account_id, &imap_folder).await?;
                new_count += 1;
            }
        }

        // 3. 检测已删除的文件夹（可选）
        // TODO: 实现删除检测逻辑

        let total_folders = new_count + updated_count;

        tracing::info!(
            "文件夹同步完成: account_id={}, new={}, updated={}, total={}",
            account_id,
            new_count,
            updated_count,
            total_folders
        );

        Ok(FolderSyncResult {
            new_folders: new_count,
            updated_folders: updated_count,
            deleted_folders: 0,
            total_folders,
        })
    }

    /// 获取本地文件夹列表
    async fn get_local_folders(&self, account_id: i32) -> Result<Vec<folder::Model>> {
        use sea_orm::{QueryFilter, ColumnTrait, QueryOrder, Order};

        let folders = folder::Entity::find()
            .filter(folder::Column::AccountId.eq(account_id))
            .order_by(folder::Column::Id, Order::Asc)
            .all(self.db.as_ref())
            .await?;

        Ok(folders)
    }

    /// 创建新文件夹
    async fn create_folder(&self, account_id: i32, imap_folder: &ImapFolder) -> Result<()> {
        use crate::models::folder::ActiveModel;

        let folder_active = ActiveModel {
            account_id: Set(account_id),
            name: Set(imap_folder.special_use.standard_name().to_string()),
            imap_name: Set(imap_folder.imap_name.clone()),
            parent_id: Set(None),
            attributes: Set(Some(serde_json::to_string(&imap_folder.attributes).unwrap_or_default())),
            email_count: Set(0),
            unread_count: Set(0),
            synced_at: Set(Some(chrono::Utc::now().timestamp())),
            uidvalidity: Set(imap_folder.uidvalidity.map(|v| v as i64)),
            uidnext: Set(imap_folder.uidnext.map(|v| v as i64)),
            highest_modseq: Set(imap_folder.highest_modseq.map(|v| v as i64)),
            ..Default::default()
        };

        folder_active
            .insert(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("创建文件夹失败: {}", e)))?;

        tracing::debug!(
            "创建文件夹: account_id={}, imap_name={}, special_use={:?}",
            account_id,
            imap_folder.imap_name,
            imap_folder.special_use
        );

        Ok(())
    }

    /// 更新现有文件夹
    async fn update_folder(&self, local_folder: &folder::Model, imap_folder: &ImapFolder) -> Result<()> {
        use crate::models::folder::ActiveModel;

        let mut folder_active: ActiveModel = local_folder.clone().into();

        // 更新 IMAP 元数据
        folder_active.uidvalidity = Set(imap_folder.uidvalidity.map(|v| v as i64));
        folder_active.uidnext = Set(imap_folder.uidnext.map(|v| v as i64));
        folder_active.highest_modseq = Set(imap_folder.highest_modseq.map(|v| v as i64));
        folder_active.synced_at = Set(Some(chrono::Utc::now().timestamp()));

        folder_active
            .update(self.db.as_ref())
            .await
            .map_err(|e| MailError::Internal(format!("更新文件夹失败: {}", e)))?;

        tracing::debug!(
            "更新文件夹: account_id={}, imap_name={}",
            local_folder.account_id,
            imap_folder.imap_name
        );

        Ok(())
    }

    /// 解析 IMAP LIST 响应中的文件夹
    ///
    /// # 参数
    ///
    /// * `name` - 文件夹名称
    /// * `flags` - IMAP 标志列表
    ///
    /// # 返回
    ///
    /// 返回 ImapFolder
    pub fn parse_imap_folder(name: String, flags: Vec<String>) -> ImapFolder {
        let special_use = SpecialUse::from_imap_flags(&flags);

        ImapFolder {
            imap_name: name.clone(),
            special_use,
            attributes: flags.clone(),
            uidvalidity: None,
            uidnext: None,
            highest_modseq: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_use_from_flags() {
        let flags = vec!["\\Sent".to_string()];
        assert_eq!(SpecialUse::from_imap_flags(&flags), SpecialUse::Sent);

        let flags = vec!["\\Drafts".to_string()];
        assert_eq!(SpecialUse::from_imap_flags(&flags), SpecialUse::Drafts);

        let flags = vec!["\\Trash".to_string()];
        assert_eq!(SpecialUse::from_imap_flags(&flags), SpecialUse::Trash);
    }

    #[test]
    fn test_special_use_standard_name() {
        assert_eq!(SpecialUse::Inbox.standard_name(), "inbox");
        assert_eq!(SpecialUse::Sent.standard_name(), "sent");
        assert_eq!(SpecialUse::Drafts.standard_name(), "drafts");
        assert_eq!(SpecialUse::Trash.standard_name(), "trash");
        assert_eq!(SpecialUse::Junk.standard_name(), "junk");
        assert_eq!(SpecialUse::Normal.standard_name(), "other");
    }

    #[test]
    fn test_special_use_is_system_folder() {
        assert!(SpecialUse::Inbox.is_system_folder());
        assert!(SpecialUse::Sent.is_system_folder());
        assert!(SpecialUse::Drafts.is_system_folder());
        assert!(!SpecialUse::Normal.is_system_folder());
    }

    #[test]
    fn test_parse_imap_folder() {
        let name = "INBOX".to_string();
        let flags = vec![];
        let folder = FolderManager::parse_imap_folder(name, flags);

        assert_eq!(folder.imap_name, "INBOX");
        assert_eq!(folder.special_use, SpecialUse::Normal);
    }
}
