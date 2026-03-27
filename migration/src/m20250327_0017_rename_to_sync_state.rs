//! 数据库迁移：重构 folder_sync_states 为 sync_state
//!
//! 简化表结构，移除冗余的同步进度字段，这些功能由内存中的 ProgressTracker 管理。
//! - 表名：folder_sync_states → sync_state
//! - 字段重命名：imap_name → folder
//! - 新增字段：folder_nick_name（存储 IMAP UTF-7 编码的原始文件夹名称）
//! - 删除字段：folder_type, highest_uid, total_emails, sync_count, is_first_sync, error_count, last_error

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. 创建新的 sync_state 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE sync_state (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    folder TEXT NOT NULL,
                    folder_nick_name TEXT,
                    uidvalidity INTEGER,
                    uidnext INTEGER,
                    synced_at INTEGER,
                    last_sync_uid INTEGER,
                    created_at INTEGER DEFAULT (strftime('%s', 'now')),
                    updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
                    UNIQUE(account_id, folder)
                );

                CREATE INDEX IF NOT EXISTS idx_sync_state_account
                ON sync_state(account_id);

                CREATE INDEX IF NOT EXISTS idx_sync_state_folder
                ON sync_state(folder);
            "#,
            )
            .await?;

        // 2. 迁移现有数据
        let migrate_data_sql = r#"
            INSERT INTO sync_state (
                account_id, folder, folder_nick_name, uidvalidity, uidnext,
                synced_at, last_sync_uid, created_at, updated_at
            )
            SELECT
                account_id,
                imap_name as folder,
                NULL as folder_nick_name,
                uidvalidity,
                uidnext,
                synced_at,
                last_sync_uid,
                created_at,
                updated_at
            FROM folder_sync_states;
        "#;

        if let Err(e) = manager.get_connection().execute_unprepared(migrate_data_sql).await {
            tracing::warn!("迁移 folder_sync_states 数据失败（可能表不存在）: {}", e);
        }

        // 3. 删除旧表
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS folder_sync_states")
            .await?;

        tracing::info!("成功迁移 folder_sync_states 到 sync_state");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚：重新创建 folder_sync_states 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE folder_sync_states (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL,
                    imap_name TEXT NOT NULL,
                    folder_type TEXT,
                    uidvalidity INTEGER,
                    uidnext INTEGER,
                    synced_at INTEGER,
                    last_sync_uid INTEGER,
                    highest_uid INTEGER,
                    total_emails INTEGER,
                    sync_count INTEGER DEFAULT 0,
                    is_first_sync BOOLEAN DEFAULT 1,
                    error_count INTEGER DEFAULT 0,
                    last_error TEXT,
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

        // 迁移数据回去
        let rollback_data_sql = r#"
            INSERT INTO folder_sync_states (
                account_id, imap_name, folder_type, uidvalidity, uidnext,
                synced_at, last_sync_uid, highest_uid, total_emails, sync_count,
                is_first_sync, error_count, created_at, updated_at
            )
            SELECT
                account_id,
                folder as imap_name,
                NULL as folder_type,
                uidvalidity,
                uidnext,
                synced_at,
                last_sync_uid,
                NULL as highest_uid,
                NULL as total_emails,
                0 as sync_count,
                1 as is_first_sync,
                0 as error_count,
                NULL as last_error,
                created_at,
                updated_at
            FROM sync_state;
        "#;

        if let Err(e) = manager.get_connection().execute_unprepared(rollback_data_sql).await {
            tracing::warn!("回滚 sync_state 数据失败: {}", e);
        }

        // 删除新表
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sync_state")
            .await?;

        tracing::info!("成功回滚到 folder_sync_states");

        Ok(())
    }
}
