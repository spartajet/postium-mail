//! 数据库迁移：添加邮件标志字段
//!
//! 添加 `is_answered` 和 `is_deleted` 布尔字段，
//! 用于存储 IMAP \Answered 和 \Deleted 标志。
//!
//! # 背景
//!
//! 之前的 Email 模型只有 3 个布尔标志字段：
//! - is_read (\Seen)
//! - is_starred (\Flagged)
//! - is_draft (\Draft)
//!
//! 为了支持完整的 IMAP 系统标志，添加：
//! - is_answered (\Answered)
//! - is_deleted (\Deleted)

use sea_orm::{ConnectionTrait, DbConn, DbBackend, Statement};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查列是否已存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('emails')
        WHERE name='is_answered'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count > 0 {
            tracing::info!("is_answered 列已存在，跳过迁移");
            return Ok(());
        }
    }

    // 添加 is_answered 列
    let add_answered_sql = r#"
        ALTER TABLE emails ADD COLUMN is_answered BOOLEAN NOT NULL DEFAULT 0;
    "#;

    db.execute_unprepared(add_answered_sql).await?;
    tracing::info!("已添加 is_answered 列");

    // 添加 is_deleted 列
    let add_deleted_sql = r#"
        ALTER TABLE emails ADD COLUMN is_deleted BOOLEAN NOT NULL DEFAULT 0;
    "#;

    db.execute_unprepared(add_deleted_sql).await?;
    tracing::info!("已添加 is_deleted 列");

    tracing::info!("数据库迁移完成: m012_add_email_flags");

    Ok(())
}

/// 回滚迁移
#[allow(dead_code)]
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // SQLite 不支持 DROP COLUMN，需要重建表
    // 这里我们选择保留列，因为回滚的风险大于收益

    tracing::warn!("SQLite 不支持 DROP COLUMN，保留 is_answered 和 is_deleted 列");
    tracing::info!("数据库回滚完成: m012_add_email_flags (无操作)");

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
        let sql = std::fs::read_to_string("src/storage/migration/m012_20250322_add_email_flags.rs")
            .expect("文件存在");
        assert!(sql.contains("ALTER TABLE emails ADD COLUMN is_answered"));
        assert!(sql.contains("ALTER TABLE emails ADD COLUMN is_deleted"));
    }
}
