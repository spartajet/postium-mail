use std::path::Path;

use crate::error::MailError;

pub const SCHEMA_SQL: &str = include_str!("../../../sql/schema.sql");

/// 异步 SQLite 数据库连接封装
///
/// 基于 `tokio_rusqlite`，将所有 SQL 操作调度到独立的专用线程执行，
/// 避免阻塞 async 运行时。内部启用外键约束、WAL 模式和忙等待超时。
/// 通过 `clone()` 可低成本地共享连接（底层是 `Arc`）。
#[derive(Clone)]
pub struct DbConn {
    inner: tokio_rusqlite::Connection,
}

impl DbConn {
    /// 打开或创建指定路径的数据库文件，并初始化表结构
    pub async fn open_or_create(db_path: &Path) -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open(db_path).await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// 创建内存数据库（仅用于测试）
    pub async fn open_in_memory_for_test() -> Result<Self, MailError> {
        let conn = tokio_rusqlite::Connection::open_in_memory().await?;
        let db = Self { inner: conn };
        db.initialize_schema().await?;
        Ok(db)
    }

    /// 初始化数据库 pragma 设置和表结构
    async fn initialize_schema(&self) -> Result<(), MailError> {
        self.call(|conn| {
            conn.pragma_update(None, "foreign_keys", "ON")?;
            conn.pragma_update(None, "busy_timeout", 5000)?;
            conn.pragma_update(None, "journal_mode", "WAL")?;
            migrate_email_message_id_unique_constraint(conn)?;
            migrate_email_uid_unique_index(conn)?;
            conn.execute_batch(SCHEMA_SQL)?;
            migrate_sync_state_history_columns(conn)?;
            Ok(())
        })
        .await
    }

    /// 在专用线程中执行单次数据库操作
    ///
    /// `f` 接收一个可变的 `rusqlite::Connection`，操作自动串行化。
    pub async fn call<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        Ok(self.inner.call(f).await?)
    }

    /// 在事务中执行数据库操作
    ///
    /// 操作成功时自动提交，失败时自动回滚。
    pub async fn transaction<F, R>(&self, f: F) -> Result<R, MailError>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> rusqlite::Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.call(move |conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
        .await
    }
}

pub async fn init_database(data_dir: &Path) -> Result<DbConn, MailError> {
    let db_path = data_dir.join("postium.sqlite");
    std::fs::create_dir_all(data_dir).map_err(|err| MailError::DatabaseError(err.to_string()))?;

    tracing::info!("打开数据库: {}", db_path.display());
    DbConn::open_or_create(&db_path).await
}

fn table_has_column(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.iter().any(|name| name == column))
}

fn migrate_sync_state_history_columns(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    if !table_has_column(conn, "sync_state", "history_synced_since")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_synced_since INTEGER",
            [],
        )?;
    }
    if !table_has_column(conn, "sync_state", "history_exhausted")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_exhausted INTEGER DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

fn index_exists(conn: &rusqlite::Connection, index_name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [index_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

fn unique_index_has_exact_columns(
    conn: &rusqlite::Connection,
    index_name: &str,
    expected_columns: &[&str],
) -> rusqlite::Result<bool> {
    let escaped_index_name = index_name.replace('\'', "''");
    let mut stmt = conn.prepare(&format!("PRAGMA index_info('{escaped_index_name}')"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(2))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns.len() == expected_columns.len()
        && columns
            .iter()
            .zip(expected_columns.iter())
            .all(|(actual, expected)| actual == expected))
}

fn email_message_id_has_unique_index(conn: &rusqlite::Connection) -> rusqlite::Result<bool> {
    if !table_exists(conn, "emails")? {
        return Ok(false);
    }

    let mut stmt = conn.prepare("PRAGMA index_list(emails)")?;
    let indexes = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? == 1))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    for (index_name, is_unique) in indexes {
        if is_unique && unique_index_has_exact_columns(conn, &index_name, &["message_id"])? {
            return Ok(true);
        }
    }

    Ok(false)
}

fn migrate_email_message_id_unique_constraint(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<()> {
    if !email_message_id_has_unique_index(conn)? {
        return Ok(());
    }

    let foreign_keys_enabled =
        conn.query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))? != 0;
    conn.pragma_update(None, "foreign_keys", "OFF")?;

    let result = (|| {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "
            CREATE TABLE emails_without_message_id_unique (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
                folder TEXT NOT NULL,
                uid INTEGER NOT NULL,
                message_id TEXT,
                subject TEXT,
                sender_name TEXT,
                sender_email TEXT NOT NULL,
                recipient_emails TEXT NOT NULL,
                cc_emails TEXT,
                bcc_emails TEXT,
                preview TEXT,
                body_text TEXT,
                body_html TEXT,
                is_read INTEGER DEFAULT 0,
                is_starred INTEGER DEFAULT 0,
                is_draft INTEGER DEFAULT 0,
                is_answered INTEGER DEFAULT 0,
                is_deleted INTEGER DEFAULT 0,
                sent_at INTEGER NOT NULL,
                received_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            INSERT INTO emails_without_message_id_unique (
                id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            )
            SELECT
                id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                is_read, is_starred, is_draft, is_answered, is_deleted,
                sent_at, received_at, created_at, updated_at
            FROM emails;
            DROP TABLE emails;
            ALTER TABLE emails_without_message_id_unique RENAME TO emails;
            ",
        )?;
        tx.commit()
    })();

    if foreign_keys_enabled {
        conn.pragma_update(None, "foreign_keys", "ON")?;
    }

    result
}

fn migrate_email_uid_unique_index(conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    if index_exists(conn, "idx_emails_account_folder_uid")? {
        return Ok(());
    }
    if !table_exists(conn, "emails")? {
        return Ok(());
    }

    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM emails
         WHERE id IN (
             SELECT duplicate.id
             FROM emails duplicate
             JOIN emails kept
               ON kept.account_id = duplicate.account_id
              AND kept.folder = duplicate.folder
              AND kept.uid = duplicate.uid
              AND kept.id != duplicate.id
             WHERE kept.id = (
                 SELECT candidate.id
                 FROM emails candidate
                 WHERE candidate.account_id = duplicate.account_id
                   AND candidate.folder = duplicate.folder
                   AND candidate.uid = duplicate.uid
                 ORDER BY
                   CASE
                     WHEN candidate.body_text IS NOT NULL OR candidate.body_html IS NOT NULL THEN 0
                     ELSE 1
                   END,
                   candidate.updated_at DESC,
                   candidate.id DESC
                 LIMIT 1
             )
             AND duplicate.id != kept.id
         )",
        [],
    )?;
    tx.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid
         ON emails(account_id, folder, uid)",
        [],
    )?;
    tx.commit()
}
