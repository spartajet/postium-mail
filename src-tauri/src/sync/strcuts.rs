use serde::{Deserialize, Serialize};

/// 同步元数据
///
/// 根据同步策略包含不同的元数据
#[derive(Debug, Clone)]
pub enum SyncMode {
    /// 全量同步元数据
    Full { uidvalidity: u64 },
    /// 增量同步元数据
    Incremental {
        /// 上次同步的最高 UID
        last_sync_uid: i32,
    },
}

/// 同步进度信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncProgress {
    pub stage: SyncStage,
    pub folder: Option<String>,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

/// 同步阶段
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SyncStage {
    Connecting,
    SyncingFolders,
    SyncingEmails,
    Completed,
    Error,
}

/// 同步策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStrategy {
    /// 使用 UID 搜索对比
    UidSearch,

    /// 完整同步
    FullSync,
}

/// 同步结果
#[derive(Debug, Clone)]
pub struct SyncResult {
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
    /// 最后同步的uid
    pub last_sync_uid: u32,
}

impl SyncResult {
    /// 是否有变更
    pub fn has_changes(&self) -> bool {
        self.new_emails > 0 || self.modified_emails > 0 || self.deleted_emails > 0
    }

    /// 变更总数
    pub fn total_changes(&self) -> usize {
        self.new_emails + self.modified_emails + self.deleted_emails
    }
}
