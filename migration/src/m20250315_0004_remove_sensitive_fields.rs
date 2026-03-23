//! 数据库迁移：删除敏感字段
//!
//! 删除数据库中的敏感字段（密码和 OAuth token）
//! 这些敏感数据现在只存储在 Stronghold 中

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查是否存在需要删除的敏感字段
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('accounts') WHERE name='password'
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
                // SQLite 不支持 DROP COLUMN，需要重建表
                // 1. 创建新表（不包含敏感字段）
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TABLE accounts_new (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL,
                            email TEXT NOT NULL UNIQUE,
                            provider TEXT NOT NULL,
                            imap_host TEXT,
                            imap_port INTEGER,
                            imap_ssl INTEGER DEFAULT 1,
                            smtp_host TEXT,
                            smtp_port INTEGER,
                            smtp_ssl INTEGER DEFAULT 1,
                            color TEXT,
                            sync_enabled INTEGER DEFAULT 1,
                            last_sync_at INTEGER,
                            created_at INTEGER NOT NULL,
                            updated_at INTEGER NOT NULL,
                            auth_type TEXT DEFAULT 'password',
                            oauth_provider TEXT,
                            oauth_expires_at INTEGER
                        )
                    "#,
                    )
                    .await?;

                // 2. 复制数据（排除敏感字段）
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        INSERT INTO accounts_new
                        SELECT id, name, email, provider,
                               imap_host, imap_port, imap_ssl,
                               smtp_host, smtp_port, smtp_ssl,
                               color, sync_enabled, last_sync_at,
                               created_at, updated_at,
                               auth_type, oauth_provider, oauth_expires_at
                        FROM accounts
                    "#,
                    )
                    .await?;

                // 3. 删除旧表
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE accounts")
                    .await?;

                // 4. 重命名新表
                manager
                    .get_connection()
                    .execute_unprepared("ALTER TABLE accounts_new RENAME TO accounts")
                    .await?;

                // 5. 重建索引
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
                        CREATE INDEX IF NOT EXISTS idx_accounts_provider ON accounts(provider);
                    "#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚需要重新添加敏感字段，但这不符合安全策略
        tracing::warn!("回滚迁移不会重新添加敏感字段");
        Ok(())
    }
}
