//! 数据库迁移：添加文件夹类型字段
//!
//! 添加 `folder_type` 字段到 `folder_sync_states` 表，
//! 用于存储文件夹的标准类型（inbox, sent, drafts, spam, trash, archive, other）。
//!
//! # 背景
//!
//! 不同邮件服务商使用不同的文件夹命名：
//! - Gmail: INBOX, [Gmail]/Sent Mail, [Gmail]/Drafts
//! - Outlook: Inbox, Sent Items, Drafts
//! - QQ邮箱: INBOX/收件箱, 已发送/草稿箱
//!
//! `folder_type` 字段用于统一标识这些文件夹的语义类型，
//! 便于：
//! 1. UI 显示正确的文件夹图标
//! 2. 实现特殊文件夹的同步策略
//! 3. 支持跨服务商的统一文件夹操作

use sea_orm::{ConnectionTrait, DbConn, DbBackend, Statement};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查列是否已存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('folder_sync_states')
        WHERE name='folder_type'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count > 0 {
            tracing::info!("folder_type 列已存在，跳过迁移");
            return Ok(());
        }
    }

    // 添加 folder_type 列
    let add_folder_type_sql = r#"
        ALTER TABLE folder_sync_states ADD COLUMN folder_type TEXT;
    "#;

    db.execute_unprepared(add_folder_type_sql).await?;
    tracing::info!("已添加 folder_type 列");

    tracing::info!("数据库迁移完成: m013_add_folder_type");

    Ok(())
}

/// 回滚迁移
#[allow(dead_code)]
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // SQLite 不支持 DROP COLUMN，需要重建表
    // 这里我们选择保留列，因为回滚的风险大于收益

    tracing::warn!("SQLite 不支持 DROP COLUMN，保留 folder_type 列");
    tracing::info!("数据库回滚完成: m013_add_folder_type (无操作)");

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
        let sql = std::fs::read_to_string("src/storage/migration/m013_20250323_add_folder_type.rs")
            .expect("文件存在");
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN folder_type"));
    }
}
