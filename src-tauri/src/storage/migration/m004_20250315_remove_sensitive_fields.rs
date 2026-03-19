use anyhow::Result;
use sea_orm::{ConnectionTrait, DbConn, Statement, DbBackend};

/// 删除数据库中的敏感字段（密码和 OAuth token）
/// 这些敏感数据现在只存储在 Stronghold 中
///
/// 注意：由于 SQLite 限制，需要重建表来删除列
/// 新安装的数据库（m001）已经不包含这些字段
pub async fn migrate_remove_sensitive_fields(db: &DbConn) -> Result<()> {
    // 检查是否存在需要删除的敏感字段
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('accounts') WHERE name='password'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await
        .map_err(|e| anyhow::anyhow!("检查列失败: {}", e))?;

    if let Some(row) = result {
        let count: i64 = row
            .try_get_by("count")
            .map_err(|e| anyhow::anyhow!("解析检查结果失败: {}", e))?;

        if count > 0 {
            tracing::info!("检测到旧版本数据库，正在删除敏感字段（password, oauth_token, oauth_refresh_token）...");

            // SQLite 不支持 DROP COLUMN，需要重建表
            // 步骤：创建新表 → 复制数据 → 删除旧表 → 重命名新表 → 重建索引

            // 1. 创建新表（不包含敏感字段）
            db.execute_unprepared(
                r#"
                    CREATE TABLE accounts_new (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL,
                        email TEXT NOT NULL UNIQUE,
                        provider TEXT NOT NULL,
                        imap_host TEXT,
                        imap_port INTEGER,
                        imap_ssl INTEGER DEFAULT 1,
                        smtp_host TEXT,
                        smtp_port INTEGER,
                        smtp_ssl INTEGER DEFAULT 1,
                        color TEXT,
                        sync_enabled INTEGER DEFAULT 1,
                        last_sync_at INTEGER,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL,
                        auth_type TEXT DEFAULT 'password',
                        oauth_provider TEXT,
                        oauth_expires_at INTEGER
                    )
                "#,
            )
            .await
            .map_err(|e| anyhow::anyhow!("创建新表失败: {}", e))?;

            // 2. 复制数据（排除敏感字段）
            db.execute_unprepared(
                r#"
                    INSERT INTO accounts_new
                    SELECT id, name, email, provider,
                           imap_host, imap_port, imap_ssl,
                           smtp_host, smtp_port, smtp_ssl,
                           color, sync_enabled, last_sync_at,
                           created_at, updated_at,
                           auth_type, oauth_provider, oauth_expires_at
                    FROM accounts
                "#,
            )
            .await
            .map_err(|e| anyhow::anyhow!("复制数据失败: {}", e))?;

            // 3. 删除旧表
            db.execute_unprepared("DROP TABLE accounts")
            .await
            .map_err(|e| anyhow::anyhow!("删除旧表失败: {}", e))?;

            // 4. 重命名新表
            db.execute_unprepared("ALTER TABLE accounts_new RENAME TO accounts")
            .await
            .map_err(|e| anyhow::anyhow!("重命名表失败: {}", e))?;

            // 5. 重建索引
            db.execute_unprepared(
                r#"
                    CREATE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
                    CREATE INDEX IF NOT EXISTS idx_accounts_provider ON accounts(provider);
                "#,
            )
            .await
            .map_err(|e| anyhow::anyhow!("创建索引失败: {}", e))?;

            tracing::info!("数据库迁移完成：敏感字段已删除");
        } else {
            tracing::debug!("数据库已是最新版本（无需迁移）");
        }
    }

    Ok(())
}

/// 运行所有迁移
pub async fn run_migrations(db: &DbConn) -> Result<()> {
    migrate_remove_sensitive_fields(db).await?;
    Ok(())
}
