use sea_orm::{ConnectionTrait, DbConn, DbBackend, Statement};
use anyhow::Result;

/// 为 folders 表添加 IMAP 元数据字段
pub async fn add_imap_metadata(db: &DbConn) -> Result<()> {
    // 检查 uidvalidity 列是否已存在
    let check_sql = r#"
        SELECT COUNT(*) as count FROM pragma_table_info('folders') WHERE name='uidvalidity'
    "#;

    let result = db
        .query_one_raw(Statement::from_string(DbBackend::Sqlite, check_sql))
        .await
        .map_err(|e| anyhow::anyhow!("检查列失败: {}", e))?;

    // 如果列不存在，则添加
    if let Some(row) = result {
        let count: i64 = row
            .try_get_by("count")
            .map_err(|e| anyhow::anyhow!("解析检查结果失败: {}", e))?;

        if count == 0 {
            tracing::info!("检测到旧版本数据库，添加 IMAP 元数据字段...");

            let sql = r#"
                ALTER TABLE folders ADD COLUMN uidvalidity INTEGER;
                ALTER TABLE folders ADD COLUMN uidnext INTEGER;
                ALTER TABLE folders ADD COLUMN highest_modseq INTEGER;
            "#;

            db.execute_unprepared(sql)
            .await
            .map_err(|e| anyhow::anyhow!("添加 IMAP 元数据字段失败: {}", e))?;

            tracing::info!("folders 表添加 IMAP 元数据字段完成");
        } else {
            tracing::info!("IMAP 元数据字段已存在，跳过迁移");
        }
    }

    Ok(())
}
