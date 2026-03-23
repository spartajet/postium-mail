//! 数据库迁移：合并 sync_states 表到 folder_sync_states
//!
//! 将 `sync_states` 表的同步进度字段合并到 `folder_sync_states` 表

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 sync_count 列是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('folder_sync_states')
            WHERE name='sync_count'
        "#;

        let result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_sql,
            ))
            .await?;

        if let Some(row) = result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count == 0 {
                // 添加 sync_states 的字段到 folder_sync_states
                let migrations = vec![
                    "ALTER TABLE folder_sync_states ADD COLUMN last_sync_uid INTEGER",
                    "ALTER TABLE folder_sync_states ADD COLUMN highest_uid INTEGER",
                    "ALTER TABLE folder_sync_states ADD COLUMN total_emails INTEGER",
                    "ALTER TABLE folder_sync_states ADD COLUMN sync_count INTEGER DEFAULT 0",
                    "ALTER TABLE folder_sync_states ADD COLUMN is_first_sync BOOLEAN DEFAULT TRUE",
                    "ALTER TABLE folder_sync_states ADD COLUMN error_count INTEGER DEFAULT 0",
                    "ALTER TABLE folder_sync_states ADD COLUMN last_error TEXT",
                ];

                for sql in migrations {
                    manager.get_connection().execute_unprepared(sql).await?;
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留合并的字段");
        Ok(())
    }
}
