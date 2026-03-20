//! 文件夹同步状态管理器
//!
//! 管理文件夹同步状态，不管理文件夹配置

use crate::error::{MailError, Result};
use crate::protocols::imap::FolderInfo as ImapFolderInfo;
use crate::storage::models::folder_sync_state;
use sea_orm::{ActiveModelTrait, ActiveValue, DbConn, EntityTrait, Set, ColumnTrait, QueryFilter};
use std::sync::Arc;

/// 文件夹同步状态管理器
///
/// 负责管理文件夹同步状态的更新
pub struct FolderManager {
    db: Arc<DbConn>,
}

impl FolderManager {
    /// 创建新的文件夹状态管理器
    pub fn new(db: Arc<DbConn>) -> Self {
        Self { db }
    }

    /// 更新文件夹同步状态
    ///
    /// 只更新同步时间戳，IMAP 元数据（uidvalidity, uidnext, highest_modseq）
    /// 需要在同步邮件时单独获取并更新。
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder_infos` - 从 IMAP 服务器获取的文件夹信息列表
    ///
    /// # 返回
    ///
    /// 返回更新结果
    pub async fn update_sync_states(
        &self,
        account_id: i32,
        folder_infos: &[ImapFolderInfo],
    ) -> Result<SyncStateUpdateResult> {
        tracing::info!(
            "开始更新文件夹同步状态: account_id={}, count={}",
            account_id,
            folder_infos.len()
        );

        let mut updated_count = 0;

        for info in folder_infos {
            // 使用 upsert（插入或更新）
            let existing_state = folder_sync_state::Entity::find()
                .filter(folder_sync_state::Column::AccountId.eq(account_id))
                .filter(folder_sync_state::Column::ImapName.eq(&info.name))
                .one(self.db.as_ref())
                .await?;

            let now = chrono::Utc::now().timestamp();

            if let Some(existing) = existing_state {
                // 更新现有记录
                let mut active: folder_sync_state::ActiveModel = existing.into();
                // IMAP 元数据需要在同步邮件时单独更新
                active.synced_at = Set(Some(now));
                active.updated_at = Set(Some(now));

                active.update(self.db.as_ref()).await
                    .map_err(|e| MailError::Internal(format!("更新文件夹同步状态失败: {}", e)))?;

                tracing::debug!(
                    "更新文件夹同步状态: account_id={}, imap_name={}",
                    account_id,
                    info.name
                );

                updated_count += 1;
            } else {
                // 创建新记录（IMAP 元数据初始为 None）
                let active = folder_sync_state::ActiveModel {
                    id: ActiveValue::NotSet, // 自增
                    account_id: Set(account_id),
                    imap_name: Set(info.name.clone()),
                    uidvalidity: Set(None),
                    uidnext: Set(None),
                    highest_modseq: Set(None),
                    synced_at: Set(Some(now)),
                    ..Default::default()
                };

                active.insert(self.db.as_ref()).await
                    .map_err(|e| MailError::Internal(format!("创建文件夹同步状态失败: {}", e)))?;

                tracing::debug!(
                    "创建文件夹同步状态: account_id={}, imap_name={}",
                    account_id,
                    info.name
                );

                updated_count += 1;
            }
        }

        tracing::info!(
            "文件夹同步状态更新完成: account_id={}, updated={}",
            account_id,
            updated_count
        );

        Ok(SyncStateUpdateResult {
            updated: updated_count,
        })
    }

    /// 更新文件夹的 IMAP 元数据
    ///
    /// 在同步邮件时调用，更新 uidvalidity, uidnext, highest_modseq
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `imap_name` - IMAP 文件夹名称
    /// * `uidvalidity` - IMAP UIDVALIDITY 值
    /// * `uidnext` - 预期的下一个 UID
    /// * `highest_modseq` - CONDSTORE 最高修改序列号
    pub async fn update_folder_metadata(
        &self,
        account_id: i32,
        imap_name: &str,
        uidvalidity: Option<u64>,
        uidnext: Option<u64>,
        highest_modseq: Option<u64>,
    ) -> Result<()> {
        let existing_state = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::ImapName.eq(imap_name))
            .one(self.db.as_ref())
            .await?;

        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = existing_state {
            // 更新现有记录
            let mut active: folder_sync_state::ActiveModel = existing.into();
            active.uidvalidity = Set(uidvalidity.map(|v| v as i64));
            active.uidnext = Set(uidnext.map(|v| v as i64));
            active.highest_modseq = Set(highest_modseq.map(|v| v as i64));
            active.synced_at = Set(Some(now));
            active.updated_at = Set(Some(now));

            active.update(self.db.as_ref()).await
                .map_err(|e| MailError::Internal(format!("更新文件夹元数据失败: {}", e)))?;
        } else {
            // 创建新记录
            let active = folder_sync_state::ActiveModel {
                id: ActiveValue::NotSet,
                account_id: Set(account_id),
                imap_name: Set(imap_name.to_string()),
                uidvalidity: Set(uidvalidity.map(|v| v as i64)),
                uidnext: Set(uidnext.map(|v| v as i64)),
                highest_modseq: Set(highest_modseq.map(|v| v as i64)),
                synced_at: Set(Some(now)),
                ..Default::default()
            };

            active.insert(self.db.as_ref()).await
                .map_err(|e| MailError::Internal(format!("创建文件夹元数据失败: {}", e)))?;
        }

        Ok(())
    }

    /// 获取文件夹同步状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `imap_name` - IMAP 文件夹名称
    ///
    /// # 返回
    ///
    /// 返回同步状态
    pub async fn get_sync_state(
        &self,
        account_id: i32,
        imap_name: &str,
    ) -> Result<Option<folder_sync_state::Model>> {
        let state = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .filter(folder_sync_state::Column::ImapName.eq(imap_name))
            .one(self.db.as_ref())
            .await?;

        Ok(state)
    }

    /// 获取账号的所有文件夹同步状态
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    ///
    /// # 返回
    ///
    /// 返回同步状态列表
    pub async fn get_all_sync_states(
        &self,
        account_id: i32,
    ) -> Result<Vec<folder_sync_state::Model>> {
        let states = folder_sync_state::Entity::find()
            .filter(folder_sync_state::Column::AccountId.eq(account_id))
            .all(self.db.as_ref())
            .await?;

        Ok(states)
    }
}

/// 同步状态更新结果
#[derive(Debug, Clone)]
pub struct SyncStateUpdateResult {
    /// 更新的文件夹数量
    pub updated: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_state_update_result() {
        let result = SyncStateUpdateResult { updated: 5 };
        assert_eq!(result.updated, 5);
    }
}
