//! 操作管理器
//!
//! 用于管理和跟踪离线操作（如移动、删除邮件等）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, StorageError};

/// 操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationType {
    /// 移动邮件
    MoveEmail {
        email_id: i64,
        from_folder: String,
        to_folder: String,
    },
    /// 删除邮件
    DeleteEmail {
        email_id: i64,
        folder: String,
    },
    /// 标记已读/未读
    MarkAsRead {
        email_id: i64,
        folder: String,
        read: bool,
    },
    /// 设置星标
    SetFlag {
        email_id: i64,
        folder: String,
        flag: String,
        value: bool,
    },
    /// 创建草稿
    CreateDraft {
        folder: String,
        data: String,
    },
    /// 发送邮件
    SendEmail {
        data: String,
    },
}

/// 操作状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationStatus {
    /// 待执行
    Pending,
    /// 执行中
    InProgress,
    /// 成功
    Success,
    /// 失败
    Failed { error: String },
    /// 已取消
    Cancelled,
}

/// 离线操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineOperation {
    /// 操作 ID
    pub id: i64,
    /// 操作类型
    pub op_type: OperationType,
    /// 操作状态
    pub status: OperationStatus,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 更新时间
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// 重试次数
    pub retry_count: u32,
}

/// 操作管理器
pub struct OperationManager {
    operations: Arc<RwLock<HashMap<i64, OfflineOperation>>>,
    next_id: Arc<RwLock<i64>>,
}

impl OperationManager {
    /// 创建新的操作管理器
    pub fn new() -> Self {
        Self {
            operations: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// 添加操作
    pub async fn add_operation(&self, op_type: OperationType) -> Result<i64> {
        let mut next_id = self.next_id.write().await;
        let id = *next_id;
        *next_id += 1;

        let now = chrono::Utc::now();
        let operation = OfflineOperation {
            id,
            op_type,
            status: OperationStatus::Pending,
            created_at: now,
            updated_at: now,
            retry_count: 0,
        };

        let mut operations = self.operations.write().await;
        operations.insert(id, operation);

        Ok(id)
    }

    /// 获取操作
    pub async fn get_operation(&self, id: i64) -> Option<OfflineOperation> {
        let operations = self.operations.read().await;
        operations.get(&id).cloned()
    }

    /// 获取所有待执行的操作
    pub async fn get_pending_operations(&self) -> Vec<OfflineOperation> {
        let operations = self.operations.read().await;
        operations
            .values()
            .filter(|op| op.status == OperationStatus::Pending)
            .cloned()
            .collect()
    }

    /// 获取账号相关的操作
    pub async fn get_operations_for_account(&self, account_id: i32) -> Vec<OfflineOperation> {
        // 暂时返回所有操作，后续可以根据 account_id 过滤
        let operations = self.operations.read().await;
        operations.values().cloned().collect()
    }

    /// 更新操作状态
    pub async fn update_status(&self, id: i64, status: OperationStatus) -> Result<()> {
        let mut operations = self.operations.write().await;
        if let Some(op) = operations.get_mut(&id) {
            op.status = status;
            op.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(StorageError::NotFound(format!("操作 {} 不存在", id)).into())
        }
    }

    /// 删除操作
    pub async fn remove_operation(&self, id: i64) -> Result<()> {
        let mut operations = self.operations.write().await;
        operations
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| StorageError::NotFound(format!("操作 {} 不存在", id)).into())
    }

    /// 清理已完成的操作
    pub async fn cleanup_completed(&self) -> usize {
        let mut operations = self.operations.write().await;
        let initial_count = operations.len();

        operations.retain(|_, op| {
            !matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Cancelled
            )
        });

        initial_count - operations.len()
    }

    /// 获取操作数量
    pub async fn count(&self) -> usize {
        let operations = self.operations.read().await;
        operations.len()
    }

    /// 增加重试次数
    pub async fn increment_retry(&self, id: i64) -> Result<()> {
        let mut operations = self.operations.write().await;
        if let Some(op) = operations.get_mut(&id) {
            op.retry_count += 1;
            Ok(())
        } else {
            Err(StorageError::NotFound(format!("操作 {} 不存在", id)).into())
        }
    }
}

impl Default for OperationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_operation() {
        let manager = OperationManager::new();

        let op_type = OperationType::DeleteEmail {
            email_id: 1,
            folder: "INBOX".to_string(),
        };

        let id = manager.add_operation(op_type).await.unwrap();
        assert!(id > 0);

        let op = manager.get_operation(id).await;
        assert!(op.is_some());
        assert_eq!(op.unwrap().status, OperationStatus::Pending);
    }

    #[tokio::test]
    async fn test_update_status() {
        let manager = OperationManager::new();

        let op_type = OperationType::MarkAsRead {
            email_id: 1,
            folder: "INBOX".to_string(),
            read: true,
        };

        let id = manager.add_operation(op_type).await.unwrap();

        manager
            .update_status(id, OperationStatus::Success)
            .await
            .unwrap();

        let op = manager.get_operation(id).await.unwrap();
        assert_eq!(op.status, OperationStatus::Success);
    }

    #[tokio::test]
    async fn test_remove_operation() {
        let manager = OperationManager::new();

        let op_type = OperationType::SetFlag {
            email_id: 1,
            folder: "INBOX".to_string(),
            flag: "\\Flagged".to_string(),
            value: true,
        };

        let id = manager.add_operation(op_type).await.unwrap();
        manager.remove_operation(id).await.unwrap();

        let op = manager.get_operation(id).await;
        assert!(op.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_completed() {
        let manager = OperationManager::new();

        // 添加一个成功的操作
        let op_type = OperationType::DeleteEmail {
            email_id: 1,
            folder: "INBOX".to_string(),
        };
        let id = manager.add_operation(op_type).await.unwrap();
        manager
            .update_status(id, OperationStatus::Success)
            .await
            .unwrap();

        // 清理
        let cleaned = manager.cleanup_completed().await;
        assert_eq!(cleaned, 1);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_get_pending_operations() {
        let manager = OperationManager::new();

        // 添加两个操作
        let op1 = manager
            .add_operation(OperationType::DeleteEmail {
                email_id: 1,
                folder: "INBOX".to_string(),
            })
            .await
            .unwrap();

        let op2 = manager
            .add_operation(OperationType::MarkAsRead {
                email_id: 2,
                folder: "INBOX".to_string(),
                read: true,
            })
            .await
            .unwrap();

        // 标记一个为成功
        manager
            .update_status(op2, OperationStatus::Success)
            .await
            .unwrap();

        // 获取待执行操作
        let pending = manager.get_pending_operations().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, op1);
    }
}
