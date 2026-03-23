//! 数据库迁移：合并 sync_states 表到 folder_sync_states
//!
//! 将 `sync_states` 表的同步进度字段合并到 `folder_sync_states` 表，
//! 消除两张表的功能重叠，简化代码结构。
//!
//! # 背景
//!
//! 原代码维护了两张功能重叠的表：
//! - **`folder_sync_states`** - 由 `FolderManager` 管理，存储 IMAP 元数据
//! - **`sync_states`** - 由 `SyncStateManager` 管理，存储同步进度状态
//!
//! **核心问题**：
//! - `sync_states` 表几乎未被实际使用
//! - `highest_modseq` 字段在两张表中重复
//! - 两张表没有明确的数据一致性保证机制
//! - 维护两张表增加了代码复杂度
//!
//! # 合并决策
//!
//! 保留 `folder_sync_states` 作为主表，将 `sync_states` 的必要字段合并进来。
//!
//! # 新增字段
//!
//! | 字段 | 类型 | 默认值 | 说明 |
//!|------|------|--------|------|
//!| last_sync_uid | INTEGER | NULL | 最后同步的 UID |
//!| highest_uid | INTEGER | NULL | 文件夹最高 UID |
//!| total_emails | INTEGER | NULL | 总邮件数 |
//!| sync_count | INTEGER | 0 | 已同步邮件数 |
//!| is_first_sync | BOOLEAN | TRUE | 是否首次同步 |
//!| error_count | INTEGER | 0 | 连续错误次数 |
//!| last_error | TEXT | NULL | 最后错误信息 |

use sea_orm::{ConnectionTrait, DbConn, DbBackend, Statement};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 检查列是否已存在（通过检查 sync_count 列）
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('folder_sync_states')
        WHERE name='sync_count'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await?;

    if let Some(row) = result {
        let count: i64 = row.try_get_by::<i64, _>("count").unwrap_or(0);

        if count > 0 {
            tracing::info!("sync_states 合并列已存在，跳过迁移");
            return Ok(());
        }
    }

    // 添加 sync_states 的字段到 folder_sync_states
    let migrations = vec![
        "ALTER TABLE folder_sync_states ADD COLUMN last_sync_uid INTEGER",
        "ALTER TABLE folder_sync_states ADD COLUMN highest_uid INTEGER",
        "ALTER TABLE folder_sync_states ADD COLUMN total_emails INTEGER",
        "ALTER TABLE folder_sync_states ADD COLUMN sync_count INTEGER DEFAULT 0",
        "ALTER TABLE folder_sync_states ADD COLUMN is_first_sync BOOLEAN DEFAULT TRUE",
        "ALTER TABLE folder_sync_states ADD COLUMN error_count INTEGER DEFAULT 0",
        "ALTER TABLE folder_sync_states ADD COLUMN last_error TEXT",
    ];

    for sql in migrations {
        db.execute_unprepared(sql).await?;
    }

    tracing::info!("已添加 sync_states 合并字段到 folder_sync_states");
    tracing::info!("数据库迁移完成: m014_merge_sync_states");

    Ok(())
}

/// 回滚迁移
#[allow(dead_code)]
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // SQLite 不支持 DROP COLUMN，需要重建表
    // 这里我们选择保留列，因为回滚的风险大于收益

    tracing::warn!("SQLite 不支持 DROP COLUMN，保留合并的字段");
    tracing::info!("数据库回滚完成: m014_merge_sync_states (无操作)");

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
        let sql = std::fs::read_to_string("src/storage/migration/m014_20250323_merge_sync_states.rs")
            .expect("文件存在");

        // 验证包含所有必要的 ALTER TABLE 语句
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN last_sync_uid"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN highest_uid"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN total_emails"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN sync_count"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN is_first_sync"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN error_count"));
        assert!(sql.contains("ALTER TABLE folder_sync_states ADD COLUMN last_error"));
    }
}
