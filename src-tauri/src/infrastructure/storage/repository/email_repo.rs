//! 邮件仓库模块（Email Repository）
//!
//! 负责 `emails` 表的数据访问，是存储层中最大的仓库模块。
//! 涵盖邮件的完整生命周期管理：增删改查、状态变更、批量同步、文件夹统计等。
//!
//! 主要操作：
//! - 查询：按文件夹/多文件夹/星标查询、按 ID/UID 查询单条、文件夹统计
//! - 写入：单条创建、批量创建（邮件头/完整邮件）、更新
//! - 状态管理：标记已读、切换星标、软删除、移动文件夹、更新正文
//! - 删除：软删除、按文件夹删除、级联删除文件夹内容、按账号删除

use crate::domain::sync::FolderStat;
use crate::error::MailError;
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use crate::infrastructure::protocols::utils::{
    extract_email_from_address, extract_name_from_address, serialize_addresses,
};
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::emails;
use crate::infrastructure::storage::repository::attachment_repo::{
    AttachmentWrite, INSERT_ATTACHMENT_SQL, execute_attachment_insert,
};
use crate::infrastructure::storage::row::{bool_to_int, opt_bool_to_int, opt_int_to_bool};
use rusqlite::types::Value;

/// 批量插入邮件时使用的 SQL 语句常量
const INSERT_EMAIL_SQL: &str = "INSERT INTO emails (
        account_id, folder, uid, message_id, subject, sender_name, sender_email,
        recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
        is_read, is_starred, is_draft, is_answered, is_deleted,
        sent_at, received_at, created_at, updated_at
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
        ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
     )";

const INSERT_EMAIL_HEADER_SQL: &str = "INSERT INTO emails (
        account_id, folder, uid, message_id, subject, sender_name, sender_email,
        recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
        is_read, is_starred, is_draft, is_answered, is_deleted,
        sent_at, received_at, created_at, updated_at
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
        ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
     )
     ON CONFLICT(account_id, folder, uid) DO NOTHING";

const UPSERT_EMAIL_SQL: &str = "INSERT INTO emails (
        account_id, folder, uid, message_id, subject, sender_name, sender_email,
        recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
        is_read, is_starred, is_draft, is_answered, is_deleted,
        sent_at, received_at, created_at, updated_at
     ) VALUES (
        ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
        ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
     )
     ON CONFLICT(account_id, folder, uid) DO UPDATE SET
        message_id = COALESCE(excluded.message_id, emails.message_id),
        subject = excluded.subject,
        sender_name = excluded.sender_name,
        sender_email = excluded.sender_email,
        recipient_emails = excluded.recipient_emails,
        cc_emails = excluded.cc_emails,
        bcc_emails = excluded.bcc_emails,
        preview = excluded.preview,
        body_text = excluded.body_text,
        body_html = excluded.body_html,
        is_read = excluded.is_read,
        is_starred = excluded.is_starred,
        is_draft = excluded.is_draft,
        is_answered = excluded.is_answered,
        is_deleted = excluded.is_deleted,
        sent_at = excluded.sent_at,
        received_at = excluded.received_at,
        updated_at = excluded.updated_at";

