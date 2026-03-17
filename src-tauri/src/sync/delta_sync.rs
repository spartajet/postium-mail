//! 增量同步
//!
//! 实现高效的增量同步，支持 IMAP CONDSTORE 扩展

use crate::error::{MailError, Result};
use crate::sync::change_detector::{ChangeDetector, ChangeDetectionResult};
use sea_orm::DbConn;
use std::sync::Arc;

/// 同步策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStrategy {
    /// 使用 MODSEQ 增量同步（CONDSTORE）
    Condstore,

    /// 使用 UID 搜索对比
    UidSearch,

    /// 完整同步
    FullSync,
}

/// 同步结果
#[derive(Debug, Clone)]
pub struct DeltaSyncResult {
    /// 使用的同步策略
    pub strategy_used: SyncStrategy,

    /// 新邮件数量
    pub new_emails: usize,

    /// 修改邮件数量
    pub modified_emails: usize,

    /// 删除邮件数量
    pub deleted_emails: usize,

    /// 标志变更数量
    pub flags_changed: usize,

    /// 同步耗时（毫秒）
    pub duration_ms: u64,
}

impl DeltaSyncResult {
    /// 创建空的同步结果
    pub fn empty(strategy: SyncStrategy) -> Self {
        Self {
            strategy_used: strategy,
            new_emails: 0,
            modified_emails: 0,
            deleted_emails: 0,
            flags_changed: 0,
            duration_ms: 0,
        }
    }

    /// 是否有变更
    pub fn has_changes(&self) -> bool {
        self.new_emails > 0 || self.modified_emails > 0 || self.deleted_emails > 0
    }

    /// 变更总数
    pub fn total_changes(&self) -> usize {
        self.new_emails + self.modified_emails + self.deleted_emails
    }
}

/// 增量同步器
///
/// 负责高效的增量同步，自动选择最佳策略
pub struct DeltaSync {
    db: Arc<DbConn>,
    change_detector: ChangeDetector,
    // TODO: 添加更多依赖
    // imap_client: Arc<AsyncImapClient>,
    // auth_manager: Arc<AuthManager>,
}

impl DeltaSync {
    /// 创建新的增量同步器
    pub fn new(db: Arc<DbConn>) -> Self {
        let change_detector = ChangeDetector::new(db.clone());
        Self { db, change_detector }
    }

    /// 检查是否支持 CONDSTORE
    ///
    /// 通过 CAPABILITY 命令检测服务器是否支持 CONDSTORE 扩展
    pub async fn check_condstore_support(&self, _account_id: i32) -> Result<bool> {
        // TODO: 实现 CONDSTORE 支持检测
        // 1. 连接到 IMAP 服务器
        // 2. 执行 CAPABILITY 命令
        // 3. 检查是否包含 CONDSTORE
        Ok(false) // 暂时返回 false
    }

