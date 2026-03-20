//! 数据模型定义
//!
//! 定义所有数据库实体模型（Entity）和关联关系。
//!
//! # 核心实体
//!
//! - [`account`]: 邮箱账号实体
//! - [`email`]: 邮件实体
//! - [`folder`]: 文件夹实体
//! - [`attachment`]: 附件实体
//! - [`sync_state`]: 同步状态实体
//! - [`sync_error`]: 同步错误实体
//!
//! # 实体关系
//!
//! ```text
//! Account (账号)
//!   ├─ 1:N ─ Email (邮件)
//!   ├─ 1:N ─ Folder (文件夹)
//!   ├─ 1:N ─ SyncState (同步状态)
//!   └─ 1:N ─ SyncError (同步错误)
//!
//! Email (邮件)
//!   ├─ N:1 ─ Account
//!   ├─ N:1 ─ Folder
//!   └─ 1:N ─ Attachment (附件)
//!
//! Folder (文件夹)
//!   ├─ N:1 ─ Account
//!   └─ 1:N ─ Email
//!
//! Attachment (附件)
//!   └─ N:1 ─ Email
//! ```
//!
//! # 数据传输对象 (DTO)
//!
//! 每个实体模块导出用于前端传输的 DTO：
//!
//! - **List DTO**: 列表项，包含显示所需的最少字段
//! - **Detail DTO**: 详情，包含完整的关联数据
//! - **Request DTO**: 请求参数，用于创建和更新操作
//!
//! # SeaORM 实体
//!
//! 每个实体由以下部分组成：
//!
//! ```rust
//! #[derive(DeriveEntityModel)]
//! pub struct Model { /* 字段定义 */ }
//!
//! #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
//! pub enum Relation { /* 关联关系 */ }
//!
//! impl ActiveModelBehavior for ActiveModel { /* 默认值 */ }
//! ```
//!
//! # 使用示例
//!
//! ## 查询邮件
//!
//! ```rust,no_run
//! use crate::storage::models::email;
//! use sea_orm::{EntityTrait, ColumnTrait};
//!
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let emails = email::Entity::find()
//!     .filter(email::Column::AccountId.eq(1))
//!     .all(db)
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 创建邮件
//!
//! ```rust,no_run
//! use crate::storage::models::email::ActiveModel;
//! use sea_orm::ActiveValue::*;
//!
//! # fn example() {
//! let email = email::ActiveModel {
//!     subject: Set("Hello".to_string()),
//!     sender_email: Set("user@example.com".to_string()),
//!     ..Default::default()
//! };
//! # }
//! ```

pub mod account;
pub mod email;
pub mod attachment;
pub mod folder;
pub mod sync_state;
pub mod sync_error;

pub use account::Entity as AccountEntity;
pub use email::Entity as EmailEntity;
pub use attachment::Entity as AttachmentEntity;
pub use folder::Entity as FolderEntity;
pub use sync_state::Entity as SyncStateEntity;
pub use sync_error::Entity as SyncErrorEntity;
