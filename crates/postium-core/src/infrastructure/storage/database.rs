//! SQLite 数据库连接与 schema 管理模块。
//!
//! 本模块提供异步的 SQLite 访问封装 [`DbConn`]，职责包括：
//!
//! - **连接管理**：基于 `tokio_rusqlite` 将所有 SQL 调度到专用线程，避免阻塞
//!   async 运行时；底层为 `Arc`，[`DbConn::clone`] 可低成本共享连接。
//! - **pragma 设置**：打开连接时启用 `foreign_keys = ON`（强制外键约束）、
//!   `busy_timeout = 5000`（写冲突时等待而非立即报错）、`journal_mode = WAL`
//!   （提升并发读写性能）。
//! - **schema 初始化**：执行 [`SCHEMA_SQL`] 创建全部表与索引；该 SQL 大量使用
//!   `IF NOT EXISTS`，因此对已有数据库是安全的幂等操作。
//! - **渐进式迁移**：历史版本遗留的 schema（如 `message_id` 上的全局唯一约束、
//!   缺失的 `(account_id, folder, uid)` 唯一索引、`sync_state` 缺少的 history
//!   列）由专用迁移函数修正。所有迁移遵循**幂等**原则——先用
//!   `PRAGMA table_info` / [`index_exists`] / [`table_exists`] 等检查当前状态，
//!   仅在确有必要时才执行，可安全反复运行。
//!
//! ## 迁移执行顺序
//!
//! [`DbConn::initialize_schema`] 中迁移与建表语句的顺序是有意安排的：
//!
//! 1. **先**执行邮件相关迁移（[`migrate_email_message_id_unique_constraint`]、
//!    [`migrate_email_uid_unique_index`]）。其中 [`migrate_email_uid_unique_index`]
//!    会先对历史重复数据去重、再建唯一索引——必须赶在 [`SCHEMA_SQL`] 里的
//!    `CREATE UNIQUE INDEX IF NOT EXISTS idx_emails_account_folder_uid` 之前，
//!    否则该建索引语句会因已存在的重复行而失败。
//! 2. **再**执行 [`SCHEMA_SQL`]，补齐当前版本的表与索引定义。
//! 3. **最后**执行 [`migrate_sync_state_history_columns`]，为旧库的 `sync_state`
//!    表 `ALTER TABLE ADD COLUMN`。放在建表之后是为了确保该表一定存在。

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

/// 打开（或创建）指定数据目录下的 `postium.sqlite` 数据库。
///
/// 会自动递归创建 `data_dir`（若不存在），并通过 [`DbConn::open_or_create`]
/// 完成 schema 初始化与迁移。返回可低成本克隆共享的 [`DbConn`]。
pub async fn init_database(data_dir: &Path) -> Result<DbConn, MailError> {
    let db_path = data_dir.join("postium.sqlite");
    std::fs::create_dir_all(data_dir).map_err(|err| MailError::DatabaseError(err.to_string()))?;

    tracing::info!("打开数据库: {}", db_path.display());
    DbConn::open_or_create(&db_path).await
}

/// 检查指定表是否已存在某列（用于迁移前的幂等判断）。
///
/// 通过 `PRAGMA table_info(<table>)` 读取该表全部列名，判断 `column` 是否在内。
/// 返回 `true` 表示该列已存在、对应迁移无需再执行。
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

