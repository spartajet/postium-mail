//! 数据库迁移：删除旧的 folders 表

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 folders 表是否还存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='folders'
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
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE IF EXISTS folders")
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 重建 folders 表（用于回滚）
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS folders (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    imap_name TEXT NOT NULL,
                    parent_id INTEGER,
                    attributes TEXT,
                    email_count INTEGER DEFAULT 0,
                    unread_count INTEGER DEFAULT 0,
                    synced_at INTEGER,
                    uidvalidity INTEGER,
                    uidnext INTEGER,
                    highest_modseq INTEGER,
                    created_at INTEGER DEFAULT (strftime('%s', 'now')),
                    updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                    UNIQUE(account_id, imap_name)
                );

                CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
            "#,
            )
            .await?;

        // 从 folder_sync_states 迁移数据回 folders 表
        let rollback_data_sql = r#"
            INSERT INTO folders (account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at, email_count, unread_count)
            SELECT account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at, 0, 0
            FROM folder_sync_states;
        "#;

        if let Err(e) = manager.get_connection().execute_unprepared(rollback_data_sql).await {
            tracing::warn!("回滚 folders 数据失败: {}", e);
        }

        Ok(())
    }
}
