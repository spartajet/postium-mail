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
//! | `imap::client` | `protocols::imap::client` | ⏳ 待迁移 |
//! | `sync_manager` | `sync::sync_manager` | ⏳ 待迁移 |
//! | `account_service` | `auth::auth_manager` | ⏳ 待迁移 |
//! | `email_service` | `sync::mail_processor` | ⏳ 待迁移 |
//! | `folder_service` | `sync::folder_manager` | ⏳ 待迁移 |
//!
//! ## 使用指南
//!
//! ### ✅ 可以继续使用（兼容层）
//!
//! 这些模块在 `command` 层仍被使用，暂时保留：
//! - `account_service`, `email_service`, `folder_service` 等
//! - `oauth_service` - 标记为 deprecated，功能正常
//!
//! ### 🔄 正在迁移
//!
//! - `services::imap` → `protocols::imap`
//! - 优先级：阶段2
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
pub mod imap;
pub mod smtp_service;
pub mod search_service;

// ⚠️ DEPRECATED: oauth_service 保留用于向后兼容
// 迁移目标: auth::oauth_handler + providers::oauth_utils
// 计划移除: 阶段3 - command 层迁移后
pub mod oauth_service;

pub mod folder_service;
pub mod sync_state_service;
pub mod sync_error_service;
pub mod sync_manager;

// 重新导出常用类型（保留必要的导出）
pub use account_service::*;
pub use email_service::*;
pub use imap::*;
pub use smtp_service::*;
pub use search_service::*;
pub use folder_service::*;
pub use sync_state_service::*;
pub use sync_error_service::*;
pub use sync_manager::*;
