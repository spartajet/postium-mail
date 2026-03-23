//! 数据库迁移：添加账号类型

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 account_type 列是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('accounts') WHERE name='account_type'
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
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE accounts
                        ADD COLUMN account_type TEXT DEFAULT 'personal'
                        NOT NULL CHECK(account_type IN ('personal', 'enterprise'));
                    "#,
                    )
                    .await?;
            }
        }

        // 创建 enterprise_configs 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE IF NOT EXISTS enterprise_configs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_id INTEGER NOT NULL UNIQUE,
                    tenant_id TEXT,
                    domain TEXT,
                    conditional_access INTEGER DEFAULT 0,
                    mfa_required INTEGER DEFAULT 0,
                    custom_server INTEGER DEFAULT 0,
                    custom_imap_host TEXT,
                    custom_imap_port INTEGER,
                    custom_imap_ssl INTEGER DEFAULT 1,
                    custom_smtp_host TEXT,
                    custom_smtp_port INTEGER,
                    custom_smtp_ssl INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
                );
            "#,
            )
            .await?;

        // 创建索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_enterprise_configs_account
                ON enterprise_configs(account_id);
            "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS enterprise_configs")
            .await?;

        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留 account_type 字段");

        Ok(())
    }
}
