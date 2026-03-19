//! 旧服务层模块
//!
//! # 🎉 迁移完成
//!
//! 所有服务层已完成迁移到新架构：
//! - `storage::accounts` - 账号存储层
//! - `storage::emails` - 邮件存储层
//! - `storage::folders` - 文件夹存储层
//! - `protocols::imap` - IMAP 客户端
//! - `protocols::smtp` - SMTP 客户端
//! - `sync::SyncManager` - 同步管理器
//! - `sync::SyncStateManager` - 同步状态管理器（已迁移）
//! - `sync::SyncErrorManager` - 同步错误管理器（已迁移）
//! - `auth::AuthManager` - 认证管理器（已整合 OAuth）
//!
//! ## 保留的模块
//!
//! - **`operations`** - 操作管理模块
//! - **`search_service`** - 邮件全文搜索（待迁移）

pub mod operations;  // 操作管理模块
pub mod search_service;

// 重新导出常用类型
pub use search_service::*;
