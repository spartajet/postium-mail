//! 数据库迁移：添加账号类型
//!
//! 添加个人/企业邮箱的区分机制

use sea_orm::{ConnectionTrait, DbConn};

/// 执行迁移
pub async fn migrate(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 1. 添加 account_type 字段到 accounts 表
    let alter_sql = r#"
        ALTER TABLE accounts
        ADD COLUMN account_type TEXT DEFAULT 'personal'
        NOT NULL CHECK(account_type IN ('personal', 'enterprise'));
    "#;

    db.execute_unprepared(alter_sql).await?;

    // 2. 创建 enterprise_configs 表
    let create_table_sql = r#"
        CREATE TABLE IF NOT EXISTS enterprise_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL UNIQUE,
            tenant_id TEXT,
            domain TEXT,
            conditional_access INTEGER DEFAULT 0,
            mfa_required INTEGER DEFAULT 0,
            custom_server INTEGER DEFAULT 0,
            custom_imap_host TEXT,
            custom_imap_port INTEGER,
            custom_imap_ssl INTEGER DEFAULT 1,
            custom_smtp_host TEXT,
            custom_smtp_port INTEGER,
            custom_smtp_ssl INTEGER DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );
    "#;

    db.execute_unprepared(create_table_sql).await?;

    // 3. 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_enterprise_configs_account
        ON enterprise_configs(account_id);
    "#;

    db.execute_unprepared(index_sql).await?;

    tracing::info!("数据库迁移完成: m007_add_account_types");
    Ok(())
}

/// 回滚迁移
pub async fn rollback(db: &DbConn) -> Result<(), sea_orm::DbErr> {
    // 删除表
    let drop_table_sql = "DROP TABLE IF EXISTS enterprise_configs;";
    db.execute_unprepared(drop_table_sql).await?;

    // SQLite 不支持 DROP COLUMN，需要重建表
    tracing::warn!("SQLite 不支持 DROP COLUMN，需要手动重建 accounts 表");

    tracing::info!("数据库回滚完成: m007_add_account_types");
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_migration_sql() {
        // 验证 SQL 语法
        let sql = std::fs::read_to_string("src/storage/migration/m007_20250317_add_account_types.rs")
            .expect("文件存在");
        assert!(sql.contains("ALTER TABLE accounts"));
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS enterprise_configs"));
    }
}
