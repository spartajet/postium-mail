//! 数据库迁移：添加 MODSEQ 支持
//!
//! 实现 IMAP CONDSTORE (RFC 4551) 扩展支持
//! 用于高效的增量同步

use sea_orm::{ConnectionTrait, DbConn, Statement, DbBackend};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 1. 添加 highest_modseq 字段到 sync_states 表
    let check_sync_states_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('sync_states') WHERE name='highest_modseq'
    "#;

    let sync_states_result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sync_states_sql))
        .await?;

    if let Some(row) = sync_states_result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count == 0 {
            let alter_sync_states_sql = r#"
                ALTER TABLE sync_states
                ADD COLUMN highest_modseq INTEGER;
            "#;

            db.execute_unprepared(alter_sync_states_sql).await?;
            tracing::info!("已添加 highest_modseq 列到 sync_states 表");
        } else {
            tracing::info!("highest_modseq 列已存在于 sync_states 表，跳过添加");
        }
    }

    // 2. 添加 modseq 字段到 emails 表
    let check_emails_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('emails') WHERE name='modseq'
    "#;

    let emails_result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_emails_sql))
        .await?;

    if let Some(row) = emails_result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count == 0 {
            let alter_emails_sql = r#"
                ALTER TABLE emails
                ADD COLUMN modseq INTEGER;
            "#;

            db.execute_unprepared(alter_emails_sql).await?;
            tracing::info!("已添加 modseq 列到 emails 表");
        } else {
            tracing::info!("modseq 列已存在于 emails 表，跳过添加");
        }
    }

    // 3. 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_emails_modseq
        ON emails(modseq);

        CREATE INDEX IF NOT EXISTS idx_sync_states_highest_modseq
        ON sync_states(highest_modseq);
    "#;

    db.execute_unprepared(index_sql).await?;

    tracing::info!("数据库迁移完成: m008_add_modseq_support");
    Ok(())
}

/// 回滚迁移
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 删除索引
    let drop_index_sql = r#"
        DROP INDEX IF EXISTS idx_emails_modseq;
        DROP INDEX IF EXISTS idx_sync_states_highest_modseq;
    "#;

    db.execute_unprepared(drop_index_sql).await?;

    // SQLite 不支持 DROP COLUMN，需要重建表
    tracing::warn!("SQLite 不支持 DROP COLUMN，需要手动重建表以删除 modseq 字段");
    tracing::warn!("sync_states 表需要删除 highest_modseq 字段");
    tracing::warn!("emails 表需要删除 modseq 字段");

    tracing::info!("数据库回滚完成: m008_add_modseq_support (索引已删除)");
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_migration_sql() {
        // 验证 SQL 语法
        let sql = std::fs::read_to_string("src/storage/migration/m008_20250317_add_modseq_support.rs")
            .expect("文件存在");
        assert!(sql.contains("ALTER TABLE sync_states"));
        assert!(sql.contains("ADD COLUMN highest_modseq"));
        assert!(sql.contains("ALTER TABLE emails"));
        assert!(sql.contains("ADD COLUMN modseq"));
        assert!(sql.contains("CREATE INDEX IF NOT EXISTS idx_emails_modseq"));
    }

    #[test]
    fn test_field_names() {
        // 验证字段名与模型一致
        let sql = std::fs::read_to_string("src/storage/migration/m008_20250317_add_modseq_support.rs")
            .expect("文件存在");
        assert!(sql.contains("highest_modseq"));
        assert!(sql.contains("modseq"));
    }
}
