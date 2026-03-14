use sea_orm::{Database, DbConn, DbErr, ConnectionTrait, Statement};
use anyhow::Result;
use crate::config;

/// 数据库连接池类型
pub type DatabaseResult = Result<DbConn, DbErr>;

/// 建立数据库连接
/// 使用 SQLite 数据库，位置在 ~/.postium/postium.db
pub async fn establish_connection() -> DatabaseResult {
    let db_url = config::get_db_path()
        .map_err(|e| DbErr::Custom(e.to_string()))?;

    Database::connect(&db_url).await
}

/// 初始化数据库
/// 创建必要的表和索引
pub async fn init_database(db: &DbConn) -> Result<()> {
    // 使用 SQL 直接创建表（避免复杂的迁移设置）
    create_accounts_table(db).await?;
    create_emails_table(db).await?;
    create_attachments_table(db).await?;
    create_fts5_table(db).await?;

    tracing::info!("数据库初始化完成");
    Ok(())
}

/// 创建 accounts 表
async fn create_accounts_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS accounts (
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
            password TEXT NOT NULL,
            color TEXT,
            sync_enabled INTEGER DEFAULT 1,
            last_sync_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 accounts 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
        CREATE INDEX IF NOT EXISTS idx_accounts_provider ON accounts(provider);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 accounts 索引失败: {}", e))?;

    Ok(())
}

/// 创建 emails 表
async fn create_emails_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS emails (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER NOT NULL,
            folder TEXT NOT NULL,
            uid INTEGER,
            message_id TEXT UNIQUE,
            subject TEXT,
            sender_name TEXT,
            sender_email TEXT NOT NULL,
            recipient_emails TEXT NOT NULL,
            cc_emails TEXT,
            bcc_emails TEXT,
            body_text TEXT,
            body_html TEXT,
            is_read INTEGER DEFAULT 0,
            is_starred INTEGER DEFAULT 0,
            is_draft INTEGER DEFAULT 0,
            sent_at INTEGER NOT NULL,
            received_at INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 emails 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_emails_account ON emails(account_id);
        CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder);
        CREATE INDEX IF NOT EXISTS idx_emails_sent_at ON emails(sent_at DESC);
        CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 emails 索引失败: {}", e))?;

    Ok(())
}

/// 创建 attachments 表
async fn create_attachments_table(db: &DbConn) -> Result<()> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email_id INTEGER NOT NULL,
            filename TEXT NOT NULL,
            content_type TEXT,
            size INTEGER NOT NULL,
            path TEXT,
            created_at INTEGER NOT NULL,
            FOREIGN KEY (email_id) REFERENCES emails(id) ON DELETE CASCADE
        )
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 attachments 表失败: {}", e))?;

    // 创建索引
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_attachments_email ON attachments(email_id);
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        index_sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 attachments 索引失败: {}", e))?;

    Ok(())
}

/// 创建 FTS5 全文检索表
async fn create_fts5_table(db: &DbConn) -> Result<()> {
    // 注意：jieba 分词器需要额外的 SQLite 扩展
    // 这里先使用简单的 tokenizer，后续可以集成 jieba
    let sql = r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
            subject,
            sender_email,
            body_text,
            content=emails,
            content_rowid=id
        );
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        sql.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 FTS5 表失败: {}", e))?;

    // 创建触发器自动同步数据
    let triggers = r#"
        CREATE TRIGGER IF NOT EXISTS emails_ai AFTER INSERT ON emails BEGIN
            INSERT INTO emails_fts(rowid, subject, sender_email, body_text)
            VALUES (new.id, new.subject, new.sender_email, new.body_text);
        END;

        CREATE TRIGGER IF NOT EXISTS emails_ad AFTER DELETE ON emails BEGIN
            DELETE FROM emails_fts WHERE rowid = old.id;
        END;

        CREATE TRIGGER IF NOT EXISTS emails_au AFTER UPDATE ON emails BEGIN
            UPDATE emails_fts
            SET subject = new.subject,
                sender_email = new.sender_email,
                body_text = new.body_text
            WHERE rowid = new.id;
        END;
    "#;

    db.execute(Statement::from_string(
        db.get_database_backend(),
        triggers.to_string(),
    ))
    .await
    .map_err(|e| anyhow::anyhow!("创建 FTS5 触发器失败: {}", e))?;

    Ok(())
}
