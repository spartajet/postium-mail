//! 变更检测
//!
//! 检测服务器端的邮件变更（新增、修改、删除）

use crate::error::Result;
use sea_orm::DbConn;
use std::sync::Arc;

/// 变更类型
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// 新邮件
    NewEmail { uid: u32 },

    /// 标志变更
    FlagsChanged {
        uid: u32,
        old_flags: Vec<String>,
        new_flags: Vec<String>,
    },

    /// 邮件删除
    EmailDeleted { uid: u32 },
}

/// 变更检测结果
#[derive(Debug, Clone, Default)]
pub struct ChangeDetectionResult {
    /// 新邮件 UID 列表
    pub new_emails: Vec<u32>,

    /// 修改邮件 UID 列表
    pub modified_emails: Vec<u32>,

    /// 删除邮件 UID 列表
    pub deleted_emails: Vec<u32>,
}

impl ChangeDetectionResult {
    /// 是否有变更
    pub fn has_changes(&self) -> bool {
        !self.new_emails.is_empty()
            || !self.modified_emails.is_empty()
            || !self.deleted_emails.is_empty()
    }

    /// 变更总数
    pub fn total_changes(&self) -> usize {
        self.new_emails.len() + self.modified_emails.len() + self.deleted_emails.len()
    }
}

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

    /// 检测变更（统一入口）
    ///
    /// 根据 CONDSTORE 支持情况自动选择检测策略
    pub async fn detect_changes(
        &self,
        _account_id: i32,
        _folder: &str,
        _supports_condstore: bool,
    ) -> Result<ChangeDetectionResult> {
        // TODO: 实现变更检测逻辑
        // 1. 如果支持 CONDSTORE，使用 SEARCH MODSEQ
        // 2. 否则，使用 UID 对比
        Ok(ChangeDetectionResult::default())
    }

    /// 检测新邮件
    ///
    /// 使用 UID SEARCH SINCE 命令
    pub async fn detect_new_emails(
        &self,
        _account_id: i32,
        _folder: &str,
        _since_uid: u32,
    ) -> Result<Vec<u32>> {
        // TODO: 实现新邮件检测
        // 1. 执行 UID SEARCH SINCE <last_uid>
        // 2. 返回新邮件 UID 列表
        Ok(Vec::new())
    }

    /// 检测标志变更
    ///
    /// CONDSTORE: 使用 SEARCH MODSEQ
    /// 降级: 对比本地和服务器 FLAGS
    pub async fn detect_flag_changes(
        &self,
        _account_id: i32,
        _folder: &str,
        _supports_condstore: bool,
    ) -> Result<Vec<u32>> {
        // TODO: 实现标志变更检测
        // CONDSTORE: SEARCH MODSEQ <last_modseq>:*
        // 降级: FETCH 所有邮件 FLAGS，对比本地
        Ok(Vec::new())
    }

    /// 检测删除的邮件
    ///
    /// 对比本地 UID 列表和服务器 UID 列表
    pub async fn detect_deletions(
        &self,
        _account_id: i32,
        _folder: &str,
        _server_uids: &[u32],
    ) -> Result<Vec<u32>> {
        // TODO: 实现删除检测
        // 1. 获取本地 UID 列表
        // 2. 与服务器 UID 列表对比
        // 3. 返回仅在本地存在的 UID
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_detection_result_default() {
        let result = ChangeDetectionResult::default();
        assert!(result.new_emails.is_empty());
        assert!(result.modified_emails.is_empty());
        assert!(result.deleted_emails.is_empty());
        assert!(!result.has_changes());
        assert_eq!(result.total_changes(), 0);
    }

    #[test]
    fn test_change_detection_result_has_changes() {
        let mut result = ChangeDetectionResult::default();
        result.new_emails.push(100);
        assert!(result.has_changes());
        assert_eq!(result.total_changes(), 1);

        result.modified_emails.push(200);
        assert_eq!(result.total_changes(), 2);
    }

    #[test]
    fn test_change_type_equality() {
        let change1 = ChangeType::NewEmail { uid: 100 };
        let change2 = ChangeType::NewEmail { uid: 100 };
        assert_eq!(change1, change2);

        let change3 = ChangeType::NewEmail { uid: 200 };
        assert_ne!(change1, change3);
    }
}
