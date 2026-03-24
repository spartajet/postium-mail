//! 数据库迁移：删除 CONDSTORE/MODSEQ 支持
//!
//! 删除以下字段：
//! - folder_sync_states.highest_modseq
//! - sync_states.highest_modseq（如果表还存在）
//!
//! 原因：CONDSTORE 功能已从代码库中移除

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 从 folder_sync_states 表删除 highest_modseq 列
        let check_folder_sql = r#"
            SELECT COUNT(*) as count FROM pragma_table_info('folder_sync_states')
            WHERE name='highest_modseq'
        "#;

        let result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_folder_sql,
            ))
            .await?;

        if let Some(row) = result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count > 0 {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE folder_sync_states DROP COLUMN highest_modseq",
                    )
                    .await?;
                tracing::info!("已从 folder_sync_states 表删除 highest_modseq 列");
            }
        }

        // 从 sync_states 表删除 highest_modseq 列（如果表还存在）
        let check_sync_sql = r#"
            SELECT COUNT(*) as count FROM sqlite_master
            WHERE type='table' AND name='sync_states'
        "#;

        let result = manager
            .get_connection()
            .query_one_raw(crate::Statement::from_string(
                sea_orm::DbBackend::Sqlite,
                check_sync_sql,
            ))
            .await?;

        if let Some(row) = result {
            let count: i64 = row.try_get_by("count").unwrap_or(0);

            if count > 0 {
                // 检查 sync_states 表是否有 highest_modseq 列
                let check_column_sql = r#"
                    SELECT COUNT(*) as count FROM pragma_table_info('sync_states')
                    WHERE name='highest_modseq'
                "#;

                let column_result = manager
                    .get_connection()
                    .query_one_raw(crate::Statement::from_string(
                        sea_orm::DbBackend::Sqlite,
                        check_column_sql,
                    ))
                    .await?;

                if let Some(column_row) = column_result {
                    let column_count: i64 = column_row.try_get_by("count").unwrap_or(0);

                    if column_count > 0 {
                        manager
                            .get_connection()
                            .execute_unprepared(
                                "ALTER TABLE sync_states DROP COLUMN highest_modseq",
                            )
                            .await?;
                        tracing::info!("已从 sync_states 表删除 highest_modseq 列");
                    }
                }
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚需要重新添加 highest_modseq 字段，但 CONDSTORE 功能已移除
        tracing::warn!("MODSEQ 支持已删除，不支持回滚");
        Ok(())
    }
}
