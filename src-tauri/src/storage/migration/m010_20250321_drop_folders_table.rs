//! 数据库迁移：删除旧的 folders 表
//!
//! 由于文件夹配置已由 provider.folder_mapping() 提供，
//! 同步状态已迁移到 folder_sync_states 表，
//! 旧的 folders 表不再需要，予以删除。

use sea_orm::{ConnectionTrait, DbConn, Statement, DbBackend};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查 folders 表是否还存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM sqlite_master
        WHERE type='table' AND name='folders'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count == 0 {
            tracing::info!("folders 表不存在，跳过删除");
            return Ok(());
        }
    }

    // 删除旧的 folders 表
    let drop_folders_table_sql = r#"
        DROP TABLE IF EXISTS folders;
    "#;

    db.execute_unprepared(drop_folders_table_sql).await?;
    tracing::info!("已删除旧的 folders 表");

    tracing::info!("数据库迁移完成: m010_drop_folders_table");
    Ok(())
}

/// 回滚迁移
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 重建 folders 表（用于回滚）
    let recreate_folders_table_sql = r#"
        CREATE TABLE IF NOT EXISTS folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            imap_name TEXT NOT NULL,
            parent_id INTEGER,
            attributes TEXT,
            email_count INTEGER DEFAULT 0,
            unread_count INTEGER DEFAULT 0,
            synced_at INTEGER,
            uidvalidity INTEGER,
            uidnext INTEGER,
            highest_modseq INTEGER,
            created_at INTEGER DEFAULT (strftime('%s', 'now')),
            updated_at INTEGER DEFAULT (strftime('%s', 'now')),
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
            UNIQUE(account_id, imap_name)
        );

        CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
    "#;

    db.execute_unprepared(recreate_folders_table_sql).await?;

    // 从 folder_sync_states 迁移数据回 folders 表（如果有的话）
    let rollback_data_sql = r#"
        INSERT INTO folders (account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at, email_count, unread_count)
        SELECT account_id, imap_name, uidvalidity, uidnext, highest_modseq, synced_at, 0, 0
        FROM folder_sync_states;
    "#;

    if let Err(e) = db.execute_unprepared(rollback_data_sql).await {
        tracing::warn!("回滚 folders 数据失败（可能 folder_sync_states 不存在）: {}", e);
    } else {
        tracing::info!("已从 folder_sync_states 恢复数据到 folders 表");
    }

    tracing::info!("数据库回滚完成: m010_drop_folders_table");
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
        let sql = std::fs::read_to_string("src/storage/migration/m010_20250321_drop_folders_table.rs")
            .expect("文件存在");
        assert!(sql.contains("DROP TABLE IF EXISTS folders"));
    }
}
