//! 操作管理模块
//!
//! 用于管理和解决离线操作的冲突

mod conflict_resolver;
mod operation_manager;

// 重新导出公共接口
pub use conflict_resolver::{
    Conflict, ConflictResolver, ConflictResolution, ConflictType, ServerState,
};
pub use operation_manager::{OfflineOperation, OperationManager, OperationStatus, OperationType};
