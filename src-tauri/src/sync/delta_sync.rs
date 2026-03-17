//! 增量同步
//!
//! 实现增量同步功能

use crate::error::Result;

/// 增量同步
#[allow(unused_variables)]
pub struct DeltaSync {
    // TODO: 实现增量同步逻辑
}

impl DeltaSync {
    pub fn new() -> Self {
        Self {}
    }

    /// 执行增量同步
    pub async fn sync(&self, account_id: i32) -> Result<()> {
        // TODO: 实现增量同步
        Ok(())
    }
}

impl Default for DeltaSync {
    fn default() -> Self {
        Self::new()
    }
}