/// 用于创建或更新邮件的写入数据结构
///
/// 不包含自增主键 `id`，其余字段与 `emails` 表一一对应。
/// 布尔类标记字段使用 `Option<bool>`，序列化时转换为 0/1 整数存储。
#[derive(Clone, Debug)]
pub struct EmailWrite {
    pub account_id: i32,
    pub folder: String,
    pub uid: u32,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub bcc_emails: Option<String>,
    pub preview: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub is_draft: Option<bool>,
    pub is_answered: Option<bool>,
    pub is_deleted: Option<bool>,
    pub sent_at: i64,
    pub received_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 将数据库行映射为 `emails::Model`
///
/// 同时将整数形式的布尔标记字段转换为 `Option<bool>`，
/// 并将 `uid` 从 `i64` 转换为 `u32`。
fn map_email(row: &rusqlite::Row<'_>) -> rusqlite::Result<emails::Model> {
    Ok(emails::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        folder: row.get("folder")?,
        uid: row.get::<_, i64>("uid")? as u32,
        message_id: row.get("message_id")?,
        subject: row.get("subject")?,
        sender_name: row.get("sender_name")?,
        sender_email: row.get("sender_email")?,
        recipient_emails: row.get("recipient_emails")?,
        cc_emails: row.get("cc_emails")?,
        bcc_emails: row.get("bcc_emails")?,
        preview: row.get("preview")?,
        body_text: row.get("body_text")?,
        body_html: row.get("body_html")?,
        is_read: opt_int_to_bool(row.get("is_read")?),
        is_starred: opt_int_to_bool(row.get("is_starred")?),
        is_draft: opt_int_to_bool(row.get("is_draft")?),
        is_answered: opt_int_to_bool(row.get("is_answered")?),
        is_deleted: opt_int_to_bool(row.get("is_deleted")?),
        sent_at: row.get("sent_at")?,
        received_at: row.get("received_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// 生成指定数量的 SQL 占位符字符串，如 `?,?,?`
///
/// 用于动态构建 `IN (...)` 子句。
fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

/// 在预编译语句上执行单次邮件插入。
///
/// 供批量插入场景复用预编译语句，避免重复 prepare 带来的性能开销。
///
/// # 参数
///
/// - `stmt`: 已预编译的插入语句
/// - `model`: 待插入的邮件数据
fn execute_email_insert(
    stmt: &mut rusqlite::Statement<'_>,
    model: &EmailWrite,
) -> rusqlite::Result<usize> {
    stmt.execute(rusqlite::params![
        model.account_id,
        &model.folder,
        i64::from(model.uid),
        &model.message_id,
        &model.subject,
        &model.sender_name,
        &model.sender_email,
        &model.recipient_emails,
        &model.cc_emails,
        &model.bcc_emails,
        &model.preview,
        &model.body_text,
        &model.body_html,
        opt_bool_to_int(model.is_read),
        opt_bool_to_int(model.is_starred),
        opt_bool_to_int(model.is_draft),
        opt_bool_to_int(model.is_answered),
        opt_bool_to_int(model.is_deleted),
        model.sent_at,
        model.received_at,
        model.created_at,
        model.updated_at,
    ])
}

/// 按文件夹分页查询邮件。
///
/// 返回未软删除的邮件，按发送时间降序排列，同时返回符合条件的总数。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `page`: 页码（从 1 开始）
/// - `limit`: 每页数量
///
/// # 返回
///
/// `(邮件列表, 总数)` 元组。
pub async fn list_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let total = conn.query_row(
            "SELECT COUNT(*)
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND is_deleted = 0",
            rusqlite::params![account_id, &folder],
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND is_deleted = 0
             ORDER BY sent_at DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![account_id, folder, limit as i64, offset],
                map_email,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
}

/// 按多个文件夹查询邮件（用于 category → 多文件夹映射）。
///
/// 用于将一个分类映射到多个 IMAP 文件夹的场景，返回未软删除的邮件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folders`: 文件夹名称列表
/// - `page`: 页码（从 1 开始）
/// - `limit`: 每页数量
///
/// # 返回
///
/// `(邮件列表, 总数)` 元组。空文件夹列表时返回 `(空vec, 0)`。
pub async fn list_by_folders(
    db: &DbConn,
    account_id: i32,
    folders: &[String],
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    if folders.is_empty() {
        return Ok((Vec::new(), 0));
    }

    let folders = folders.to_vec();
    db.call(move |conn| {
        let in_clause = placeholders(folders.len());
        let mut count_values = Vec::with_capacity(folders.len() + 1);
        count_values.push(Value::from(account_id));
        count_values.extend(folders.iter().cloned().map(Value::from));

        let count_sql = format!(
            "SELECT COUNT(*)
             FROM emails
             WHERE account_id = ? AND folder IN ({in_clause}) AND is_deleted = 0"
        );
        let total = conn.query_row(
            &count_sql,
            rusqlite::params_from_iter(count_values.iter()),
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let select_sql = format!(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE account_id = ? AND folder IN ({in_clause}) AND is_deleted = 0
             ORDER BY sent_at DESC
             LIMIT ? OFFSET ?"
        );
        let mut select_values = count_values;
        select_values.push(Value::from(limit as i64));
        select_values.push(Value::from(offset));
        let mut stmt = conn.prepare(&select_sql)?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(select_values.iter()), map_email)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
}

/// 查询星标邮件（跨所有文件夹）。
///
/// 返回 `is_starred = 1` 且未软删除的邮件，按发送时间降序分页排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `page`: 页码（从 1 开始）
/// - `limit`: 每页数量
///
/// # 返回
///
/// `(邮件列表, 总数)` 元组。
pub async fn list_starred(
    db: &DbConn,
    account_id: i32,
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    db.call(move |conn| {
        let total = conn.query_row(
            "SELECT COUNT(*)
             FROM emails
             WHERE account_id = ?1 AND is_starred = 1 AND is_deleted = 0",
            [account_id],
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE account_id = ?1 AND is_starred = 1 AND is_deleted = 0
             ORDER BY sent_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = stmt
            .query_map(
                rusqlite::params![account_id, limit as i64, offset],
                map_email,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((items, total))
    })
    .await
}

/// 根据 ID 查询邮件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 邮件 ID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<emails::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        match stmt.query_row([id], map_email) {
            Ok(email) => Ok(Some(email)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 根据账号 ID、文件夹和 UID 查询邮件。
///
/// 用于 IMAP 同步时按唯一标识定位邮件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `uid`: IMAP UID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
pub async fn get_by_uid(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: u32,
) -> Result<Option<emails::Model>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND uid = ?3",
        )?;
        match stmt.query_row(
            rusqlite::params![account_id, folder, i64::from(uid)],
            map_email,
        ) {
            Ok(email) => Ok(Some(email)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 创建单封邮件并返回创建后的完整记录。
///
/// 在事务中执行插入并回读记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `model`: 待创建的邮件数据
///
/// # 返回
///
/// 创建成功后的邮件记录（包含自增 ID）。
pub async fn create(db: &DbConn, model: EmailWrite) -> Result<emails::Model, MailError> {
    db.transaction(move |tx| {
        let mut insert_stmt = tx.prepare(INSERT_EMAIL_SQL)?;
        execute_email_insert(&mut insert_stmt, &model)?;
        let id = tx.last_insert_rowid() as i32;
        let mut stmt = tx.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_email)
    })
    .await
}

/// 批量插入邮件。
///
/// 在单个事务中复用预编译语句逐条插入，空列表直接返回。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `models`: 待插入的邮件列表
///
/// # 返回
///
/// 插入成功返回 `Ok(())`。
pub async fn bulk_insert(db: &DbConn, models: Vec<EmailWrite>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }

    db.transaction(move |tx| {
        let mut stmt = tx.prepare(INSERT_EMAIL_SQL)?;
        for model in &models {
            execute_email_insert(&mut stmt, model)?;
        }
        Ok(())
    })
    .await
}

/// 根据 ID 更新邮件，返回更新后的完整记录。
///
/// 覆盖更新所有字段，调用方需提供完整的 `EmailWrite` 数据。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 待更新的邮件 ID
/// - `model`: 新的邮件数据
///
/// # 返回
///
/// 更新后的邮件记录。
pub async fn update(db: &DbConn, id: i32, model: EmailWrite) -> Result<emails::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE emails
             SET account_id = ?1, folder = ?2, uid = ?3, message_id = ?4, subject = ?5,
                 sender_name = ?6, sender_email = ?7, recipient_emails = ?8,
                 cc_emails = ?9, bcc_emails = ?10, preview = ?11, body_text = ?12,
                 body_html = ?13, is_read = ?14, is_starred = ?15, is_draft = ?16,
                 is_answered = ?17, is_deleted = ?18, sent_at = ?19, received_at = ?20,
                 created_at = ?21, updated_at = ?22
             WHERE id = ?23",
            rusqlite::params![
                model.account_id,
                model.folder,
                i64::from(model.uid),
                model.message_id,
                model.subject,
                model.sender_name,
                model.sender_email,
                model.recipient_emails,
                model.cc_emails,
                model.bcc_emails,
                model.preview,
                model.body_text,
                model.body_html,
                opt_bool_to_int(model.is_read),
                opt_bool_to_int(model.is_starred),
                opt_bool_to_int(model.is_draft),
                opt_bool_to_int(model.is_answered),
                opt_bool_to_int(model.is_deleted),
                model.sent_at,
                model.received_at,
                model.created_at,
                model.updated_at,
                id,
            ],
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_email)
    })
    .await
}

/// 更新邮件的已读状态。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 邮件 ID
/// - `is_read`: 是否已读
///
/// # 返回
///
/// 更新成功返回 `Ok(())`。
pub async fn mark_as_read(db: &DbConn, id: i32, is_read: bool) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE emails SET is_read = ?1 WHERE id = ?2",
            rusqlite::params![bool_to_int(is_read), id],
        )?;
        Ok(())
    })
    .await
}

