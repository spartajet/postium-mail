//! 附件仓库模块（Attachment Repository）
//!
//! 负责 `attachments` 表的数据访问，管理邮件附件的存储和查询。
//! 支持单条插入、批量插入、按邮件查询和删除附件。
//!
//! 主要操作：
//! - 按邮件 ID 查询附件列表
//! - 创建、批量创建附件
//! - 根据 ID 查询单个附件
//! - 按邮件 ID 删除附件

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::{accounts, attachments, emails};
use crate::infrastructure::storage::row::opt_int_to_bool;
use std::collections::{HashMap, HashSet};

/// 批量插入附件时使用的 SQL 语句常量
pub const INSERT_ATTACHMENT_SQL: &str = "INSERT INTO attachments (
        email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

/// 用于创建附件的写入数据结构
///
/// 不包含自增主键 `id`，调用批量插入时需先插入邮件并获取 `email_id` 后再赋值。
#[derive(Clone, Debug)]
pub struct AttachmentWrite {
    pub email_id: i32,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: i64,
    pub section_path: String,
    pub disposition: Option<String>,
    pub content_id: Option<String>,
    pub path: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct AttachmentWithEmailContext {
    pub attachment: attachments::Model,
    pub email: emails::Model,
    pub account: accounts::Model,
}

/// 将数据库行映射为 `attachments::Model`
fn map_attachment(row: &rusqlite::Row<'_>) -> rusqlite::Result<attachments::Model> {
    Ok(attachments::Model {
        id: row.get("id")?,
        email_id: row.get("email_id")?,
        filename: row.get("filename")?,
        content_type: row.get("content_type")?,
        size: row.get("size")?,
        section_path: row.get("section_path")?,
        disposition: row.get("disposition")?,
        content_id: row.get("content_id")?,
        path: row.get("path")?,
        created_at: row.get("created_at")?,
    })
}

fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

/// 在预编译语句上执行单次附件插入。
///
/// 供批量插入场景复用预编译语句，避免重复 prepare 带来的性能开销。
///
/// # 参数
///
/// - `stmt`: 已预编译的插入语句
/// - `model`: 待插入的附件数据
pub fn execute_attachment_insert(
    stmt: &mut rusqlite::Statement<'_>,
    model: &AttachmentWrite,
) -> rusqlite::Result<()> {
    stmt.execute(rusqlite::params![
        model.email_id,
        &model.filename,
        &model.content_type,
        model.size,
        &model.section_path,
        &model.disposition,
        &model.content_id,
        &model.path,
        model.created_at,
    ])?;
    Ok(())
}

/// 查询指定邮件的所有附件，按 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
///
/// # 返回
///
/// 该邮件的附件列表。
pub async fn list_by_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<attachments::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE email_id = ?1
             ORDER BY id ASC",
        )?;
        stmt.query_map([email_id], map_attachment)?.collect()
    })
    .await
}

