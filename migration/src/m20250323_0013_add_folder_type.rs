//! 数据库迁移：添加文件夹类型字段
//!
//! 添加 `folder_type` 字段到 `folder_sync_states` 表

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查 folder_type 列是否已存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('folder_sync_states')
            WHERE name='folder_type'
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
                        ALTER TABLE folder_sync_states ADD COLUMN folder_type TEXT;
                    "#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 不支持 DROP COLUMN
        tracing::warn!("SQLite 不支持 DROP COLUMN，保留 folder_type 列");
        Ok(())
    }
}
