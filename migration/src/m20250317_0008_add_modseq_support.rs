//! 数据库迁移：添加 MODSEQ 支持（CONDSTORE 扩展）

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. 添加 highest_modseq 字段到 sync_states 表
        let check_sync_states_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('sync_states') WHERE name='highest_modseq'
        "#;

        let sync_states_result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_sync_states_sql,
            ))
            .await?;

        if let Some(row) = sync_states_result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count == 0 {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE sync_states
                        ADD COLUMN highest_modseq INTEGER;
                    "#,
                    )
                    .await?;
            }
        }

        // 2. 添加 modseq 字段到 emails 表
        let check_emails_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('emails') WHERE name='modseq'
        "#;

        let emails_result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_emails_sql,
            ))
            .await?;

        if let Some(row) = emails_result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count == 0 {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE emails
                        ADD COLUMN modseq INTEGER;
                    "#,
                    )
                    .await?;
            }
        }

        // 3. 创建索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_emails_modseq ON emails(modseq);
                CREATE INDEX IF NOT EXISTS idx_sync_states_highest_modseq ON sync_states(highest_modseq);
            "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_emails_modseq")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_sync_states_highest_modseq")
            .await?;

        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留 modseq 字段");

        Ok(())
    }
}
