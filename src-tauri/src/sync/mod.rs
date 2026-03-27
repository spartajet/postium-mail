//! 邮件同步模块
//!
//! 提供完整的邮件同步解决方案，支持增量同步、多账号并发同步。
//!
//! # 核心功能
//!
//! - **增量同步**: 基于 UID 的高效同步
//! - **变更检测**: 检测邮件标志、内容变更
//! - **多账号支持**: 并发同步多个账号
//! - **进度跟踪**: 实时同步进度反馈
//! - **错误处理**: 结构化的错误处理和重试机制
//! - **任务调度**: 统一的同步任务调度和管理
//!
//! # 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────┐
//! │           SyncManager                  │
//! │  (同步调度器、流程编排)                   │
//! └────────────┬────────────────────────────┘
//!               │
//!     ┌─────────┼─────────┬──────────┐
//!     │         │         │          │
//! ┌───▼───┐ ┌───▼────┐ ┌─▼──────┐ ┌─▼──────┐
//! │Strategy│ │Change  │ │Folder  │ │Mail    │
//! │Selector│ │Detector│ │Manager │ │Processor│
//! └───────┘ └────────┘ └────────┘ └─────────┘
//!     ▲
//!     │
//! ┌───▼───┐ ┌───▼────┐
//! │Full   │ │Incre-  │
//! │Sync   │ │mental  │
//! │Engine │ │ Sync   │
//! └───────┘ └────────┘
//!       │
//! ┌──────▼───────┐
//! │   Scheduler  │
//! │  (任务调度)   │
//! └──────────────┘
//!       │
//! ┌──────▼───────┐
//! │    State     │
//! │  (状态管理)   │
//! └──────────────┘
//! ```
//!
//! # 子模块说明
//!
//! ## [`SyncManager`] - 同步管理器
//!
//! 核心同步调度器，负责：
//! - 编排同步流程
//! - 管理同步状态
//! - 处理同步错误
//! - 进度事件发布
//!
//! ## [`strategy`] - 同步策略模块
//!
//! 包含全量和增量同步引擎：
//! - [`FullSyncEngine`][]: 完整同步所有邮件
//! - [`IncrementalSyncEngine`][]: 基于变更检测的增量同步
//! - [`SyncPreparation`][]: 统一的同步准备结果
//!
//! ## [`change`] - 变化检测模块
//!
//! 检测邮件变更：
//! - [`ChangeDetector`][]: 核心检测逻辑
//! - [`EmailFlags`][]: 邮件标志状态
//! - [`UidSet`][]: UID 集合辅助类型
//! - [`ChangeDetectionResult`][]: 变更检测结果
//!
//! ## [`state`] - 状态管理模块
//!
//! 统一的同步状态管理：
//! - [`SyncState`][]: 统一的同步状态管理器
//! - [`ProgressTracker`][]: 同步进度追踪器
//! - [`FolderSyncState`][]: 文件夹同步状态
//!
//! ## [`scheduler`] - 任务调度模块
//!
//! 同步任务调度：
//! - [`SyncScheduler`][]: 任务调度器
//! - [`SyncTask`][]: 同步任务定义
//!
//! # 同步流程
//!
//! ## 完整同步流程
//!
//! ```text
//! 1. 连接服务器
//!    ↓
//! 2. 获取文件夹列表
//!    ↓
//! 3. 检测同步策略（全量/增量）
//!    ↓
//! 4. 准备同步（获取 UID 列表）
//!    ↓
//! 5. 变更检测
//!    ↓
//! 6. 获取新/变更邮件
//!    ↓
//! 7. 保存到数据库
//!    ↓
//! 8. 更新同步状态
//! ```
//!
//! # 性能优化
//!
//! - **并发同步**: 多账号可并发同步
//! - **批量获取**: 批量获取邮件减少网络往返
//! - **增量同步**: 仅同步变更数据
//! - **连接复用**: 同一账号的多次同步复用连接
//!
//! # 注意事项
//!
//! - 同步操作可能耗时较长，建议在后台线程执行
//! - 大账号首次同步可能需要较长时间
//! - 网络不稳定时会自动重试
//! - 某些操作（如删除）不可逆，请谨慎处理

// ========== 子模块声明 ==========

// 核心模块
pub mod error;
pub mod mail_processor;
pub mod sync_manager;

// 新增子模块
pub mod change;
mod folder;
pub mod scheduler;
pub mod state;
pub mod strategy;
mod strcuts;

// ========== 重新导出主要类型 ==========

// 核心类型
pub use sync_manager::SyncManager;

pub use strcuts::{SyncProgress, SyncResult, SyncStage};

// 变化检测模块
pub use change::{
    ChangeDetectionResult, ChangeDetector, ChangeType, EmailFlags, UidSet, imap_flags,
};

// 状态管理模块
pub use state::{FolderSyncState, ProgressTracker, SyncState, SyncStatus as SyncStateStatus};

// 调度模块
pub use scheduler::{SyncScheduler, SyncTask, SyncTaskPriority, SyncTaskStatus};

// 其他模块
pub use error::SyncErrorManager;
pub use mail_processor::{MailData, MailProcessResult, MailProcessor};