/// 为 `sync_state` 表补齐“历史同步”相关的列。
///
/// **为什么迁移**：旧版 `sync_state` 仅有增量同步游标，无法记录“回溯历史邮件”
/// 所需的状态；新增列 `folder_category`、`history_synced_since`、
/// `history_before_uid`、`history_exhausted` 支持按文件夹类别拉取更早的邮件。
///
/// **如何幂等**：对每一列都先调用 [`table_has_column`] 判断，已存在则跳过
/// 该列的 `ALTER TABLE ADD COLUMN`，避免重复加列报错。新增列均允许
/// `NULL`（`history_exhausted` 默认 `0`），对历史数据无破坏。
///
/// **数据风险**：无。仅 `ADD COLUMN`，不修改或删除已有数据。
fn migrate_sync_state_history_columns(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    if !table_has_column(conn, "sync_state", "folder_category")? {
        conn.execute("ALTER TABLE sync_state ADD COLUMN folder_category TEXT", [])?;
    }
    if !table_has_column(conn, "sync_state", "history_synced_since")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_synced_since INTEGER",
            [],
        )?;
    }
    if !table_has_column(conn, "sync_state", "history_before_uid")? {
        conn.execute(
            "ALTER TABLE sync_state ADD COLUMN history_before_uid INTEGER",
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

/// 检查指定名称的索引是否存在（用于迁移前的幂等判断）。
///
/// 查询 `sqlite_master` 中 `type='index'` 且名称匹配的记录，行数 > 0 即存在。
/// 仅按名称判断，不区分唯一/普通索引。
fn index_exists(conn: &rusqlite::Connection, index_name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
        [index_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

/// 检查指定名称的表是否存在（用于迁移前的幂等判断）。
///
/// 查询 `sqlite_master` 中 `type='table'` 且名称匹配的记录，行数 > 0 即存在。
fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

/// 检查指定索引的列集合是否与 `expected_columns` 严格一致（顺序、个数均匹配）。
///
/// 通过 `PRAGMA index_info(<index>)` 读取该索引的全部列名（按索引内定义顺序），
/// 与 `expected_columns` 逐位比较。用于精确识别某个旧约束，避免误判列名相同但
/// 范围不同（如单列 `message_id` 与多列复合索引）的索引。
///
/// 注意：本函数**不**校验该索引是否为 UNIQUE（unique 由调用方在
/// [`email_message_id_has_unique_index`] 中另行判断）。
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

/// 检查 `emails` 表是否存在“仅以 `message_id` 为列的唯一索引”。
///
/// 这是旧 schema 的遗留约束：早期版本对 `message_id` 加了全局唯一约束，
/// 但同一封邮件可能同时存在于多个文件夹（收件箱、已发送、归档……），其
/// `message_id` 相同，导致重复入库失败。本函数即用于检测该约束是否仍在。
///
/// 实现：先确认 `emails` 表存在；再用 `PRAGMA index_list(emails)` 枚举其全部
/// 索引，对其中标记为 unique（第 3 列 = 1）的索引，调用
/// [`unique_index_has_exact_columns`] 精确判断其列是否恰好为 `["message_id"]`。
/// 命中即返回 `true`。
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

/// 移除 `emails.message_id` 上的全局唯一约束（仅当它仍存在时）。
///
/// **为什么迁移**：旧版对 `message_id` 设有唯一约束，而同一封邮件常按文件夹
/// 多处存储，导致除首份外其余入库全部失败。正确做法是以
/// `(account_id, folder, uid)` 作为唯一键（见 [`migrate_email_uid_unique_index`]），
/// `message_id` 不应全局唯一。
///
/// **如何幂等**：先由 [`email_message_id_has_unique_index`] 判断约束是否存在，
/// 不存在则直接返回，避免对已迁移的库重复操作。
///
/// **如何改**：SQLite 无法直接 `DROP CONSTRAINT`，故采用“重建表”手法——
/// 新建一个不含该唯一约束的 `emails_without_message_id_unique` 表，将原表
/// 数据全量复制过去，再 `DROP TABLE emails` 并把新表 `RENAME` 回 `emails`。
/// 整个过程包在事务内。
///
/// **外键处理**：重建表期间临时关闭 `foreign_keys`（`PRAGMA foreign_keys=OFF`），
/// 以免删除/重命名 `emails` 时触发外键级联或校验失败；迁移结束后按原值恢复。
///
/// **数据风险**：迁移本身不丢弃或合并任何邮件行（仅复制 schema 不变的全列）；
/// 若原表内存在 `message_id` 重复的行，本迁移不做去重（去重交由
/// [`migrate_email_uid_unique_index`] 负责），故不会丢失数据。
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

/// 建立 `(account_id, folder, uid)` 唯一索引，并在此前清理历史重复数据。
///
/// **为什么迁移**：旧版缺少该唯一索引，同一 `(account_id, folder, uid)` 可能
/// 入库多份；当前版本以该三元组作为邮件的稳定唯一键，必须先去重才能建唯一
/// 索引，否则 `CREATE UNIQUE INDEX` 会因重复行失败。
///
/// **如何幂等**：若 [`index_exists`] 表明索引已存在，则视为已迁移完成、直接返回；
/// 若 `emails` 表尚不存在（极早期空库）也直接返回。
///
/// **去重策略（会删除数据，需重点理解）**：对每组重复的 `(account_id, folder, uid)`
/// 按以下优先级选出**要保留**的那一行，删除组内其余所有行：
/// 1. 优先保留**已下载正文**的行（`body_text` 或 `body_html` 非空）——
///    正文是稀缺资源，丢弃代价高；
/// 2. 其次保留 `updated_at` 最新者；
/// 3. 再者保留 `id` 最大者（最新写入）。
///
/// 注意 SQL 中“保留行”的子查询与“删除”的 JOIN 条件对应同一三元组，并通过
/// `kept.id != duplicate.id` 保证不会误删被保留的那一行。整个去重 + 建索引
/// 包在同一事务内，要么全部成功、要么全部回滚。
///
/// **数据风险**：会**永久删除**每组重复中除被保留行外的其余行（无法撤销）。
/// 这是设计上的必要取舍——只有删除重复才能建立唯一约束。
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