    /// 增量同步（简化版本）
    ///
    /// 不需要服务器 UID 列表的简化版本，用于向后兼容
    ///
    /// # 注意
    ///
    /// 此方法返回空结果，需要使用完整版本的 `sync_incremental`
    pub async fn sync_incremental_simple(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<DeltaSyncResult> {
        // TODO: 需要集成 IMAP 客户端才能获取服务器 UID
        // 临时实现：返回空结果
        tracing::warn!(
            "sync_incremental_simple 尚未集成 IMAP 客户端，返回空结果: account_id={}, folder={}",
            account_id,
            folder
        );
        Ok(DeltaSyncResult::empty(SyncStrategy::UidSearch))
    }

    /// 增量同步（主入口）
    ///
    /// 自动选择最佳同步策略：
    /// - 如果支持 CONDSTORE，使用 MODSEQ 增量同步
    /// - 否则，降级到 UID 搜索对比
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids` - 服务器上的 UID 列表
    /// * `last_sync_uid` - 上次同步的最高 UID（可选）
    /// * `supports_condstore` - 是否支持 CONDSTORE（由调用者检测）
    /// * `server_uids_with_flags` - 服务器 UID 和标志列表（可选，用于标志变更检测）
    pub async fn sync_incremental(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
        last_sync_uid: Option<u32>,
        supports_condstore: bool,
        server_uids_with_flags: Option<&[(u32, Vec<String>)]>,
    ) -> Result<DeltaSyncResult> {
        let start = std::time::Instant::now();

        // 根据支持情况选择策略
        let result = if supports_condstore {
            // TODO: 实现真正的 CONDSTORE 同步
            // 临时：降级到 UID 搜索
            self.sync_with_uid_search(account_id, folder, server_uids, last_sync_uid, server_uids_with_flags)
                .await?
        } else {
            self.sync_with_uid_search(account_id, folder, server_uids, last_sync_uid, server_uids_with_flags)
                .await?
        };

        // 记录耗时
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(DeltaSyncResult {
            duration_ms,
            ..result
        })
    }

    /// 使用 CONDSTORE 策略同步
    ///
    /// 使用 MODSEQ 进行增量同步，仅获取变更的邮件
    pub async fn sync_with_condstore(
        &self,
        _account_id: i32,
        _folder: &str,
    ) -> Result<DeltaSyncResult> {
        // TODO: 实现 CONDSTORE 同步逻辑
        // 1. 获取上一次同步的 highest_modseq
        // 2. 执行 SEARCH MODSEQ <last_modseq>:*
        // 3. 仅获取变更的邮件
        // 4. 更新 highest_modseq
        Ok(DeltaSyncResult::empty(SyncStrategy::Condstore))
    }

    /// 使用 UID 搜索策略同步
    ///
    /// 使用 UID 搜索对比，适用于不支持 CONDSTORE 的服务器
    ///
    /// # 参数
    ///
    /// * `account_id` - 账号 ID
    /// * `folder` - 文件夹名称
    /// * `server_uids` - 服务器上的 UID 列表（由调用者从 IMAP 服务器获取）
    /// * `last_sync_uid` - 上次同步的最高 UID（可选）
    /// * `server_uids_with_flags` - 服务器 UID 和标志列表（可选，用于标志变更检测）
    pub async fn sync_with_uid_search(
        &self,
        account_id: i32,
        folder: &str,
        server_uids: &[u32],
        last_sync_uid: Option<u32>,
        server_uids_with_flags: Option<&[(u32, Vec<String>)]>,
    ) -> Result<DeltaSyncResult> {
        // 1. 检测新邮件
        let new_emails = self
            .change_detector
            .detect_new_emails(account_id, folder, server_uids, last_sync_uid)
            .await?;

        // 2. 检测删除的邮件
        let deleted_emails = self
            .change_detector
            .detect_deletions(account_id, folder, server_uids)
            .await?;

        // 3. 检测标志变更（如果提供了服务器 flags）
        let modified_emails = if let Some(uids_with_flags) = server_uids_with_flags {
            self.change_detector
                .detect_flag_changes_uid_search(account_id, folder, uids_with_flags)
                .await?
        } else {
            Vec::new()
        };

        let flags_changed = modified_emails.len();

        tracing::info!(
            "UID 搜索同步完成: account_id={}, folder={}, new={}, deleted={}, modified={}",
            account_id,
            folder,
            new_emails.len(),
            deleted_emails.len(),
            modified_emails.len()
        );

        Ok(DeltaSyncResult {
            strategy_used: SyncStrategy::UidSearch,
            new_emails: new_emails.len(),
            modified_emails: modified_emails.len(),
            deleted_emails: deleted_emails.len(),
            flags_changed,
            duration_ms: 0, // 由调用者设置
        })
    }

    /// 完整同步
    ///
    /// 同步所有邮件，不使用任何增量优化
    pub async fn sync_full(
        &self,
        _account_id: i32,
        _folder: &str,
    ) -> Result<DeltaSyncResult> {
        // TODO: 实现完整同步逻辑
        // 1. 获取服务器所有邮件 UID
        // 2. 获取本地所有邮件 UID
        // 3. 对比并同步差异
        Ok(DeltaSyncResult::empty(SyncStrategy::FullSync))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_strategy_equality() {
        assert_eq!(SyncStrategy::Condstore, SyncStrategy::Condstore);
        assert_ne!(SyncStrategy::Condstore, SyncStrategy::UidSearch);
    }

    #[test]
    fn test_delta_sync_result_empty() {
        let result = DeltaSyncResult::empty(SyncStrategy::Condstore);
        assert!(!result.has_changes());
        assert_eq!(result.total_changes(), 0);
        assert_eq!(result.strategy_used, SyncStrategy::Condstore);
    }

    #[test]
    fn test_delta_sync_result_has_changes() {
        let mut result = DeltaSyncResult::empty(SyncStrategy::UidSearch);
        result.new_emails = 10;
        result.modified_emails = 5;

        assert!(result.has_changes());
        assert_eq!(result.total_changes(), 15);
    }
}
