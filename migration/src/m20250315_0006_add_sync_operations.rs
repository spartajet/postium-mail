//! 数据库迁移：添加离线操作队列和邮件同步元数据表

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 offline_operations 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS offline_operations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    folder_id INTEGER,
                    operation_type TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    synced_at INTEGER,
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
                )
            "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_offline_operations_account ON offline_operations(account_id);
                CREATE INDEX IF NOT EXISTS idx_offline_operations_synced ON offline_operations(synced_at);
                CREATE INDEX IF NOT EXISTS idx_offline_operations_type ON offline_operations(operation_type);
            "#,
            )
            .await?;

        // 创建 email_sync_metadata 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS email_sync_metadata (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    email_id INTEGER NOT NULL UNIQUE,
                    modseq INTEGER,
                    body_fetched INTEGER DEFAULT 0,
                    last_synced_at INTEGER,
                    FOREIGN KEY (email_id) REFERENCES emails(id) ON DELETE CASCADE
                )
            "#,
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_email_sync_metadata_email ON email_sync_metadata(email_id);
                CREATE INDEX IF NOT EXISTS idx_email_sync_metadata_body_fetched ON email_sync_metadata(body_fetched);
            "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS email_sync_metadata")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS offline_operations")
            .await?;

        Ok(())
    }
}
