use sea_orm::{ConnectionTrait, DbConn};
use anyhow::Result;

/// 添加离线操作队列和邮件同步元数据表
pub async fn add_sync_operations(db: &DbConn) -> Result<()> {
    create_offline_operations_table(db).await?;
    create_email_sync_metadata_table(db).await?;

    tracing::info!("同步操作相关表创建完成");
    Ok(())
}

/// 创建 offline_operations 表 - 存储离线操作队列
async fn create_offline_operations_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS offline_operations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            folder_id INTEGER,
            operation_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            synced_at INTEGER,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
            FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE CASCADE
        )
    "#;

    db.execute_unprepared(sql)
    .await
    .map_err(|e| anyhow::anyhow!("创建 offline_operations 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_offline_operations_account ON offline_operations(account_id);
        CREATE INDEX IF NOT EXISTS idx_offline_operations_synced ON offline_operations(synced_at);
        CREATE INDEX IF NOT EXISTS idx_offline_operations_type ON offline_operations(operation_type);
    "#;

    db.execute_unprepared(index_sql)
    .await
    .map_err(|e| anyhow::anyhow!("创建 offline_operations 索引失败: {}", e))?;

    tracing::info!("offline_operations 表创建完成");
    Ok(())
}

/// 创建 email_sync_metadata 表 - 存储邮件同步元数据
async fn create_email_sync_metadata_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS email_sync_metadata (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email_id INTEGER NOT NULL UNIQUE,
            modseq INTEGER,
            body_fetched INTEGER DEFAULT 0,
            last_synced_at INTEGER,
            FOREIGN KEY (email_id) REFERENCES emails(id) ON DELETE CASCADE
        )
    "#;

    db.execute_unprepared(sql)
    .await
    .map_err(|e| anyhow::anyhow!("创建 email_sync_metadata 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_email_sync_metadata_email ON email_sync_metadata(email_id);
        CREATE INDEX IF NOT EXISTS idx_email_sync_metadata_body_fetched ON email_sync_metadata(body_fetched);
    "#;

    db.execute_unprepared(index_sql)
    .await
    .map_err(|e| anyhow::anyhow!("创建 email_sync_metadata 索引失败: {}", e))?;

    tracing::info!("email_sync_metadata 表创建完成");
    Ok(())
}
