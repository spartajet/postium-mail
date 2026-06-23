use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::accounts;
use crate::infrastructure::storage::row::{opt_bool_to_int, opt_int_to_bool};

pub struct AccountWrite {
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub provider: String,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_ssl: Option<bool>,
    pub imap_ssl_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_ssl: Option<bool>,
    pub smtp_ssl_mode: Option<String>,
    pub color: Option<String>,
    pub sync_enabled: Option<bool>,
    pub last_sync_at: Option<i64>,
    pub auth_type: Option<String>,
    pub account_type: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn map_account(row: &rusqlite::Row<'_>) -> rusqlite::Result<accounts::Model> {
    Ok(accounts::Model {
        id: row.get("id")?,
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
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub async fn list(db: &DbConn) -> Result<Vec<accounts::Model>, MailError> {
    db.call(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                    imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                    sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
             FROM accounts
             ORDER BY id ASC",
        )?;
        stmt.query_map([], map_account)?.collect()
    })
    .await
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<accounts::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                    imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                    sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
             FROM accounts
             WHERE id = ?1",
        )?;
        match stmt.query_row([id], map_account) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<accounts::Model>, MailError> {
    let email = email.to_string();
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                    imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                    sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
             FROM accounts
             WHERE email = ?1",
        )?;
        match stmt.query_row([email], map_account) {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn create(db: &DbConn, model: AccountWrite) -> Result<accounts::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO accounts (
                name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            rusqlite::params![
                model.name,
                model.email,
                model.display_name,
                model.provider,
                model.imap_host,
                model.imap_port,
                opt_bool_to_int(model.imap_ssl),
                model.imap_ssl_mode,
                model.smtp_host,
                model.smtp_port,
                opt_bool_to_int(model.smtp_ssl),
                model.smtp_ssl_mode,
                model.color,
                opt_bool_to_int(model.sync_enabled),
                model.last_sync_at,
                model.auth_type,
                model.account_type,
                model.created_at,
                model.updated_at,
            ],
        )?;

        let id = conn.last_insert_rowid() as i32;
        let mut stmt = conn.prepare(
            "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                    imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                    sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
             FROM accounts
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_account)
    })
    .await
}

pub async fn update(
    db: &DbConn,
    id: i32,
    model: AccountWrite,
) -> Result<accounts::Model, MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE accounts
             SET name = ?1, email = ?2, display_name = ?3, provider = ?4,
                 imap_host = ?5, imap_port = ?6, imap_ssl = ?7, imap_ssl_mode = ?8,
                 smtp_host = ?9, smtp_port = ?10, smtp_ssl = ?11, smtp_ssl_mode = ?12,
                 color = ?13, sync_enabled = ?14, last_sync_at = ?15, auth_type = ?16,
                 account_type = ?17, created_at = ?18, updated_at = ?19
             WHERE id = ?20",
            rusqlite::params![
                model.name,
                model.email,
                model.display_name,
                model.provider,
                model.imap_host,
                model.imap_port,
                opt_bool_to_int(model.imap_ssl),
                model.imap_ssl_mode,
                model.smtp_host,
                model.smtp_port,
                opt_bool_to_int(model.smtp_ssl),
                model.smtp_ssl_mode,
                model.color,
                opt_bool_to_int(model.sync_enabled),
                model.last_sync_at,
                model.auth_type,
                model.account_type,
                model.created_at,
                model.updated_at,
                id,
            ],
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, name, email, display_name, provider, imap_host, imap_port, imap_ssl,
                    imap_ssl_mode, smtp_host, smtp_port, smtp_ssl, smtp_ssl_mode, color,
                    sync_enabled, last_sync_at, auth_type, account_type, created_at, updated_at
             FROM accounts
             WHERE id = ?1",
        )?;
        stmt.query_row([id], map_account)
    })
    .await
}

pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
}

pub fn delete_tx(tx: &rusqlite::Transaction<'_>, id: i32) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    Ok(())
}

pub async fn update_last_sync(db: &DbConn, id: i32) -> Result<(), MailError> {
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "UPDATE accounts SET last_sync_at = ?1, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id],
        )?;
        Ok(())
    })
    .await
}
