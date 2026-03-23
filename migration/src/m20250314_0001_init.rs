//! 数据库迁移：初始化数据库表结构

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 创建 accounts 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
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
            .await?;

        // 创建 accounts 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_accounts_email ON accounts(email);
                CREATE INDEX IF NOT EXISTS idx_accounts_provider ON accounts(provider);
            "#,
            )
            .await?;

        // 创建 emails 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
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
            "#,
            )
            .await?;

        // 创建 emails 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_emails_account ON emails(account_id);
                CREATE INDEX IF NOT EXISTS idx_emails_folder ON emails(folder);
                CREATE INDEX IF NOT EXISTS idx_emails_sent_at ON emails(sent_at DESC);
                CREATE INDEX IF NOT EXISTS idx_emails_is_read ON emails(is_read);
            "#,
            )
            .await?;

        // 创建 attachments 表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
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
            "#,
            )
            .await?;

        // 创建 attachments 索引
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_attachments_email ON attachments(email_id);
            "#,
            )
            .await?;

        // 创建 FTS5 全文检索表
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS emails_fts USING fts5(
                    subject,
                    sender_email,
                    body_text,
                    content=emails,
                    content_rowid=id
                );
            "#,
            )
            .await?;

        // 创建 FTS5 触发器
        manager
            .get_connection()
            .execute_unprepared(
                r#"
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
            "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除 FTS5 触发器
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TRIGGER IF EXISTS emails_au;
                DROP TRIGGER IF EXISTS emails_ad;
                DROP TRIGGER IF EXISTS emails_ai;
            "#,
            )
            .await?;

        // 删除表（按依赖顺序）
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS emails_fts")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS attachments")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS emails")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS accounts")
            .await?;

        Ok(())
    }
}