/// 切换邮件的星标状态，返回切换后的新状态。
///
/// 先读取当前状态取反，再写回数据库。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 邮件 ID
///
/// # 返回
///
/// 切换后的星标状态（`true` 表示已星标）。邮件不存在时返回 `EmailNotFound` 错误。
pub async fn toggle_star(db: &DbConn, id: i32) -> Result<bool, MailError> {
    let new_state = db
        .call(move |conn| {
            let current =
                match conn.query_row("SELECT is_starred FROM emails WHERE id = ?1", [id], |row| {
                    row.get::<_, Option<i64>>(0)
                }) {
                    Ok(value) => Some(value),
                    Err(rusqlite::Error::QueryReturnedNoRows) => None,
                    Err(err) => return Err(err),
                };
            let Some(current) = current else {
                return Ok(None);
            };
            let new_state = !opt_int_to_bool(current).unwrap_or(false);
            conn.execute(
                "UPDATE emails SET is_starred = ?1 WHERE id = ?2",
                rusqlite::params![bool_to_int(new_state), id],
            )?;
            Ok(Some(new_state))
        })
        .await?;

    new_state.ok_or(MailError::EmailNotFound(id))
}

/// 软删除多封邮件（标记 `is_deleted = 1`）。
///
/// 批量将指定邮件标记为已删除，不真正从数据库移除。空列表直接返回。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `ids`: 待软删除的邮件 ID 列表
///
/// # 返回
///
/// 受影响的行数。
pub async fn soft_delete(db: &DbConn, ids: Vec<i32>) -> Result<usize, MailError> {
    if ids.is_empty() {
        return Ok(0);
    }

    db.call(move |conn| {
        let sql = format!(
            "UPDATE emails
             SET is_deleted = 1
             WHERE id IN ({})",
            placeholders(ids.len())
        );
        let mut values = Vec::with_capacity(ids.len());
        values.extend(ids.into_iter().map(Value::from));
        conn.execute(&sql, rusqlite::params_from_iter(values.iter()))
    })
    .await
}

