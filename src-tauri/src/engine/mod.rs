//! 流程引擎模块
//!
//! 提供任务调度、通知管理和操作管理功能

pub mod conflict_resolver;
pub mod flow_engine;
pub mod operation_manager;
mod notification_manager;
mod task_scheduler;

// 重新导出主要类型
pub use conflict_resolver::{
    Conflict, ConflictResolver, ConflictResolution, ConflictType, ServerState,
};
pub use flow_engine::{
    FlowEngine, FlowEngineConfig, EngineState, EngineStatusReport,
};
pub use operation_manager::{OfflineOperation, OperationManager, OperationStatus, OperationType};
