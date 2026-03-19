//! 旧服务层模块
//!
//! # 🚧 迁移状态
//!
//! 此目录包含旧的服务层实现，正在逐步迁移到新架构。
//!
//! ## 模块迁移对照表
//!
//! | 旧模块 | 新模块 | 状态 |
//! |--------|--------|------|
//! | `oauth_service` | `auth::oauth_handler` + `providers::oauth_utils` | ⚠️ 保留兼容 |
//! | `imap/*` | `protocols::imap/*` | ✅ 已迁移 |
//! | `smtp_service` | `protocols::smtp/*` | ✅ 已迁移 |
//! | `sync_manager` | `sync::sync_manager` | ✅ 已迁移 |
//! | `account_service` | `auth::auth_manager` | ⏳ 待迁移 |
//! | `email_service` | `sync::mail_processor` | ⏳ 待迁移 |
//! | `folder_service` | `sync::folder_manager` | ⏳ 待迁移 |
//!
//! ## 迁移完成
//!
//! ✅ **IMAP 客户端** - 已完全迁移到 `protocols::imap`
//! - 所有 IMAP 相关功能现在位于 `crate::protocols::imap`
//! - 使用示例：
//!   ```rust,no_run
//!   use crate::protocols::imap::{AsyncImapClient, ImapAuth};
//!   ```
//!
//! ✅ **SMTP 客户端** - 已完全迁移到 `protocols::smtp`
//! - 所有 SMTP 相关功能现在位于 `crate::protocols::smtp`
//! - 使用示例：
//!   ```rust,no_run
//!   use crate::protocols::smtp::{SmtpClient, SmtpAuth, SendEmailRequest};
//!   ```
//!
//! ✅ **同步管理器** - 已完全迁移到 `sync::sync_manager`
//! - 所有同步管理功能现在位于 `crate::sync::SyncManager`
//! - 新实现集成了 AuthManager 和 ProviderPool，支持服务商自动检测和统一认证
//! - 使用示例：
//!   ```rust,no_run
//!   use crate::sync::SyncManager;
//!   let sync_manager = SyncManager::new(db, app_handle, auth_manager, provider_pool);
//!   ```
//!
//! ## 使用指南
//!
//! ### ✅ 可以继续使用（兼容层）
//!
//! 这些模块在 `command` 层仍被使用，暂时保留：
//! - `account_service`, `email_service`, `folder_service` 等
//! - `oauth_service` - 标记为 deprecated，功能正常
//!
//! ### ⏳ 待迁移
//!
//! 其他服务模块将在后续阶段逐步迁移到新架构。
//!
//! ## 迁移原则
//!
//! 1. **向后兼容**: 每次迁移确保不破坏现有功能
//! 2. **渐进式**: 一次迁移一个模块，保持代码可控
//! 3. **充分测试**: 每个阶段完成后进行完整测试
//! 4. **文档更新**: 及时更新文档和注释

pub mod account_service;
pub mod email_service;
pub mod operations;  // 操作管理模块
pub mod search_service;

// ⚠️ DEPRECATED: oauth_service 保留用于向后兼容
// 迁移目标: auth::oauth_handler + providers::oauth_utils
// 计划移除: 阶段3 - command 层迁移后
pub mod oauth_service;

pub mod folder_service;
pub mod sync_state_service;
pub mod sync_error_service;

// 重新导出常用类型（保留必要的导出）
pub use account_service::*;
pub use email_service::*;
pub use search_service::*;
pub use folder_service::*;
pub use sync_state_service::*;
pub use sync_error_service::*;
