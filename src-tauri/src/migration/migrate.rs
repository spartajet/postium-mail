use sea_orm::{ConnectionTrait, Statement, DbConn};
use anyhow::Result;

/// 添加 OAuth 相关字段到 accounts 表
pub async fn migrate_add_oauth_fields(db: &DbConn) -> Result<()> {
    // 检查 auth_type 列是否已存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('accounts') WHERE name='auth_type'
    "#;

    let result = db.query_one(Statement::from_string(
        db.get_database_backend(),
        check_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("检查列失败: {}", e))?;

    // 如果列不存在，则添加
    if let Some(row) = result {
        let count: i64 = row.try_get_by("count")
            .map_err(|e| anyhow::anyhow!("解析检查结果失败: {}", e))?;

        if count == 0 {
            tracing::info!("检测到旧版本数据库，添加 OAuth 字段...");

            // 添加新字段
            let alter_sql = r#"
                ALTER TABLE accounts ADD COLUMN auth_type TEXT DEFAULT 'password';
                ALTER TABLE accounts ADD COLUMN oauth_provider TEXT;
                ALTER TABLE accounts ADD COLUMN oauth_token TEXT;
                ALTER TABLE accounts ADD COLUMN oauth_refresh_token TEXT;
                ALTER TABLE accounts ADD COLUMN oauth_expires_at INTEGER;
            "#;

            db.execute(Statement::from_string(
                db.get_database_backend(),
                alter_sql.to_string(),
            ))
            .await
            .map_err(|e| anyhow::anyhow!("添加 OAuth 字段失败: {}", e))?;

            tracing::info!("数据库迁移完成：添加 OAuth 字段");
        } else {
            tracing::info!("数据库已是最新版本");
        }
    }

    Ok(())
}

/// 运行所有迁移
pub async fn run_migrations(db: &DbConn) -> Result<()> {
    migrate_add_oauth_fields(db).await?;
    Ok(())
}
