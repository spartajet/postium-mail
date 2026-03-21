//! 数据库迁移脚本
//!
//! SeaORM 数据库迁移定义。
//!
//! # 迁移历史
//!
//! | 版本 | 日期 | 说明 |
//! |------|------|------|
//! | m001 | 2025-03-14 | 初始化数据库（accounts, emails, folders, attachments） |
//! | m002 | 2025-03-14 | 添加 OAuth 支持字段 |
//! | m003 | 2025-03-15 | 添加同步状态表（sync_state, sync_error） |
//! | m004 | 2025-03-15 | 删除敏感字段（使用 Keyring 存储） |
//! | m005 | 2025-03-15 | 添加 IMAP 元数据（UIDVALIDITY, attributes） |
//! | m006 | 2025-03-15 | 添加同步操作表 |
//! | m007 | 2025-03-17 | 添加账号类型字段（个人/企业） |
//! | m008 | 2025-03-17 | 添加 MODSEQ 支持（CONDSTORE） |
//! | m009 | 2025-03-21 | 创建 folder_sync_states 表（分离同步状态） |
//! | m010 | 2025-03-21 | 删除 folders 表（已废弃） |
//! | m011 | 2025-03-21 | 删除 offline_operations 表（修复外键约束错误） |
//!
//! # 迁移顺序
//!
//! 迁移按版本号顺序执行，每次启动应用时自动运行未执行的迁移：
//!
//! ```text
//! 启动 → 检查迁移版本 → 执行新迁移 → 更新版本号
//! ```
//!
//! # 添加新迁移
//!
//! ## 步骤 1: 创建迁移文件
//!
//! 使用 `sea-orm-cli` 生成：
//!
//! ```bash
//! sea-orm-cli migrate generate create_my_table
//! ```
//!
//! ## 步骤 2: 定义迁移
//!
//! ```rust,ignore
//! use sea_orm_migration::prelude::*;
//!
//! pub struct Migration;
//!
//! #[async_trait::async_trait]
//! impl MigrationTrait for Migration {
//!     async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
//!         manager
//!             .create_table(
//!                 Table::create()
//!                     .table(MyTable::Table)
//!                     .col(pk_auto(MyTable::Id))
//!                     .to_owned(),
//!             )
//!             .await
//!     }
//!
//!     async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
//!         manager
//!             .drop_table(Table::drop().table(MyTable::Table).to_owned())
//!             .await
//!     }
//! }
//! ```
//!
//! ## 步骤 3: 注册迁移
//!
//! 在 `mod.rs` 中添加：
//!
//! ```rust,ignore
//! pub mod m009_YYYYMMDD_my_migration;
//! ```
//!
//! # 注意事项
//!
//! - 迁移应该是幂等的（可以重复执行）
//! - 提供向下迁移（down）以便回滚
//! - 避免在生产环境中修改现有迁移
//! - 数据破坏性操作应该分多步进行
//!
//! # 数据库索引
//!
//! ## 当前索引
//!
//! - **accounts**: email (UNIQUE)
//! - **emails**: (account_id, folder), is_read, is_starred
//! - **folder_sync_states**: (account_id, imap_name) UNIQUE, uidvalidity, synced_at
//! - **attachments**: email_id

pub mod m001_20250314_init;
pub mod m002_20250314_add_oauth_fields;
pub mod m003_20250315_add_sync_tables;
pub mod m004_20250315_remove_sensitive_fields;
pub mod m005_20250315_add_imap_metadata;
pub mod m006_20250315_add_sync_operations;
pub mod m007_20250317_add_account_types;
pub mod m008_20250317_add_modseq_support;
pub mod m009_20250321_create_folder_sync_states;
pub mod m010_20250321_drop_folders_table;
pub mod m011_20250321_drop_offline_operations;

