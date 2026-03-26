//! 同步准备结果
//!
//! 统一全量同步和增量同步的准备结果类型

use crate::sync::sync_manager::{DeltaSyncResult, SyncStrategy};

/// 同步元数据
///
/// 根据同步策略包含不同的元数据
#[derive(Debug, Clone)]
pub enum SyncMetadata {
    /// 全量同步元数据
    Full {
        /// 文件夹的 UIDVALIDITY
        uidvalidity: u64,
        /// 文件夹的 UIDNEXT
        uidnext: u64,
    },
    /// 增量同步元数据
    Incremental {
        /// 上次同步的最高 UID
        last_sync_uid: u32,
    },
}

/// 统一的同步准备结果
///
/// 包含执行同步所需的所有数据
#[derive(Debug)]
pub struct SyncPreparation {
    /// 服务器上需要同步的 UID 列表
    pub server_uids: Vec<u32>,
    /// 同步策略
    pub strategy: SyncStrategy,
    /// 同步元数据
    pub metadata: SyncMetadata,
}

impl SyncPreparation {
    /// 创建空的准备结果
    pub fn empty(strategy: SyncStrategy) -> Self {
        Self {
            server_uids: Vec::new(),
            strategy,
            metadata: SyncMetadata::Incremental { last_sync_uid: 0 },
        }
    }

    /// 是否需要同步
    pub fn needs_sync(&self) -> bool {
        !self.server_uids.is_empty()
    }

    /// 获取服务器 UID 列表
    pub fn server_uids(&self) -> &[u32] {
        &self.server_uids
    }

    /// 获取同步策略
    pub fn strategy(&self) -> SyncStrategy {
        self.strategy
    }
}

// 从 FullSyncPreparation 转换
impl From<(FullSyncPreparation, SyncStrategy)> for SyncPreparation {
    fn from((prep, strategy): (FullSyncPreparation, SyncStrategy)) -> Self {
        Self {
            server_uids: prep.server_uids,
            strategy,
            metadata: SyncMetadata::Full {
                uidvalidity: prep.uidvalidity,
                uidnext: prep.uidnext,
            },
        }
    }
}

// 从 IncrementalSyncPreparation 转换
impl From<(IncrementalSyncPreparation, SyncStrategy)> for SyncPreparation {
    fn from((prep, strategy): (IncrementalSyncPreparation, SyncStrategy)) -> Self {
        Self {
            server_uids: prep.server_uids,
            strategy,
            metadata: SyncMetadata::Incremental {
                last_sync_uid: prep.last_sync_uid,
            },
        }
    }
}

/// 全量同步准备结果（向后兼容）
///
/// 保留此类型用于向后兼容，新代码应使用 `SyncPreparation`
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
}

/// 增量同步准备结果（向后兼容）
///
/// 保留此类型用于向后兼容，新代码应使用 `SyncPreparation`
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

    /// 转换为空的 DeltaSyncResult
    pub fn to_empty_result(&self) -> DeltaSyncResult {
        DeltaSyncResult::empty(SyncStrategy::UidSearch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_preparation_empty() {
        let prep = SyncPreparation::empty(SyncStrategy::FullSync);
        assert!(prep.server_uids.is_empty());
        assert!(!prep.needs_sync());
        assert_eq!(prep.strategy(), SyncStrategy::FullSync);
    }

    #[test]
    fn test_sync_preparation_needs_sync() {
        let prep = SyncPreparation {
            server_uids: vec![100, 200],
            strategy: SyncStrategy::UidSearch,
            metadata: SyncMetadata::Incremental { last_sync_uid: 50 },
        };
        assert!(prep.needs_sync());
        assert_eq!(prep.server_uids().len(), 2);
    }

    #[test]
    fn test_from_full_sync_preparation() {
        let full_prep = FullSyncPreparation {
            server_uids: vec![1, 2, 3],
            uidvalidity: 12345,
            uidnext: 67890,
        };
        let prep = SyncPreparation::from((full_prep, SyncStrategy::FullSync));
        assert_eq!(prep.server_uids().len(), 3);
        assert!(matches!(prep.metadata, SyncMetadata::Full { .. }));
    }

    #[test]
    fn test_from_incremental_sync_preparation() {
        let inc_prep = IncrementalSyncPreparation {
            server_uids: vec![4, 5, 6],
            last_sync_uid: 100,
        };
        let prep = SyncPreparation::from((inc_prep, SyncStrategy::UidSearch));
        assert_eq!(prep.server_uids().len(), 3);
        assert!(matches!(prep.metadata, SyncMetadata::Incremental { .. }));
    }
}