/// 将邮件移动到指定文件夹。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 邮件 ID
/// - `folder`: 目标文件夹名称
///
/// # 返回
///
/// 移动成功返回 `Ok(())`。
pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<(), MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        conn.execute(
            "UPDATE emails SET folder = ?1 WHERE id = ?2",
            rusqlite::params![folder, id],
        )?;
        Ok(())
    })
    .await
}

/// 按 account_id 聚合文件夹统计（单条 SQL，替代 N+1 查询）。
///
/// 返回每个文件夹的总邮件数和未读邮件数，不含已软删除的邮件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 每个文件夹的 `FolderStat`（文件夹名、总数、未读数）列表。
pub async fn folder_stats_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<FolderStat>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT folder,
                    COUNT(*) as total,
                    SUM(CASE WHEN is_read = 0 OR is_read IS NULL THEN 1 ELSE 0 END) as unread
             FROM emails
             WHERE account_id = ?1 AND (is_deleted = 0 OR is_deleted IS NULL)
             GROUP BY folder",
        )?;
        let rows = stmt.query_map([account_id], |row| {
            Ok(FolderStat {
                folder: row.get("folder")?,
                total: row.get::<_, i64>("total")? as usize,
                unread: row.get::<_, i64>("unread")? as usize,
            })
        })?;
        rows.collect()
    })
    .await
}

/// 根据账号和文件夹获取邮件 ID。
///
/// 返回该账号指定文件夹下所有邮件的 ID，按 ID 升序排列（含已软删除的邮件）。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
///
/// # 返回
///
/// 邮件 ID 列表。
pub async fn get_ids_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Vec<i32>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id
             FROM emails
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY id ASC",
        )?;
        stmt.query_map(rusqlite::params![account_id, folder], |row| row.get("id"))?
            .collect()
    })
    .await
}

/// 根据账号和文件夹删除邮件。
///
/// 物理删除该账号指定文件夹下的所有邮件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
///
/// # 返回
///
/// 被删除的行数。
pub async fn delete_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<u64, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let deleted = conn.execute(
            "DELETE FROM emails WHERE account_id = ?1 AND folder = ?2",
            rusqlite::params![account_id, folder],
        )?;
        Ok(deleted as u64)
    })
    .await
}

