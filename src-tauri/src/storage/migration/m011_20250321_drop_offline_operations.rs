//! 数据库迁移：删除 offline_operations 表
//!
//! 由于 folders 表已被删除，而 offline_operations 表仍有对 folders 的外键引用，
//! 导致删除账号时出现 "no such table: main.folders" 错误。
//!
//! offline_operations 表目前未被使用，可以安全删除。

use sea_orm::{ConnectionTrait, DbConn, Statement, DbBackend};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查 offline_operations 表是否还存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM sqlite_master
        WHERE type='table' AND name='offline_operations'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count == 0 {
            tracing::info!("offline_operations 表不存在，跳过删除");
            return Ok(());
        }
    }

    // 删除 offline_operations 表
    let drop_table_sql = r#"
        DROP TABLE IF EXISTS offline_operations;
    "#;

    db.execute_unprepared(drop_table_sql).await?;
    tracing::info!("已删除 offline_operations 表");

    // 同时删除 email_sync_metadata 表（如果存在且未使用）
    let drop_metadata_sql = r#"
        DROP TABLE IF EXISTS email_sync_metadata;
    "#;

    db.execute_unprepared(drop_metadata_sql).await?;
    tracing::info!("已删除 email_sync_metadata 表");

    tracing::info!("数据库迁移完成: m011_drop_offline_operations");
    Ok(())
}

/// 回滚迁移
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 重建 offline_operations 表（用于回滚），但不包含对 folders 的外键引用
    let recreate_table_sql = r#"
        CREATE TABLE IF NOT EXISTS offline_operations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            folder_name TEXT,
            operation_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            synced_at INTEGER,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_offline_operations_account ON offline_operations(account_id);
        CREATE INDEX IF NOT EXISTS idx_offline_operations_synced ON offline_operations(synced_at);
        CREATE INDEX IF NOT EXISTS idx_offline_operations_type ON offline_operations(operation_type);
    "#;

    db.execute_unprepared(recreate_table_sql).await?;

    tracing::info!("数据库回滚完成: m011_drop_offline_operations");
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
        let sql = std::fs::read_to_string("src/storage/migration/m011_20250321_drop_offline_operations.rs")
            .expect("文件存在");
        assert!(sql.contains("DROP TABLE IF EXISTS offline_operations"));
    }
}
