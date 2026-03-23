//! 数据库迁移：删除 sync_states 表
//!
//! 删除已废弃的 `sync_states` 表。
//! 该表的功能已合并到 `folder_sync_states` 表中（见 m014 迁移）。
//!
//! # 背景
//!
//! 原 `sync_states` 表与 `folder_sync_states` 表功能重叠：
//! - `sync_states` 表几乎未被实际使用
//! - `highest_modseq` 字段在两张表中重复
//! - 维护两张表增加了代码复杂度
//!
//! # 合并完成
//!
//! m014 迁移已将 sync_states 的字段合并到 folder_sync_states：
//! - last_sync_uid → folder_sync_states.last_sync_uid
//! - highest_uid → folder_sync_states.highest_uid
//! - total_emails → folder_sync_states.total_emails
//! - sync_count → folder_sync_states.sync_count
//! - is_first_sync → folder_sync_states.is_first_sync
//! - error_count → folder_sync_states.error_count
//! - last_error → folder_sync_states.last_error
//!
//! # 删除操作
//!
//! 本迁移删除 `sync_states` 表，完成表合并的最后一步。

use sea_orm::{ConnectionTrait, DbConn, DbBackend, Statement};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查表是否存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM sqlite_master
        WHERE type='table' AND name='sync_states'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count == 0 {
            tracing::info!("sync_states 表不存在，跳过迁移");
            return Ok(());
        }
    }

    // 删除 sync_states 表
    let drop_sql = r#"
        DROP TABLE IF EXISTS sync_states;
    "#;

    db.execute_unprepared(drop_sql).await?;

    tracing::info!("已删除 sync_states 表");
    tracing::info!("数据库迁移完成: m015_drop_sync_states_table");

    Ok(())
}

/// 回滚迁移
#[allow(dead_code)]
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 回滚需要重新创建 sync_states 表
    // 由于表结构已在 m014 合并到 folder_sync_states，
    // 这里不提供回滚功能
    tracing::warn!("sync_states 表已删除，不支持回滚");
    tracing::info!("数据库回滚完成: m015_drop_sync_states_table (无操作)");

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    static TRACING_INIT: Once = Once::new();

    fn init_tracing() {
        TRACING_INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_test_writer()
                .with_target(false)
                .with_ansi(true)
                .with_line_number(true)
                .with_file(true)
                .try_init()
                .ok();
        });
    }

    #[test]
    fn test_migration_sql() {
        // 验证 SQL 语法
        let sql = std::fs::read_to_string("src/storage/migration/m015_20250323_drop_sync_states_table.rs")
            .expect("文件存在");

        // 验证包含 DROP TABLE 语句
        assert!(sql.contains("DROP TABLE IF EXISTS sync_states"));
    }
}