pub async fn list_by_email_ids(
    db: &DbConn,
    email_ids: Vec<i32>,
) -> Result<HashMap<i32, Vec<attachments::Model>>, MailError> {
    if email_ids.is_empty() {
        return Ok(HashMap::new());
    }

    db.call(move |conn| {
        let sql = format!(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE email_id IN ({})
             ORDER BY email_id ASC, id ASC",
            placeholders(email_ids.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(email_ids), map_attachment)?;
        let mut grouped: HashMap<i32, Vec<attachments::Model>> = HashMap::new();
        for row in rows {
            let attachment = row?;
            grouped
                .entry(attachment.email_id)
                .or_default()
                .push(attachment);
        }
        Ok(grouped)
    })
    .await
}

pub async fn has_for_email_ids(
    db: &DbConn,
    email_ids: Vec<i32>,
) -> Result<HashSet<i32>, MailError> {
    if email_ids.is_empty() {
        return Ok(HashSet::new());
    }

    db.call(move |conn| {
        let sql = format!(
            "SELECT DISTINCT email_id FROM attachments WHERE email_id IN ({})",
            placeholders(email_ids.len())
        );
        let mut stmt = conn.prepare(&sql)?;
        let ids = stmt
            .query_map(rusqlite::params_from_iter(email_ids), |row| {
                row.get::<_, i32>(0)
            })?
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(ids)
    })
    .await
}

pub async fn update_path(
    db: &DbConn,
    attachment_id: i32,
    path: Option<String>,
) -> Result<Option<attachments::Model>, MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE attachments SET path = ?1 WHERE id = ?2",
            rusqlite::params![path, attachment_id],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE id = ?1",
        )?;
        match stmt.query_row([attachment_id], map_attachment) {
            Ok(model) => Ok(Some(model)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn get_with_email_context(
    db: &DbConn,
    attachment_id: i32,
) -> Result<Option<AttachmentWithEmailContext>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT
                a.id AS attachment_id, a.email_id, a.filename, a.content_type, a.size,
                a.section_path, a.disposition, a.content_id, a.path, a.created_at AS attachment_created_at,
                e.id AS email_id_value, e.account_id, e.folder, e.uid, e.message_id, e.subject,
                e.sender_name, e.sender_email, e.recipient_emails, e.cc_emails, e.bcc_emails,
                e.preview, e.body_text, e.body_html, e.is_read, e.is_starred, e.is_draft,
                e.is_answered, e.is_deleted, e.sent_at, e.received_at, e.created_at AS email_created_at,
                e.updated_at,
                ac.id AS account_id_value, ac.name, ac.email, ac.display_name, ac.provider,
                ac.imap_host, ac.imap_port, ac.imap_ssl, ac.imap_ssl_mode, ac.smtp_host,
                ac.smtp_port, ac.smtp_ssl, ac.smtp_ssl_mode, ac.color, ac.sync_enabled,
                ac.last_sync_at, ac.auth_type, ac.account_type,
                ac.created_at AS account_created_at, ac.updated_at AS account_updated_at
             FROM attachments a
             JOIN emails e ON e.id = a.email_id
             JOIN accounts ac ON ac.id = e.account_id
             WHERE a.id = ?1",
        )?;

        match stmt.query_row([attachment_id], |row| {
            Ok(AttachmentWithEmailContext {
                attachment: attachments::Model {
                    id: row.get("attachment_id")?,
                    email_id: row.get("email_id")?,
                    filename: row.get("filename")?,
                    content_type: row.get("content_type")?,
                    size: row.get("size")?,
                    section_path: row.get("section_path")?,
                    disposition: row.get("disposition")?,
                    content_id: row.get("content_id")?,
                    path: row.get("path")?,
                    created_at: row.get("attachment_created_at")?,
                },
                email: emails::Model {
                    id: row.get("email_id_value")?,
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
                    created_at: row.get("email_created_at")?,
                    updated_at: row.get("updated_at")?,
                },
                account: accounts::Model {
                    id: row.get("account_id_value")?,
                    name: row.get("name")?,
                    email: row.get("email")?,
                    display_name: row.get("display_name")?,
                    provider: row.get("provider")?,
                    imap_host: row.get("imap_host")?,
                    imap_port: row.get("imap_port")?,
                    imap_ssl: opt_int_to_bool(row.get("imap_ssl")?),
                    imap_ssl_mode: row.get("imap_ssl_mode")?,
                    smtp_host: row.get("smtp_host")?,
                    smtp_port: row.get("smtp_port")?,
                    smtp_ssl: opt_int_to_bool(row.get("smtp_ssl")?),
                    smtp_ssl_mode: row.get("smtp_ssl_mode")?,
                    color: row.get("color")?,
                    sync_enabled: opt_int_to_bool(row.get("sync_enabled")?),
                    last_sync_at: row.get("last_sync_at")?,
                    auth_type: row.get("auth_type")?,
                    account_type: row.get("account_type")?,
                    created_at: row.get("account_created_at")?,
                    updated_at: row.get("account_updated_at")?,
                },
            })
        }) {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 创建单个附件并返回创建后的完整记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `model`: 待创建的附件数据
///
/// # 返回
///
/// 创建成功后的附件记录（包含自增 ID）。
pub async fn create(db: &DbConn, model: AttachmentWrite) -> Result<attachments::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO attachments (
                email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                model.email_id,
                model.filename,
                model.content_type,
                model.size,
                model.section_path,
                model.disposition,
                model.content_id,
                model.path,
                model.created_at,
            ],
        )?;

        let id = conn.last_insert_rowid() as i32;
        let mut stmt = conn.prepare(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_attachment)
    })
    .await
}

/// 批量插入附件。
///
/// 在单个事务中复用预编译语句逐条插入，空列表直接返回。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `models`: 待插入的附件列表
///
/// # 返回
///
/// 插入成功返回 `Ok(())`。
pub async fn bulk_insert(db: &DbConn, models: Vec<AttachmentWrite>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }

    db.transaction(move |tx| {
        let mut stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        for model in models {
            execute_attachment_insert(&mut stmt, &model)?;
        }
        Ok(())
    })
    .await
}

/// 根据 ID 查询附件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 附件 ID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<attachments::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, email_id, filename, content_type, size, section_path, disposition,
                    content_id, path, created_at
             FROM attachments
             WHERE id = ?1",
        )?;
        match stmt.query_row([id], map_attachment) {
            Ok(attachment) => Ok(Some(attachment)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

/// 删除指定邮件的所有附件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
///
/// # 返回
///
/// 删除成功返回 `Ok(())`。
pub async fn delete_by_email(db: &DbConn, email_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        Ok(())
    })
    .await
}
