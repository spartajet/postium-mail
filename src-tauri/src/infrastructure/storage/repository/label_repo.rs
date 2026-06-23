use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::labels;

pub struct LabelWrite {
    pub account_id: i32,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

fn map_label(row: &rusqlite::Row<'_>) -> rusqlite::Result<labels::Model> {
    Ok(labels::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        created_at: row.get("created_at")?,
    })
}

pub async fn list_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<labels::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, name, color, created_at
             FROM labels
             WHERE account_id = ?1
             ORDER BY id ASC",
        )?;
        stmt.query_map([account_id], map_label)?.collect()
    })
    .await
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<labels::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, name, color, created_at
             FROM labels
             WHERE id = ?1",
        )?;
        match stmt.query_row([id], map_label) {
            Ok(label) => Ok(Some(label)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn create(db: &DbConn, model: LabelWrite) -> Result<labels::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO labels (account_id, name, color, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![model.account_id, model.name, model.color, model.created_at],
        )?;

        let id = conn.last_insert_rowid() as i32;
        let mut stmt = conn.prepare(
            "SELECT id, account_id, name, color, created_at
             FROM labels
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_label)
    })
    .await
}

pub async fn update(db: &DbConn, id: i32, model: LabelWrite) -> Result<labels::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE labels
             SET account_id = ?1, name = ?2, color = ?3, created_at = ?4
             WHERE id = ?5",
            rusqlite::params![
                model.account_id,
                model.name,
                model.color,
                model.created_at,
                id
            ],
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, account_id, name, color, created_at
             FROM labels
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_label)
    })
    .await
}

pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.transaction(move |tx| delete_tx(tx, id)).await
}

pub fn delete_tx(tx: &rusqlite::Transaction<'_>, id: i32) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM email_labels WHERE label_id = ?1", [id])?;
    tx.execute("DELETE FROM labels WHERE id = ?1", [id])?;
    Ok(())
}

pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    db.transaction(move |tx| delete_by_account_tx(tx, account_id))
        .await
}

pub fn delete_by_account_tx(
    tx: &rusqlite::Transaction<'_>,
    account_id: i32,
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM email_labels
         WHERE email_id IN (SELECT id FROM emails WHERE account_id = ?1)
            OR label_id IN (SELECT id FROM labels WHERE account_id = ?1)",
        [account_id],
    )?;
    tx.execute("DELETE FROM labels WHERE account_id = ?1", [account_id])?;
    Ok(())
}

pub async fn get_labels_for_email(
    db: &DbConn,
    email_id: i32,
) -> Result<Vec<labels::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT l.id, l.account_id, l.name, l.color, l.created_at
             FROM labels l
             INNER JOIN email_labels el ON el.label_id = l.id
             WHERE el.email_id = ?1
             ORDER BY l.id ASC",
        )?;
        stmt.query_map([email_id], map_label)?.collect()
    })
    .await
}

pub async fn add_to_email(db: &DbConn, email_id: i32, label_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR IGNORE INTO email_labels (email_id, label_id, created_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![email_id, label_id, now],
        )?;
        Ok(())
    })
    .await
}

pub async fn remove_from_email(db: &DbConn, email_id: i32, label_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute(
            "DELETE FROM email_labels WHERE email_id = ?1 AND label_id = ?2",
            rusqlite::params![email_id, label_id],
        )?;
        Ok(())
    })
    .await
}

pub async fn list_emails_by_label(db: &DbConn, label_id: i32) -> Result<Vec<i32>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT email_id
             FROM email_labels
             WHERE label_id = ?1
             ORDER BY email_id ASC",
        )?;
        stmt.query_map([label_id], |row| row.get("email_id"))?
            .collect()
    })
    .await
}
