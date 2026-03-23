//! 数据库迁移：删除 offline_operations 表
//!
//! 由于 folders 表已被删除，而 offline_operations 表仍有对 folders 的外键引用

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除 offline_operations 表
        let check_offline_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='offline_operations'
        "#;

        let result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_offline_sql,
            ))
            .await?;

        if let Some(row) = result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count > 0 {
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE IF EXISTS offline_operations")
                    .await?;
            }
        }

        // 删除 email_sync_metadata 表
        let check_metadata_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='email_sync_metadata'
        "#;

        let result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_metadata_sql,
            ))
            .await?;

        if let Some(row) = result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count > 0 {
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE IF EXISTS email_sync_metadata")
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 重建 offline_operations 表（不包含对 folders 的外键引用）
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS offline_operations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    folder_name TEXT,
                    operation_type TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    synced_at INTEGER,
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_offline_operations_account ON offline_operations(account_id);
                CREATE INDEX IF NOT EXISTS idx_offline_operations_synced ON offline_operations(synced_at);
                CREATE INDEX IF NOT EXISTS idx_offline_operations_type ON offline_operations(operation_type);
            "#,
            )
            .await?;

        Ok(())
    }
}
