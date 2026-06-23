use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::{sync_errors, sync_state};
use crate::infrastructure::storage::row::opt_int_to_bool;

fn map_sync_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<sync_state::Model> {
    Ok(sync_state::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        folder: row.get("folder")?,
        folder_nick_name: row.get("folder_nick_name")?,
        uidvalidity: row.get("uidvalidity")?,
        uidnext: row.get("uidnext")?,
        synced_at: row.get("synced_at")?,
        last_sync_uid: row.get("last_sync_uid")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_sync_error(row: &rusqlite::Row<'_>) -> rusqlite::Result<sync_errors::Model> {
    Ok(sync_errors::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        folder: row.get("folder")?,
        error_type: row.get("error_type")?,
        error_message: row.get("error_message")?,
        uid: row.get("uid")?,
        stack_trace: row.get("stack_trace")?,
        resolved: opt_int_to_bool(row.get("resolved")?),
        created_at: row.get("created_at")?,
    })
}

pub async fn upsert_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidnext: Option<u32>,
    uidvalidity: Option<u32>,
    last_sync_uid: Option<u32>,
) -> Result<(), MailError> {
    let folder = folder.to_string();
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, uidnext, uidvalidity, last_sync_uid, synced_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6)
             ON CONFLICT(account_id, folder) DO UPDATE SET
                uidnext = COALESCE(excluded.uidnext, sync_state.uidnext),
                uidvalidity = COALESCE(excluded.uidvalidity, sync_state.uidvalidity),
                last_sync_uid = COALESCE(excluded.last_sync_uid, sync_state.last_sync_uid),
                synced_at = excluded.synced_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                account_id,
                folder,
                uidnext,
                uidvalidity,
                last_sync_uid,
                now,
            ],
        )?;
        Ok(())
    })
    .await
}

pub async fn upsert_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidnext: Option<u32>,
    uidvalidity: Option<u32>,
    last_sync_uid: Option<u32>,
) -> Result<(), MailError> {
    upsert_sync_state(db, account_id, folder, uidnext, uidvalidity, last_sync_uid).await
}

pub async fn get_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, folder_nick_name, uidvalidity, uidnext,
                    synced_at, last_sync_uid, created_at, updated_at
             FROM sync_state
             WHERE account_id = ?1 AND folder = ?2",
        )?;
        match stmt.query_row(rusqlite::params![account_id, folder], map_sync_state) {
            Ok(state) => Ok(Some(state)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(err),
        }
    })
    .await
}

pub async fn get_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>, MailError> {
    get_sync_state(db, account_id, folder).await
}

pub async fn update_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uidnext: Option<u32>,
    uidvalidity: Option<u32>,
    last_sync_uid: Option<u32>,
) -> Result<(), MailError> {
    upsert_sync_state(db, account_id, folder, uidnext, uidvalidity, last_sync_uid).await
}

pub async fn record_error(
    db: &DbConn,
    account_id: i32,
    folder: Option<String>,
    error_type: &str,
    error_message: &str,
    uid: Option<u32>,
) -> Result<(), MailError> {
    let error_type = error_type.to_string();
    let error_message = error_message.to_string();
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO sync_errors (
                account_id, folder, error_type, error_message, uid, stack_trace, resolved, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, ?6)",
            rusqlite::params![account_id, folder, error_type, error_message, uid, now],
        )?;
        Ok(())
    })
    .await
}

pub async fn log_error(
    db: &DbConn,
    account_id: i32,
    folder: Option<String>,
    error_type: &str,
    error_message: &str,
    uid: Option<u32>,
) -> Result<(), MailError> {
    record_error(db, account_id, folder, error_type, error_message, uid).await
}

pub async fn list_unresolved_errors(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<sync_errors::Model>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, error_type, error_message, uid, stack_trace, resolved, created_at
             FROM sync_errors
             WHERE account_id = ?1 AND (resolved = 0 OR resolved IS NULL)
             ORDER BY id ASC",
        )?;
        stmt.query_map([account_id], map_sync_error)?.collect()
    })
    .await
}

pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute(
            "DELETE FROM sync_errors WHERE account_id = ?1",
            [account_id],
        )?;
        conn.execute("DELETE FROM sync_state WHERE account_id = ?1", [account_id])?;
        Ok(())
    })
    .await
}

pub fn delete_by_account_tx(
    tx: &rusqlite::Transaction<'_>,
    account_id: i32,
) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM sync_errors WHERE account_id = ?1",
        [account_id],
    )?;
    tx.execute("DELETE FROM sync_state WHERE account_id = ?1", [account_id])?;
    Ok(())
}
