//! 数据库迁移：添加 OAuth 相关字段
//!
//! 注意：此迁移仅用于向后兼容，新数据库已在 m001 中包含这些字段。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 auth_type 列是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('accounts') WHERE name='auth_type'
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
                // 添加新字段
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE accounts ADD COLUMN auth_type TEXT DEFAULT 'password';
                        ALTER TABLE accounts ADD COLUMN oauth_provider TEXT;
                        ALTER TABLE accounts ADD COLUMN oauth_token TEXT;
                        ALTER TABLE accounts ADD COLUMN oauth_refresh_token TEXT;
                        ALTER TABLE accounts ADD COLUMN oauth_expires_at INTEGER;
                    "#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留 OAuth 字段");
        Ok(())
    }
}
