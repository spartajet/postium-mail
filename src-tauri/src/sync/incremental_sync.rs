//! 增量同步引擎
//!
//! 在 UIDVALIDITY 未变化时使用。
//! 基于 last_sync_uid 获取新增邮件进行增量同步。

use crate::error::{MailError, Result};
use crate::protocols::imap::{AsyncImapClient, FolderMetadata};
use crate::sync::delta_sync::DeltaSyncResult;
use crate::sync::folder_manager::FolderManager;
use crate::sync::SyncStrategy;
use std::sync::Arc;

/// 增量同步引擎
///
/// 负责在 UIDVALIDITY 未变化时执行增量同步。
/// 基于 last_sync_uid 获取新增邮件。
pub struct IncrementalSyncEngine {
    folder_manager: Arc<FolderManager>,
}

impl IncrementalSyncEngine {
    /// 创建新的增量同步引擎
    pub fn new(folder_manager: Arc<FolderManager>) -> Self {
        Self { folder_manager }
    }

    /// 执行增量同步
    ///
    /// 在 UIDVALIDITY 未变化时调用，仅同步新增和修改的邮件。
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
    /// 返回增量同步所需的 UID 列表
    pub async fn prepare_sync(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut AsyncImapClient,
        _metadata: &FolderMetadata,
    ) -> Result<IncrementalSyncPreparation> {
        tracing::info!(
            "准备增量同步: account_id={}, folder={}",
            account_id,
            folder
        );

        // 1. 获取本地 last_sync_uid
        let last_sync_uid = self
            .folder_manager
            .get_sync_state(account_id, folder)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.last_sync_uid)
            .map(|v| v as u32)
            .unwrap_or(0);

        tracing::info!(
            "增量同步，获取 last UID > {} 的邮件: folder={}",
            last_sync_uid,
            folder
        );

        // 2. 获取新增邮件 UID 列表
        let server_uids = imap_client
            .list_uids_after(folder, last_sync_uid)
            .await
            .map_err(|e| MailError::Internal(format!("获取服务器 UID 列表失败: {}", e)))?;

        tracing::info!(
            "增量同步准备完成: folder={}, 需同步邮件数={}",
            folder,
            server_uids.len()
        );

        Ok(IncrementalSyncPreparation {
            server_uids,
            last_sync_uid,
        })
    }

    /// 获取文件夹管理器引用
    pub fn folder_manager(&self) -> &Arc<FolderManager> {
        &self.folder_manager
    }
}

/// 增量同步准备结果
///
/// 包含增量同步所需的所有数据
#[derive(Debug)]
pub struct IncrementalSyncPreparation {
    /// 服务器上需要同步的 UID 列表
    pub server_uids: Vec<u32>,
    /// 上次同步的最高 UID
    pub last_sync_uid: u32,
}

impl IncrementalSyncPreparation {
    /// 创建空的准备结果
    pub fn empty() -> Self {
        Self {
            server_uids: Vec::new(),
            last_sync_uid: 0,
        }
    }

    /// 是否需要同步
    pub fn needs_sync(&self) -> bool {
        !self.server_uids.is_empty()
    }

    /// 转换为空的同步结果（用于跳过同步时）
    pub fn to_empty_result(&self) -> DeltaSyncResult {
        DeltaSyncResult::empty(SyncStrategy::UidSearch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_sync_preparation_empty() {
        let prep = IncrementalSyncPreparation::empty();
        assert!(prep.server_uids.is_empty());
        assert_eq!(prep.last_sync_uid, 0);
        assert!(!prep.needs_sync());
    }

    #[test]
    fn test_incremental_sync_preparation_needs_sync() {
        let prep = IncrementalSyncPreparation {
            server_uids: vec![100, 200],
            last_sync_uid: 50,
        };
        assert!(prep.needs_sync());
    }

    #[test]
    fn test_incremental_sync_preparation_to_empty_result() {
        let prep = IncrementalSyncPreparation::empty();
        let result = prep.to_empty_result();
        assert_eq!(result.new_emails, 0);
        assert_eq!(result.strategy_used, SyncStrategy::UidSearch);
    }
}
