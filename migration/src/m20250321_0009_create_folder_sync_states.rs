//! 数据库迁移：创建 folder_sync_states 表
//!
//! 将文件夹同步状态从 folders 表分离

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查表是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='folder_sync_states'
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

            if count > 0 {
                return Ok(());
            }
        }

        // 创建 folder_sync_states 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE folder_sync_states (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    imap_name TEXT NOT NULL,
                    uidvalidity INTEGER,
                    uidnext INTEGER,
                    highest_modseq INTEGER,
                    synced_at INTEGER,
                    created_at INTEGER DEFAULT (strftime('%s', 'now')),
                    updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                    UNIQUE(account_id, imap_name)
                );

                CREATE INDEX IF NOT EXISTS idx_folder_sync_states_account
                ON folder_sync_states(account_id);

                CREATE INDEX IF NOT EXISTS idx_folder_sync_states_uidvalidity
                ON folder_sync_states(uidvalidity);

                CREATE INDEX IF NOT EXISTS idx_folder_sync_states_synced_at
                ON folder_sync_states(synced_at);
            "#,
            )
            .await?;

        // 迁移现有 folders 表数据
        let migrate_data_sql = r#"
            INSERT INTO folder_sync_states (account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at)
            SELECT account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at
            FROM folders
            WHERE imap_name IS NOT NULL;
        "#;

        if let Err(e) = manager.get_connection().execute_unprepared(migrate_data_sql).await {
            tracing::warn!("迁移 folders 数据失败（可能 folders 表不存在）: {}", e);
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_folder_sync_states_synced_at")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_folder_sync_states_uidvalidity")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_folder_sync_states_account")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS folder_sync_states")
            .await?;

        Ok(())
    }
}
