use sea_orm::{ConnectionTrait, Statement, DbConn};
use anyhow::Result;

/// 添加同步相关表结构
pub async fn add_sync_tables(db: &DbConn) -> Result<()> {
    create_folders_table(db).await?;
    create_sync_states_table(db).await?;
    create_sync_errors_table(db).await?;

    tracing::info!("同步相关表创建完成");
    Ok(())
}

/// 创建 folders 表
async fn create_folders_table(db: &DbConn) -> Result<()> {
    let sql = r#"
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
            UNIQUE(account_id, imap_name),
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
            FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 folders 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_folders_account ON folders(account_id);
        CREATE INDEX IF NOT EXISTS idx_folders_name ON folders(name);
        CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 folders 索引失败: {}", e))?;

    tracing::info!("folders 表创建完成");
    Ok(())
}

/// 创建 sync_states 表
async fn create_sync_states_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS sync_states (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            folder TEXT NOT NULL,
            last_sync_uid INTEGER,
            last_sync_at INTEGER,
            highest_uid INTEGER,
            total_emails INTEGER,
            sync_count INTEGER DEFAULT 0,
            is_first_sync BOOLEAN DEFAULT 1,
            error_count INTEGER DEFAULT 0,
            last_error TEXT,
            updated_at INTEGER NOT NULL,
            UNIQUE(account_id, folder),
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 sync_states 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_sync_states_account ON sync_states(account_id);
        CREATE INDEX IF NOT EXISTS idx_sync_states_folder ON sync_states(folder);
        CREATE INDEX IF NOT EXISTS idx_sync_states_last_sync ON sync_states(last_sync_at);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 sync_states 索引失败: {}", e))?;

    tracing::info!("sync_states 表创建完成");
    Ok(())
}

/// 创建 sync_errors 表
async fn create_sync_errors_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS sync_errors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            folder TEXT,
            error_type TEXT NOT NULL,
            error_message TEXT NOT NULL,
            uid INTEGER,
            stack_trace TEXT,
            resolved BOOLEAN DEFAULT 0,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 sync_errors 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_sync_errors_account ON sync_errors(account_id);
        CREATE INDEX IF NOT EXISTS idx_sync_errors_type ON sync_errors(error_type);
        CREATE INDEX IF NOT EXISTS idx_sync_errors_resolved ON sync_errors(resolved);
        CREATE INDEX IF NOT EXISTS idx_sync_errors_created ON sync_errors(created_at);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 sync_errors 索引失败: {}", e))?;

    tracing::info!("sync_errors 表创建完成");
    Ok(())
}
