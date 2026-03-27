//! 数据库迁移模块
//!
//! 使用 SeaORM 官方迁移系统管理数据库版本。

pub use sea_orm_migration::prelude::*;

// 导出额外的 sea_orm 类型
pub use sea_orm::{ConnectionTrait, DbBackend, Statement};

mod m20250314_0001_init;
mod m20250314_0002_add_oauth_fields;
mod m20250315_0003_add_sync_tables;
mod m20250315_0004_remove_sensitive_fields;
mod m20250315_0005_add_imap_metadata;
mod m20250315_0006_add_sync_operations;
mod m20250317_0007_add_account_types;
mod m20250317_0008_add_modseq_support;
mod m20250321_0009_create_folder_sync_states;
mod m20250321_0010_drop_folders_table;
mod m20250321_0011_drop_offline_operations;
mod m20250322_0012_add_email_flags;
mod m20250323_0013_add_folder_type;
mod m20250323_0014_merge_sync_states;
mod m20250323_0015_drop_sync_states_table;
mod m20250324_0016_remove_modseq_support;
mod m20250327_0017_rename_to_sync_state;

/// 数据库迁移器
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250314_0001_init::Migration),
            Box::new(m20250314_0002_add_oauth_fields::Migration),
            Box::new(m20250315_0003_add_sync_tables::Migration),
            Box::new(m20250315_0004_remove_sensitive_fields::Migration),
            Box::new(m20250315_0005_add_imap_metadata::Migration),
            Box::new(m20250315_0006_add_sync_operations::Migration),
            Box::new(m20250317_0007_add_account_types::Migration),
            Box::new(m20250317_0008_add_modseq_support::Migration),
            Box::new(m20250321_0009_create_folder_sync_states::Migration),
            Box::new(m20250321_0010_drop_folders_table::Migration),
            Box::new(m20250321_0011_drop_offline_operations::Migration),
            Box::new(m20250322_0012_add_email_flags::Migration),
            Box::new(m20250323_0013_add_folder_type::Migration),
            Box::new(m20250323_0014_merge_sync_states::Migration),
            Box::new(m20250323_0015_drop_sync_states_table::Migration),
            Box::new(m20250324_0016_remove_modseq_support::Migration),
            Box::new(m20250327_0017_rename_to_sync_state::Migration),
        ]
    }
}
