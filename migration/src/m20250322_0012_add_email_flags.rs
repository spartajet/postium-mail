//! 数据库迁移：添加邮件标志字段
//!
//! 添加 `is_answered` 和 `is_deleted` 布尔字段

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 is_answered 列是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('emails')
            WHERE name='is_answered'
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
                        ALTER TABLE emails ADD COLUMN is_answered BOOLEAN NOT NULL DEFAULT 0;
                        ALTER TABLE emails ADD COLUMN is_deleted BOOLEAN NOT NULL DEFAULT 0;
                    "#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留 is_answered 和 is_deleted 列");
        Ok(())
    }
}