/// 级联删除指定文件夹的所有邮件及其关联数据。
///
/// 在事务中依次删除 `email_labels` 关联、`attachments` 附件，最后删除邮件本身。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
///
/// # 返回
///
/// 被删除的邮件行数。
pub async fn delete_folder_contents(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<u64, MailError> {
    let folder = folder.to_string();
    db.transaction(move |tx| {
        tx.execute(
            "DELETE FROM email_labels
             WHERE email_id IN (
                SELECT id FROM emails WHERE account_id = ?1 AND folder = ?2
             )",
            rusqlite::params![account_id, &folder],
        )?;
        tx.execute(
            "DELETE FROM attachments
             WHERE email_id IN (
                SELECT id FROM emails WHERE account_id = ?1 AND folder = ?2
             )",
            rusqlite::params![account_id, &folder],
        )?;
        let deleted = tx.execute(
            "DELETE FROM emails WHERE account_id = ?1 AND folder = ?2",
            rusqlite::params![account_id, &folder],
        )?;
        Ok(deleted as u64)
    })
    .await
}

/// 删除指定账号的所有邮件及其附件。
///
/// 委托给 [`delete_by_account_tx`] 在事务中执行。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 被删除的邮件行数。
pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<u64, MailError> {
    db.transaction(move |tx| delete_by_account_tx(tx, account_id))
        .await
}

/// 在指定事务中删除账号的所有邮件及其附件（事务版本）
///
/// 先删除 `attachments`，再删除 `emails`。供需要事务性删除的调用方使用。
///
/// # 参数
///
/// - `tx`: 数据库事务
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 被删除的邮件行数。
pub fn delete_by_account_tx(
    tx: &rusqlite::Transaction<'_>,
    account_id: i32,
) -> rusqlite::Result<u64> {
    tx.execute(
        "DELETE FROM attachments
         WHERE email_id IN (SELECT id FROM emails WHERE account_id = ?1)",
        [account_id],
    )?;
    let deleted = tx.execute("DELETE FROM emails WHERE account_id = ?1", [account_id])?;
    Ok(deleted as u64)
}

/// 批量保存邮件头。
///
/// 从 IMAP 同步的邮件头批量保存到数据库。
/// 这个方法主要用于快速同步邮件列表，不包含邮件正文（body_text/body_html）。
///
/// 将 `EmailHeader` 转换为 `EmailWrite`，在单个事务中批量插入邮件和附件元数据，
/// 附件 `email_id` 在插入邮件后通过 `last_insert_rowid` 填充。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `headers`: 从 IMAP 同步的邮件头列表
///
/// # 返回
///
/// 成功保存的邮件数量。空列表时返回 0。
pub async fn save_batch_email_headers(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    headers: &[EmailHeader],
) -> Result<usize, MailError> {
    if headers.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp();
    let folder = folder.to_string();
    let writes: Vec<(EmailWrite, Vec<AttachmentWrite>)> = headers
        .iter()
        .map(|header| {
            let email = EmailWrite {
                account_id,
                folder: folder.clone(),
                uid: header.uid,
                subject: Some(header.subject.clone()),
                sender_name: extract_name_from_address(&header.from),
                sender_email: extract_email_from_address(&header.from),
                recipient_emails: serialize_addresses(&header.to),
                cc_emails: if header.cc.is_empty() {
                    None
                } else {
                    Some(serialize_addresses(&header.cc))
                },
                bcc_emails: None,
                body_text: None,
                body_html: None,
                message_id: None,
                preview: None,
                is_read: Some(header.flags.seen),
                is_starred: Some(header.flags.flagged),
                is_draft: Some(header.flags.draft),
                is_answered: Some(header.flags.answered),
                is_deleted: Some(header.flags.deleted),
                sent_at: header.date.timestamp(),
                received_at: header.date.timestamp(),
                created_at: now,
                updated_at: now,
            };
            let attachments = header
                .attachments
                .iter()
                .map(|att| AttachmentWrite {
                    email_id: 0,
                    filename: att.filename.clone(),
                    content_type: Some(att.content_type.clone()),
                    size: att.size as i64,
                    section_path: att.section_path.clone(),
                    disposition: att.disposition.clone(),
                    content_id: att.content_id.clone(),
                    path: None,
                    created_at: now,
                })
                .collect();
            (email, attachments)
        })
        .collect();

    db.transaction(move |tx| {
        let mut email_stmt = tx.prepare(INSERT_EMAIL_HEADER_SQL)?;
        let mut attachment_stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        let mut inserted_count = 0;
        for (email, attachments) in &writes {
            let inserted = execute_email_insert(&mut email_stmt, email)?;
            if inserted == 0 {
                continue;
            }
            inserted_count += 1;
            let email_id = tx.last_insert_rowid() as i32;
            for attachment in attachments {
                let mut attachment = attachment.clone();
                attachment.email_id = email_id;
                execute_attachment_insert(&mut attachment_stmt, &attachment)?;
            }
        }
        Ok(inserted_count)
    })
    .await
}

/// 批量保存完整邮件（含正文和附件）。
///
/// 将 `WholeEmailDto` 转换为 `EmailWrite`，在单个事务中批量插入邮件和附件。
/// 与 [`save_batch_email_headers`] 不同，本方法包含完整的邮件正文、preview、message_id 等字段。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `emails`: 待保存的完整邮件列表
///
/// # 返回
///
/// 成功插入或更新的邮件行数。空列表时返回 0。
pub async fn save_batch_emails(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    emails: &[WholeEmailDto],
) -> Result<usize, MailError> {
    if emails.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp();
    let folder = folder.to_string();
    let writes: Vec<(EmailWrite, Vec<AttachmentWrite>)> = emails
        .iter()
        .map(|email| {
            let write = EmailWrite {
                account_id,
                folder: folder.clone(),
                uid: email.uid,
                message_id: email.message_id.clone(),
                subject: email.subject.clone(),
                sender_name: email.sender_name.clone(),
                sender_email: email.sender_email.clone(),
                recipient_emails: email.recipient_emails.clone(),
                cc_emails: email.cc_emails.clone(),
                bcc_emails: email.bcc_emails.clone(),
                preview: email.preview.clone(),
                body_text: email.body_text.clone(),
                body_html: email.body_html.clone(),
                is_read: Some(email.is_read),
                is_starred: Some(email.is_starred),
                is_draft: Some(email.is_draft),
                is_answered: Some(email.is_answered),
                is_deleted: Some(email.is_deleted),
                sent_at: email.sent_at,
                received_at: email.received_at,
                created_at: now,
                updated_at: now,
            };
            let attachments = email
                .attachments
                .iter()
                .map(|att| AttachmentWrite {
                    email_id: 0,
                    filename: att.filename.clone(),
                    content_type: Some(att.content_type.clone()),
                    size: att.size as i64,
                    section_path: att.section_path.clone(),
                    disposition: att.disposition.clone(),
                    content_id: att.content_id.clone(),
                    path: None,
                    created_at: now,
                })
                .collect();
            (write, attachments)
        })
        .collect();

    db.transaction(move |tx| {
        let mut email_stmt = tx.prepare(UPSERT_EMAIL_SQL)?;
        let mut attachment_stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        let mut affected_rows = 0;
        for (email, attachments) in &writes {
            affected_rows += execute_email_insert(&mut email_stmt, email)?;
            let email_id = tx.query_row(
                "SELECT id FROM emails WHERE account_id = ?1 AND folder = ?2 AND uid = ?3",
                rusqlite::params![email.account_id, &email.folder, i64::from(email.uid)],
                |row| row.get::<_, i32>(0),
            )?;
            tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
            for attachment in attachments {
                let mut attachment = attachment.clone();
                attachment.email_id = email_id;
                execute_attachment_insert(&mut attachment_stmt, &attachment)?;
            }
        }
        Ok(affected_rows)
    })
    .await
}

/// 查询指定账号文件夹中最早一封未删除邮件的发送时间。
pub async fn earliest_sent_at_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<i64>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        match conn.query_row(
            "SELECT MIN(sent_at)
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND is_deleted = 0",
            rusqlite::params![account_id, folder],
            |row| row.get::<_, Option<i64>>(0),
        ) {
            Ok(value) => Ok(value),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 查询指定账号文件夹中最小的未删除邮件 UID。
pub async fn min_uid_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<u32>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        match conn.query_row(
            "SELECT MIN(uid)
             FROM emails
             WHERE account_id = ?1 AND folder = ?2 AND is_deleted = 0",
            rusqlite::params![account_id, folder],
            |row| row.get::<_, Option<i64>>(0),
        ) {
            Ok(value) => Ok(value.map(|uid| uid as u32)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 查询账号本地邮件中实际存在的未删除文件夹名称。
pub async fn distinct_folders_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<String>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT folder
             FROM emails
             WHERE account_id = ?1 AND is_deleted = 0
             ORDER BY folder",
        )?;
        let folders = stmt
            .query_map([account_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(folders)
    })
    .await
}

/// 用一封完整邮件替换本地已有邮件及其附件元数据。
///
/// 在同一个事务中覆盖 `emails` 记录、删除旧附件数据库元数据并插入新的附件元数据。
/// 不处理磁盘上的附件文件。
pub async fn replace_email_with_attachments(
    db: &DbConn,
    email_id: i32,
    account_id: i32,
    folder: &str,
    email: WholeEmailDto,
) -> Result<emails::Model, MailError> {
    let now = chrono::Utc::now().timestamp();
    let folder = folder.to_string();
    let write = EmailWrite {
        account_id,
        folder: folder.clone(),
        uid: email.uid,
        message_id: email.message_id,
        subject: email.subject,
        sender_name: email.sender_name,
        sender_email: email.sender_email,
        recipient_emails: email.recipient_emails,
        cc_emails: email.cc_emails,
        bcc_emails: email.bcc_emails,
        preview: email.preview,
        body_text: email.body_text,
        body_html: email.body_html,
        is_read: Some(email.is_read),
        is_starred: Some(email.is_starred),
        is_draft: Some(email.is_draft),
        is_answered: Some(email.is_answered),
        is_deleted: Some(email.is_deleted),
        sent_at: email.sent_at,
        received_at: email.received_at,
        created_at: now,
        updated_at: now,
    };
    let attachments: Vec<AttachmentWrite> = email
        .attachments
        .into_iter()
        .map(|att| AttachmentWrite {
            email_id,
            filename: att.filename,
            content_type: Some(att.content_type),
            size: i64::from(att.size),
            section_path: att.section_path,
            disposition: att.disposition,
            content_id: att.content_id,
            path: None,
            created_at: now,
        })
        .collect();

    db.transaction(move |tx| {
        tx.execute(
            "UPDATE emails
             SET account_id = ?1, folder = ?2, uid = ?3, message_id = ?4, subject = ?5,
                 sender_name = ?6, sender_email = ?7, recipient_emails = ?8,
                 cc_emails = ?9, bcc_emails = ?10, preview = ?11, body_text = ?12,
                 body_html = ?13, is_read = ?14, is_starred = ?15, is_draft = ?16,
                 is_answered = ?17, is_deleted = ?18, sent_at = ?19, received_at = ?20,
                 created_at = ?21, updated_at = ?22
             WHERE id = ?23",
            rusqlite::params![
                write.account_id,
                write.folder,
                i64::from(write.uid),
                write.message_id,
                write.subject,
                write.sender_name,
                write.sender_email,
                write.recipient_emails,
                write.cc_emails,
                write.bcc_emails,
                write.preview,
                write.body_text,
                write.body_html,
                opt_bool_to_int(write.is_read),
                opt_bool_to_int(write.is_starred),
                opt_bool_to_int(write.is_draft),
                opt_bool_to_int(write.is_answered),
                opt_bool_to_int(write.is_deleted),
                write.sent_at,
                write.received_at,
                write.created_at,
                write.updated_at,
                email_id,
            ],
        )?;
        tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        let mut attachment_stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        for attachment in &attachments {
            execute_attachment_insert(&mut attachment_stmt, attachment)?;
        }
        let mut stmt = tx.prepare(
            "SELECT id, account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
             FROM emails
             WHERE id = ?1",
        )?;
        stmt.query_row([email_id], map_email)
    })
    .await
}

/// 删除单封本地邮件及附件数据库元数据。
///
/// 仅删除数据库中的附件元数据和邮件记录，不删除已下载到磁盘的附件文件。
pub async fn delete_one_with_attachments(db: &DbConn, email_id: i32) -> Result<bool, MailError> {
    db.transaction(move |tx| {
        tx.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        let deleted = tx.execute("DELETE FROM emails WHERE id = ?1", [email_id])?;
        Ok(deleted > 0)
    })
    .await
}

/// 根据 account_id + folder + uid 更新邮件正文。
///
/// 同步正文后回填 `body_text`、`body_html` 和自动生成的 `preview`（取正文前 200 字符）。
/// 定位依据为账号 ID + 文件夹 + UID，以避免不同文件夹中 UID 重复时误更新。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `uid`: IMAP UID
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 正文
///
/// # 返回
///
/// 更新成功返回 `Ok(())`。
pub(crate) async fn update_body(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: u32,
    body_text: String,
    body_html: String,
) -> Result<(), MailError> {
    let preview: Option<String> = Some(body_text.chars().take(200).collect());
    let now = chrono::Utc::now().timestamp();
    let folder = folder.to_string();

    db.call(move |conn| {
        conn.execute(
            "UPDATE emails
             SET body_text = ?1, body_html = ?2, preview = ?3, updated_at = ?4
             WHERE account_id = ?5 AND folder = ?6 AND uid = ?7",
            rusqlite::params![
                body_text,
                body_html,
                preview,
                now,
                account_id,
                folder,
                i64::from(uid)
            ],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_account(db: &DbConn) -> i32 {
        let now = chrono::Utc::now().timestamp();
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO accounts (
                    name, email, provider, imap_host, imap_port, imap_ssl, imap_ssl_mode,
                    smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, auth_type, account_type,
                    created_at, updated_at
                ) VALUES (
                    'Test', 'test@example.com', 'gmail', 'imap.example.com', 993, 1, 'SSL',
                    'smtp.example.com', 465, 1, 'SSL', 'Password', 'personal', ?1, ?1
                )",
                [now],
            )?;
            Ok(conn.last_insert_rowid() as i32)
        })
        .await
        .unwrap()
    }

    async fn insert_email(db: &DbConn, account_id: i32, folder: &str, uid: u32, body: &str) -> i32 {
        let now = chrono::Utc::now().timestamp();
        let folder = folder.to_string();
        let body = body.to_string();
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO emails (
                    account_id, folder, uid, message_id, subject, sender_name, sender_email,
                    recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
                    is_read, is_starred, is_draft, is_answered, is_deleted,
                    sent_at, received_at, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, NULL, 'Subject', 'Sender', 'sender@example.com',
                    'recipient@example.com', NULL, NULL, NULL, ?4, NULL,
                    0, 0, 0, 0, 0, ?5, ?5, ?5, ?5
                )",
                rusqlite::params![account_id, folder, i64::from(uid), body, now],
            )?;
            Ok(conn.last_insert_rowid() as i32)
        })
        .await
        .unwrap()
    }

    async fn body_text(db: &DbConn, id: i32) -> Option<String> {
        db.call(move |conn| {
            conn.query_row("SELECT body_text FROM emails WHERE id = ?1", [id], |row| {
                row.get(0)
            })
        })
        .await
        .unwrap()
    }

    async fn insert_attachment(db: &DbConn, email_id: i32, filename: &str) {
        let filename = filename.to_string();
        db.call(move |conn| {
            conn.execute(
                "INSERT INTO attachments (
                    email_id, filename, content_type, size, section_path,
                    disposition, content_id, path, created_at
                ) VALUES (?1, ?2, 'application/pdf', 10, '2', 'attachment', NULL, NULL, ?3)",
                rusqlite::params![email_id, filename, chrono::Utc::now().timestamp()],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    }

    async fn attachment_filenames(db: &DbConn, email_id: i32) -> Vec<String> {
        db.call(move |conn| {
            let mut stmt = conn
                .prepare("SELECT filename FROM attachments WHERE email_id = ?1 ORDER BY id ASC")?;
            let rows = stmt.query_map([email_id], |row| row.get::<_, Option<String>>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map(|items| items.into_iter().flatten().collect())
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn replace_email_with_attachments_overwrites_email_and_attachments() {
        use crate::infrastructure::protocols::types::{AttachmentInfo, WholeEmailDto};

        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let account_id = create_account(&db).await;
        let email_id = insert_email(&db, account_id, "INBOX", 100, "old body").await;
        insert_attachment(&db, email_id, "old.pdf").await;

        let replacement = WholeEmailDto {
            id: 0,
            account_id: 0,
            folder: "INBOX".to_string(),
            uid: 100,
            message_id: Some("<new@example.com>".to_string()),
            subject: Some("新主题".to_string()),
            sender_name: Some("New Sender".to_string()),
            sender_email: "new@example.com".to_string(),
            recipient_emails: "to@example.com".to_string(),
            cc_emails: Some("cc@example.com".to_string()),
            bcc_emails: None,
            preview: Some("新正文".to_string()),
            body_text: Some("新正文".to_string()),
            body_html: Some("<p>新正文</p>".to_string()),
            attachments: vec![AttachmentInfo {
                filename: Some("new.pdf".to_string()),
                content_type: "application/pdf".to_string(),
                size: 42,
                section_path: "2".to_string(),
                disposition: Some("attachment".to_string()),
                content_id: None,
            }],
            is_read: true,
            is_starred: true,
            is_draft: false,
            is_answered: true,
            is_deleted: false,
            sent_at: 1_800_000_100,
            received_at: 1_800_000_101,
            created_at: 0,
        };

        let updated =
            replace_email_with_attachments(&db, email_id, account_id, "INBOX", replacement)
                .await
                .unwrap();

        assert_eq!(updated.id, email_id);
        assert_eq!(updated.subject.as_deref(), Some("新主题"));
        assert_eq!(updated.body_text.as_deref(), Some("新正文"));

        let attachment_names = attachment_filenames(&db, email_id).await;
        assert_eq!(attachment_names, vec!["new.pdf".to_string()]);
    }

    #[tokio::test]
    async fn update_body_updates_only_matching_folder_when_uid_repeats() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let account_id = create_account(&db).await;
        let inbox_id = insert_email(&db, account_id, "INBOX", 42, "inbox body").await;
        let sent_id = insert_email(&db, account_id, "Sent", 42, "sent body").await;

        update_body(
            &db,
            account_id,
            "INBOX",
            42,
            "updated inbox body".to_string(),
            "<p>updated inbox body</p>".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(
            body_text(&db, inbox_id).await.as_deref(),
            Some("updated inbox body")
        );
        assert_eq!(body_text(&db, sent_id).await.as_deref(), Some("sent body"));
    }
}
