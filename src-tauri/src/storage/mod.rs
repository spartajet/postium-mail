//! 存储层模块
//!
//! 提供数据库访问、缓存管理和数据持久化功能。
//!
//! # 核心功能
//!
//! - **数据库管理**: SeaORM 连接池、迁移管理
//! - **数据访问**: 账号、邮件、文件夹的 CRUD 操作
//! - **缓存系统**: 内存缓存、计数器缓存、内容缓存
//! - **全文搜索**: 邮件内容搜索
//! - **数据模型**: 实体定义和关系映射
//!
//! # 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────┐
//! │              应用层                       │
//! └────────────────┬────────────────────────┘
//!                  │
//! ┌─────────────────▼───────────────────────┐
//! │          Repository 层                   │
//! │  ┌───────────────────────────────────┐  │
//! │  │ AccountRepository │ EmailRepository │  │
//!  │  │ FolderRepository  │  SearchService  │  │
//!  │  └───────────────────────────────────┘  │
//! └─────────────────┬───────────────────────┘
//!                  │
//! ┌─────────────────▼───────────────────────┐
//! │              Database                     │
//! │         (SeaORM + SQLite)               │
//! └───────────────────────────────────────┘
//!
//! ┌────────────────────────────────────────┐
//! │           Cache 层                       │
//! │  ┌──────────────────────────────────┐ │
//!  │  │ EmailContentCache │ CounterCache   │ │
//!  │  └──────────────────────────────────┘ │
//! └───────────────────────────────────────┘
//! ```
//!
//! # 模块说明
//!
//! ## [`database`] - 数据库连接
//!
//! 管理 SeaORM 连接池和数据库生命周期。
//!
//! ## [`accounts`] - 账号数据访问
//!
//! 提供账号的 CRUD 操作：
//! - 创建账号
//! - 更新账号
//! - 删除账号
//! - 查询账号
//! - OAuth 令牌管理
//!
//! ## [`emails`] - 邮件数据访问
//!
//! 提供邮件的 CRUD 操作：
//! - 保存邮件
//! - 查询邮件列表
//! - 获取邮件详情
//! - 更新邮件标志
//! - 删除邮件
//!
//! ## [`folders`] - 文件夹数据访问
//!
//! 提供文件夹的管理操作：
//! - 创建文件夹
//! - 更新文件夹
//! - 删除文件夹
//! - 查询文件夹列表
//!
//! ## [`cache`] - 缓存管理
//!
//! 提供多级缓存策略：
//! - [`EmailContentCache`][]: 邮件内容缓存
//! - [`CounterCache`][]: 计数缓存（未读数等）
//! - [`FolderListCache`][]: 文件夹列表缓存
//! - [`MemoryCache`][]: 通用内存缓存
//!
//! ## [`search`] - 全文搜索
//!
//! 基于 FTS5 或数据库 LIKE 的邮件搜索。
//!
//! ## [`models`] - 数据模型
//!
//! 定义所有实体模型和关系：
//! - [`account`][]: 账号实体
//! - [`email`][]: 邮件实体
//! - [`folder`][]: 文件夹实体
//! - [`attachment`][]: 附件实体
//! - [`sync_state`][]: 同步状态实体
//! - [`sync_error`][]: 同步错误实体
//!
//! ## [`migration`] - 数据库迁移
//!
//! SeaORM 数据库迁移脚本。
//!
//! # 数据模型关系
//!
//! ```
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
//! ```
//!
//! # 使用示例
//!
//! ## 数据库初始化
//!
//! ```rust,no_run
//! use crate::storage::DatabaseConnection;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let db = DatabaseConnection::new("data.db").await?;
//!
//! // 运行迁移
//! db.run_migrations().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 创建账号
//!
//! ```rust,no_run
//! # use crate::storage::AccountRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! use crate::storage::CreateAccountRequest;
//!
//! let request = CreateAccountRequest {
//!     name: "我的邮箱".to_string(),
//!     email: "user@example.com".to_string(),
//!     provider: "gmail".to_string(),
//!     // ...
//! };
//!
//! let account_id = AccountRepository::create_account(&db, request).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 查询邮件列表
//!
//! ```rust,no_run
//! # use crate::storage::EmailRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let response = EmailRepository::list_emails(
//!     &db,
//!     account_id,
//!     folder,
//!     limit,
//!     offset
//! ).await?;
//!
//! println!("总数: {}, 邮件: {}", response.total, response.emails.len());
//! # Ok(())
//! # }
//! ```
//!
//! # 缓存策略
//!
//! ## 缓存层级
//!
//! 1. **内存缓存**: 最快访问，易丢失
//! 2. **数据库缓存**: 持久化，较慢
//! 3. **远程获取**: 最慢，实时数据
//!
//! ## 缓存失效
//!
//! - 邮件变更时：清除相关缓存
//! - 新邮件到达：更新计数器缓存
//! - 用户操作：立即使缓存失效
//!
//! # 注意事项
//!
//! - 所有数据库操作都应通过 Repository 层进行
//! - 缓存数据可能不是实时的
//! - 大量数据操作应使用批量接口
//! - 数据库连接应正确关闭

pub mod cache;
pub mod database;
pub mod models;
pub mod search;
pub mod service;

// 重新导出常用类型
pub use cache::{CacheManager, CounterCache, EmailContentCache, FolderListCache, MemoryCache};
pub use database::{DatabaseConnection, Repository, establish_connection, init_database};
pub use models::{account, attachment, email, sync_error, sync_state};
pub use search::{SearchResult, SearchService};
pub use service::{
    AccountDto, AccountRepository, AttachmentInfo, CreateAccountRequest, EmailAddress, EmailDetail,
    EmailListItem, EmailListResponse, EmailSearchParams, SendEmailRequest, UpdateAccountRequest,
};
