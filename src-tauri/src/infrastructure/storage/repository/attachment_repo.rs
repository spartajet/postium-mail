use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::attachments;

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

pub fn insert_attachment_tx(
    tx: &rusqlite::Transaction<'_>,
    model: &AttachmentWrite,
) -> rusqlite::Result<i32> {
    tx.execute(
        "INSERT INTO attachments (
            email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            model.email_id,
            &model.filename,
            &model.content_type,
            model.size,
            &model.section_path,
            &model.disposition,
            &model.content_id,
            &model.path,
            model.created_at,
        ],
    )?;
    Ok(tx.last_insert_rowid() as i32)
}

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

pub async fn bulk_insert(db: &DbConn, models: Vec<AttachmentWrite>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }

    db.transaction(move |tx| {
        let mut stmt = tx.prepare(
            "INSERT INTO attachments (
                email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for model in models {
            stmt.execute(rusqlite::params![
                model.email_id,
                model.filename,
                model.content_type,
                model.size,
                model.section_path,
                model.disposition,
                model.content_id,
                model.path,
                model.created_at,
            ])?;
        }
        Ok(())
    })
    .await
}

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

pub async fn delete_by_email(db: &DbConn, email_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        Ok(())
    })
    .await
}
