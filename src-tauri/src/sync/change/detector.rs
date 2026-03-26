//! 变化检测器
//!
//! 检测服务器端的邮件变更（新增、修改、删除）

use crate::error::Result;
use sea_orm::DbConn;
use std::collections::HashMap;
use std::sync::Arc;

// 导入 types 模块中的类型
use super::types::{ChangeDetectionResult, EmailFlags, UidSet};

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
    /// * `server_flags` - 服务器端的标志映射（UID -> 标志列表）
    ///
    /// # 返回
    ///
    /// 返回标志发生变更的 UID 列表
    pub async fn detect_flag_changes(
        &self,
        account_id: i32,
        folder: &str,
        server_flags: &HashMap<u32, Vec<String>>,
    ) -> Result<Vec<u32>> {
        // 1. 从数据库获取本地邮件的标志
        let local_flags = self.get_local_flags(account_id, folder).await?;

        // 2. 对比标志
        let mut changed_uids = Vec::new();

        for (&uid, server_flag_list) in server_flags {
            let server_flags = EmailFlags::from_imap_flags(server_flag_list);

            if let Some(local_flag) = local_flags.get(&uid) {
                // 标志发生变更
                if !local_flag.equals(&server_flags) {
                    changed_uids.push(uid);
                    tracing::trace!("标志变更: uid={}, folder={}", uid, folder);
                }
            } else {
                // 新邮件（本地不存在）
                changed_uids.push(uid);
            }
        }

        tracing::debug!(
            "检测到 {} 封标志变更: account_id={}, folder={}",
            changed_uids.len(),
            account_id,
            folder
        );

        Ok(changed_uids)
    }

    /// 执行完整的变更检测
    ///
    /// 检测新邮件、删除邮件和标志变更
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids` - 服务器端的 UID 列表
    /// * `server_flags` - 服务器端的标志映射
    /// * `last_sync_uid` - 上次同步的最高 UID
    ///
    /// # 返回
    ///
    /// 返回变更检测结果
    pub async fn detect_changes(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
        server_flags: &HashMap<u32, Vec<String>>,
        last_sync_uid: Option<u32>,
    ) -> Result<ChangeDetectionResult> {
        // 1. 检测新邮件
        let new_emails = self
            .detect_new_emails(account_id, folder, server_uids, last_sync_uid)
            .await?;

        // 2. 检测删除邮件
        let deleted_emails = self
            .detect_deletions(account_id, folder, server_uids)
            .await?;

        // 3. 检测标志变更
        let modified_emails = self
            .detect_flag_changes(account_id, folder, server_flags)
            .await?;

        Ok(ChangeDetectionResult {
            new_emails,
            modified_emails,
            deleted_emails,
        })
    }

    /// 从数据库获取已同步的 UID 列表
    async fn get_local_uids(&self, account_id: i32, folder: &str) -> Result<Vec<u32>> {
        use crate::storage::models::email;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let emails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .order_by_asc(email::Column::Uid)
            .all(self.db.as_ref())
            .await
            .map_err(|e| {
                crate::error::MailError::Internal(format!("获取本地 UID 列表失败: {}", e))
            })?;

        let uids: Vec<u32> = emails
            .iter()
            .filter_map(|e| e.uid)
            .map(|uid| uid as u32)
            .collect();
        tracing::trace!(
            "获取本地 UID 列表: account_id={}, folder={}, count={}",
            account_id,
            folder,
            uids.len()
        );

        Ok(uids)
    }

    /// 从数据库获取本地邮件的标志
    async fn get_local_flags(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<HashMap<u32, EmailFlags>> {
        use crate::storage::models::email;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let emails = email::Entity::find()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .all(self.db.as_ref())
            .await
            .map_err(|e| crate::error::MailError::Internal(format!("获取本地标志失败: {}", e)))?;

        let mut flags_map = HashMap::new();
        for email in emails {
            if let Some(uid) = email.uid {
                let email_flags = EmailFlags::from_email_model(&email);
                flags_map.insert(uid as u32, email_flags);
            }
        }

        Ok(flags_map)
    }
}
