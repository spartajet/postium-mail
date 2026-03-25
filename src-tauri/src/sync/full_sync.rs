//! 全量同步引擎
//!
//! 当 UIDVALIDITY 变化或首次同步时使用。
//! 获取指定时间范围内的所有邮件进行完整同步。

use crate::error::{MailError, Result};
use crate::protocols::imap::{AsyncImapClient, FolderMetadata};
use crate::sync::delta_sync::DeltaSyncResult;
use crate::sync::folder_manager::FolderManager;
use crate::sync::SyncStrategy;
use std::sync::Arc;

/// 全量同步引擎
///
/// 负责在 UIDVALIDITY 变化或首次同步时执行完整的文件夹同步。
/// 通常获取近三个月的邮件进行同步。
pub struct FullSyncEngine {
    folder_manager: Arc<FolderManager>,
}

impl FullSyncEngine {
    /// 创建新的全量同步引擎
    pub fn new(folder_manager: Arc<FolderManager>) -> Self {
        Self { folder_manager }
    }

    /// 执行全量同步
    ///
    /// 当 UIDVALIDITY 变化或首次同步时调用。获取所有邮件进行完整同步。
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `imap_client` - IMAP 客户端引用
    /// * `metadata` - 文件夹元数据（已在调用方获取）
    ///
    /// # 返回
    ///
    /// 返回全量同步所需的 UID 列表
    pub async fn prepare_sync(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut AsyncImapClient,
        metadata: &FolderMetadata,
    ) -> Result<FullSyncPreparation> {
        tracing::info!(
            "准备全量同步: account_id={}, folder={}, uidvalidity={}",
            account_id,
            folder,
            metadata.uidvalidity
        );

        // 1. 更新文件夹元数据（全量同步特有）
        self.folder_manager
            .update_folder_metadata(
                account_id,
                folder,
                Some(metadata.uidvalidity),
                Some(metadata.uidnext),
            )
            .await
            .map_err(|e| {
                tracing::warn!("更新文件夹元数据失败: {}", e);
                e
            })?;

        tracing::debug!(
            "文件夹元数据已更新: uidvalidity={}, uidnext={}",
            metadata.uidvalidity,
            metadata.uidnext
        );

        // 2. 获取近三个月的邮件 UID
        let date_since = crate::protocols::imap::three_months_ago_imap_format();
        tracing::info!(
            "全量同步，获取三个月内的邮件: folder={}, date_since={}",
            folder,
            date_since
        );

        let server_uids = imap_client
            .list_uids_since(folder, &date_since)
            .await
            .map_err(|e| MailError::Internal(format!("获取服务器 UID 列表失败: {}", e)))?;

        tracing::info!(
            "全量同步准备完成: folder={}, 需同步邮件数={}",
            folder,
            server_uids.len()
        );

        Ok(FullSyncPreparation {
            server_uids,
            uidvalidity: metadata.uidvalidity,
            uidnext: metadata.uidnext,
        })
    }

    /// 获取文件夹管理器引用
    pub fn folder_manager(&self) -> &Arc<FolderManager> {
        &self.folder_manager
    }
}

/// 全量同步准备结果
///
/// 包含执行全量同步所需的所有数据
#[derive(Debug)]
pub struct FullSyncPreparation {
    /// 服务器上需要同步的 UID 列表
    pub server_uids: Vec<u32>,
    /// 文件夹的 UIDVALIDITY
    pub uidvalidity: u64,
    /// 文件夹的 UIDNEXT
    pub uidnext: u64,
}

impl FullSyncPreparation {
    /// 创建空的准备结果
    pub fn empty() -> Self {
        Self {
            server_uids: Vec::new(),
            uidvalidity: 0,
            uidnext: 0,
        }
    }

    /// 转换为空的同步结果（用于跳过同步时）
    pub fn to_empty_result(&self) -> DeltaSyncResult {
        DeltaSyncResult::empty(SyncStrategy::FullSync)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_sync_preparation_empty() {
        let prep = FullSyncPreparation::empty();
        assert!(prep.server_uids.is_empty());
        assert_eq!(prep.uidvalidity, 0);
        assert_eq!(prep.uidnext, 0);
    }

    #[test]
    fn test_full_sync_preparation_to_empty_result() {
        let prep = FullSyncPreparation::empty();
        let result = prep.to_empty_result();
        assert_eq!(result.new_emails, 0);
        assert_eq!(result.strategy_used, SyncStrategy::FullSync);
    }
}
