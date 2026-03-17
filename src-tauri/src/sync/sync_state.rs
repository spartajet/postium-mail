//! 同步状态管理
//!
//! 管理同步状态

use crate::error::Result;

/// 同步状态管理器
#[allow(unused_variables)]
pub struct SyncStateManager {
    // TODO: 实现同步状态管理逻辑
}

impl SyncStateManager {
    pub fn new() -> Self {
        Self {}
    }

    /// 获取同步状态
    pub async fn get_state(&self, account_id: i32, folder: &str) -> Result<SyncState> {
        // TODO: 实现状态获取
        Ok(SyncState::Idle)
    }
}

impl Default for SyncStateManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 同步状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// 空闲
    Idle,
    /// 同步中
    Syncing,
    /// 错误
    Error,
}
