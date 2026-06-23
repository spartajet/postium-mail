use crate::domain::sync::FolderStat;
use crate::error::MailError;
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use crate::infrastructure::protocols::utils::{
    extract_email_from_address, extract_name_from_address, serialize_addresses,
};
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::emails;
use crate::infrastructure::storage::repository::attachment_repo::{
    AttachmentWrite, insert_attachment_tx,
};
use crate::infrastructure::storage::row::{bool_to_int, opt_bool_to_int, opt_int_to_bool};
use rusqlite::types::Value;

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

fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

fn insert_email_tx(tx: &rusqlite::Transaction<'_>, model: &EmailWrite) -> rusqlite::Result<i32> {
    tx.execute(
        "INSERT INTO emails (
            account_id, folder, uid, message_id, subject, sender_name, sender_email,
            recipient_emails, cc_emails, bcc_emails, preview, body_text, body_html,
            is_read, is_starred, is_draft, is_answered, is_deleted,
            sent_at, received_at, created_at, updated_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
            ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
         )",
        rusqlite::params![
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
        ],
    )?;
    Ok(tx.last_insert_rowid() as i32)
}

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

/// 按多个文件夹查询邮件（用于 category → 多文件夹映射）
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

/// 查询星标邮件（跨所有文件夹）
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

pub async fn create(db: &DbConn, model: EmailWrite) -> Result<emails::Model, MailError> {
    db.transaction(move |tx| {
        let id = insert_email_tx(tx, &model)?;
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

pub async fn bulk_insert(db: &DbConn, models: Vec<EmailWrite>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }

    db.transaction(move |tx| {
        for model in &models {
            insert_email_tx(tx, model)?;
        }
        Ok(())
    })
    .await
}

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

pub async fn mark_as_read(db: &DbConn, id: i32, is_read: bool) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE emails SET is_read = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![bool_to_int(is_read), chrono::Utc::now().timestamp(), id],
        )?;
        Ok(())
    })
    .await
}

pub async fn toggle_star(db: &DbConn, id: i32) -> Result<bool, MailError> {
    db.call(move |conn| {
        let current =
            match conn.query_row("SELECT is_starred FROM emails WHERE id = ?1", [id], |row| {
                row.get::<_, Option<i64>>(0)
            }) {
                Ok(value) => value,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
                Err(err) => return Err(err),
            };
        let new_state = !opt_int_to_bool(current).unwrap_or(false);
        conn.execute(
            "UPDATE emails SET is_starred = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![bool_to_int(new_state), chrono::Utc::now().timestamp(), id],
        )?;
        Ok(new_state)
    })
    .await
    .map_err(|err| match err {
        MailError::DatabaseError(message) if message.contains("Query returned no rows") => {
            MailError::EmailNotFound(id)
        }
        other => other,
    })
}

pub async fn soft_delete(db: &DbConn, ids: Vec<i32>) -> Result<usize, MailError> {
    if ids.is_empty() {
        return Ok(0);
    }

    db.call(move |conn| {
        let sql = format!(
            "UPDATE emails
             SET is_deleted = 1, updated_at = ?
             WHERE id IN ({})",
            placeholders(ids.len())
        );
        let mut values = Vec::with_capacity(ids.len() + 1);
        values.push(Value::from(chrono::Utc::now().timestamp()));
        values.extend(ids.into_iter().map(Value::from));
        conn.execute(&sql, rusqlite::params_from_iter(values.iter()))
    })
    .await
}

pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<(), MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        conn.execute(
            "UPDATE emails SET folder = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![folder, chrono::Utc::now().timestamp(), id],
        )?;
        Ok(())
    })
    .await
}

/// 按 account_id 聚合文件夹统计（单条 SQL，替代 N+1 查询）
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

/// 根据账号和文件夹获取邮件 ID
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

/// 根据账号和文件夹删除邮件
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

pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<u64, MailError> {
    db.transaction(move |tx| delete_by_account_tx(tx, account_id))
        .await
}

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

/// 批量保存邮件头
///
/// 从 IMAP 同步的邮件头批量保存到数据库。
/// 这个方法主要用于快速同步邮件列表，不包含邮件正文和附件。
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
        for (email, attachments) in &writes {
            let email_id = insert_email_tx(tx, email)?;
            for attachment in attachments {
                let mut attachment = attachment.clone();
                attachment.email_id = email_id;
                insert_attachment_tx(tx, &attachment)?;
            }
        }
        Ok(writes.len())
    })
    .await
}

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
        for (email, attachments) in &writes {
            let email_id = insert_email_tx(tx, email)?;
            for attachment in attachments {
                let mut attachment = attachment.clone();
                attachment.email_id = email_id;
                insert_attachment_tx(tx, &attachment)?;
            }
        }
        Ok(writes.len())
    })
    .await
}

pub(crate) async fn update_body(
    db: &DbConn,
    account_id: i32,
    uid: u32,
    body_text: String,
    body_html: String,
) -> Result<(), MailError> {
    let preview: Option<String> = Some(body_text.chars().take(200).collect());
    let now = chrono::Utc::now().timestamp();

    db.call(move |conn| {
        conn.execute(
            "UPDATE emails
             SET body_text = ?1, body_html = ?2, preview = ?3, updated_at = ?4
             WHERE account_id = ?5 AND uid = ?6",
            rusqlite::params![
                body_text,
                body_html,
                preview,
                now,
                account_id,
                i64::from(uid)
            ],
        )?;
        Ok(())
    })
    .await
}
