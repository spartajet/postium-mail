//! 数据库迁移：添加同步相关表结构

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 folders 表
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
                    UNIQUE(account_id, imap_name),
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
                )
            "#,
            )
            .await?;

        // 创建 folders 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
                CREATE INDEX IF NOT EXISTS idx_folders_name ON folders(name);
                CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
            "#,
            )
            .await?;

        // 创建 sync_states 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS sync_states (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    folder TEXT NOT NULL,
                    last_sync_uid INTEGER,
                    last_sync_at INTEGER,
                    highest_uid INTEGER,
                    total_emails INTEGER,
                    sync_count INTEGER DEFAULT 0,
                    is_first_sync BOOLEAN DEFAULT 1,
                    error_count INTEGER DEFAULT 0,
                    last_error TEXT,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(account_id, folder),
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
                )
            "#,
            )
            .await?;

        // 创建 sync_states 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_sync_states_account ON sync_states(account_id);
                CREATE INDEX IF NOT EXISTS idx_sync_states_folder ON sync_states(folder);
                CREATE INDEX IF NOT EXISTS idx_sync_states_last_sync ON sync_states(last_sync_at);
            "#,
            )
            .await?;

        // 创建 sync_errors 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS sync_errors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    folder TEXT,
                    error_type TEXT NOT NULL,
                    error_message TEXT NOT NULL,
                    uid INTEGER,
                    stack_trace TEXT,
                    resolved BOOLEAN DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
                )
            "#,
            )
            .await?;

        // 创建 sync_errors 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_sync_errors_account ON sync_errors(account_id);
                CREATE INDEX IF NOT EXISTS idx_sync_errors_type ON sync_errors(error_type);
                CREATE INDEX IF NOT EXISTS idx_sync_errors_resolved ON sync_errors(resolved);
                CREATE INDEX IF NOT EXISTS idx_sync_errors_created ON sync_errors(created_at);
            "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sync_errors")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sync_states")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS folders")
            .await?;

        Ok(())
    }
}
