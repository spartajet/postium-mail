//! 数据库迁移：删除 sync_states 表
//!
//! 该表的功能已合并到 `folder_sync_states` 表中

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 检查表是否存在
        let check_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='sync_states'
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
                    .execute_unprepared("DROP TABLE IF EXISTS sync_states")
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚需要重新创建 sync_states 表，但不提供此功能
        tracing::warn!("sync_states 表已删除，不支持回滚");
        Ok(())
    }
}
