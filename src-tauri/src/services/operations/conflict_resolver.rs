//! 冲突解决器
//!
//! 处理离线操作与服务器状态之间的冲突

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::error::Result;
use crate::services::operations::{OfflineOperation, OperationType};

/// 冲突类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictType {
    /// 邮件已不存在
    EmailNotFound {
        email_id: i64,
        folder: String,
    },
    /// 文件夹已不存在
    FolderNotFound {
        folder: String,
    },
    /// 邮件已被移动
    EmailAlreadyMoved {
        email_id: i64,
        current_folder: String,
    },
    /// 操作冲突（如重复删除）
    OperationConflict {
        reason: String,
    },
}

/// 冲突解决策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution {
    /// 跳过操作
    Skip,
    /// 重试操作
    Retry,
    /// 强制执行
    Force,
    /// 取消操作
    Cancel,
    /// 用户决定（需要UI交互）
    UserDecision,
}

/// 冲突信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    /// 冲突类型
    pub conflict_type: ConflictType,
    /// 相关操作
    pub operation: OfflineOperation,
    /// 建议的解决策略
    pub resolution: ConflictResolution,
}

/// 冲突解决器
pub struct ConflictResolver;

impl ConflictResolver {
    /// 创建新的冲突解决器
    pub fn new() -> Self {
        Self
    }

    /// 检测操作冲突
    pub fn detect_conflict(
        &self,
        operation: &OfflineOperation,
        server_state: &ServerState,
    ) -> Option<Conflict> {
        match &operation.op_type {
            OperationType::MoveEmail {
                email_id,
                from_folder,
                to_folder,
            } => {
                // 检查邮件是否存在
                if !server_state.emails.contains(email_id) {
                    return Some(Conflict {
                        conflict_type: ConflictType::EmailNotFound {
                            email_id: *email_id,
                            folder: from_folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Skip,
                    });
                }

                // 检查目标文件夹是否存在
                if !server_state.folders.contains(to_folder) {
                    return Some(Conflict {
                        conflict_type: ConflictType::FolderNotFound {
                            folder: to_folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Skip,
                    });
                }

                None
            }

            OperationType::DeleteEmail { email_id, folder } => {
                if !server_state.emails.contains(email_id) {
                    return Some(Conflict {
                        conflict_type: ConflictType::EmailNotFound {
                            email_id: *email_id,
                            folder: folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Skip,
                    });
                }
                None
            }

            OperationType::MarkAsRead { email_id, folder, read: _ } => {
                if !server_state.emails.contains(email_id) {
                    return Some(Conflict {
                        conflict_type: ConflictType::EmailNotFound {
                            email_id: *email_id,
                            folder: folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Skip,
                    });
                }
                None
            }

            OperationType::SetFlag { email_id, folder, .. } => {
                if !server_state.emails.contains(email_id) {
                    return Some(Conflict {
                        conflict_type: ConflictType::EmailNotFound {
                            email_id: *email_id,
                            folder: folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Skip,
                    });
                }
                None
            }

            OperationType::CreateDraft { folder, data: _ } => {
                if !server_state.folders.contains(folder) {
                    return Some(Conflict {
                        conflict_type: ConflictType::FolderNotFound {
                            folder: folder.clone(),
                        },
                        operation: operation.clone(),
                        resolution: ConflictResolution::Cancel,
                    });
                }
                None
            }

            OperationType::SendEmail { .. } => None,
        }
    }

    /// 解决冲突
    pub fn resolve_conflict(&self, conflict: &Conflict) -> Result<ConflictResolution> {
        // 默认策略：使用建议的解决策略
        Ok(conflict.resolution.clone())
    }

    /// 批量检测冲突
    pub fn detect_conflicts(
        &self,
        operations: &[OfflineOperation],
        server_state: &ServerState,
    ) -> Vec<Conflict> {
        operations
            .iter()
            .filter_map(|op| self.detect_conflict(op, server_state))
            .collect()
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务器状态快照
#[derive(Debug, Clone)]
pub struct ServerState {
    /// 现有邮件 ID 集合
    pub emails: HashSet<i64>,
    /// 现有文件夹集合
    pub folders: HashSet<String>,
}

impl ServerState {
    /// 创建新的服务器状态
    pub fn new() -> Self {
        Self {
            emails: HashSet::new(),
            folders: HashSet::new(),
        }
    }

    /// 添加邮件
    pub fn add_email(&mut self, email_id: i64) {
        self.emails.insert(email_id);
    }

    /// 添加文件夹
    pub fn add_folder(&mut self, folder: String) {
        self.folders.insert(folder);
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::operations::OperationStatus;

    fn create_test_operation(id: i64, op_type: OperationType) -> OfflineOperation {
        OfflineOperation {
            id,
            op_type,
            status: OperationStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            retry_count: 0,
        }
    }

    #[test]
    fn test_detect_email_not_found() {
        let resolver = ConflictResolver::new();
        let mut server_state = ServerState::new();
        server_state.add_folder("INBOX".to_string());

        let operation = create_test_operation(
            1,
            OperationType::DeleteEmail {
                email_id: 999,
                folder: "INBOX".to_string(),
            },
        );

        let conflict = resolver.detect_conflict(&operation, &server_state);
        assert!(conflict.is_some());
        assert_eq!(
            conflict.unwrap().resolution,
            ConflictResolution::Skip
        );
    }

    #[test]
    fn test_detect_folder_not_found() {
        let resolver = ConflictResolver::new();
        let mut server_state = ServerState::new();
        server_state.add_email(1);

        let operation = create_test_operation(
            1,
            OperationType::MoveEmail {
                email_id: 1,
                from_folder: "INBOX".to_string(),
                to_folder: "Archive".to_string(),
            },
        );

        let conflict = resolver.detect_conflict(&operation, &server_state);
        assert!(conflict.is_some());
    }

    #[test]
    fn test_no_conflict() {
        let resolver = ConflictResolver::new();
        let mut server_state = ServerState::new();
        server_state.add_email(1);
        server_state.add_folder("INBOX".to_string());
        server_state.add_folder("Archive".to_string());

        let operation = create_test_operation(
            1,
            OperationType::MoveEmail {
                email_id: 1,
                from_folder: "INBOX".to_string(),
                to_folder: "Archive".to_string(),
            },
        );

        let conflict = resolver.detect_conflict(&operation, &server_state);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_batch_conflict_detection() {
        let resolver = ConflictResolver::new();
        let mut server_state = ServerState::new();
        server_state.add_email(1);
        server_state.add_folder("INBOX".to_string());

        let operations = vec![
            create_test_operation(
                1,
                OperationType::DeleteEmail {
                    email_id: 1,
                    folder: "INBOX".to_string(),
                },
            ),
            create_test_operation(
                2,
                OperationType::DeleteEmail {
                    email_id: 999,
                    folder: "INBOX".to_string(),
                },
            ),
        ];

        let conflicts = resolver.detect_conflicts(&operations, &server_state);
        assert_eq!(conflicts.len(), 1); // 第二个操作有冲突
    }
}
