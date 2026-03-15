use sea_orm::{ConnectionTrait, Statement, DbConn};
use anyhow::Result;

/// 为 folders 表添加 IMAP 元数据字段
pub async fn add_imap_metadata(db: &DbConn) -> Result<()> {
    let sql = r#"
        ALTER TABLE folders ADD COLUMN uidvalidity INTEGER;
        ALTER TABLE folders ADD COLUMN uidnext INTEGER;
        ALTER TABLE folders ADD COLUMN highest_modseq INTEGER;
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("添加 IMAP 元数据字段失败: {}", e))?;

    tracing::info!("folders 表添加 IMAP 元数据字段完成");
    Ok(())
}
