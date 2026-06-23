use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SearchResult {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub subject: Option<String>,
    pub sender_email: String,
    pub sent_at: i64,
    pub preview: Option<String>,
    pub rank: f64,
}

fn map_search_result(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchResult> {
    Ok(SearchResult {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        folder: row.get("folder")?,
        subject: row.get("subject")?,
        sender_email: row.get("sender_email")?,
        sent_at: row.get("sent_at")?,
        preview: row.get("preview")?,
        rank: row.get("rank")?,
    })
}

pub async fn search_fts(
    db: &DbConn,
    query: &str,
    account_id: Option<i32>,
    limit: u64,
) -> Result<Vec<SearchResult>, MailError> {
    let query = query.to_string();
    db.call(move |conn| {
        let mut results = Vec::new();
        if let Some(account_id) = account_id {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank
                 FROM emails_fts f JOIN emails e ON f.rowid = e.id
                 WHERE emails_fts MATCH ?1 AND e.account_id = ?2 AND e.is_deleted = 0
                 ORDER BY f.rank DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                rusqlite::params![query, account_id, limit as i64],
                map_search_result,
            )?;
            for row in rows {
                results.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank
                 FROM emails_fts f JOIN emails e ON f.rowid = e.id
                 WHERE emails_fts MATCH ?1 AND e.is_deleted = 0
                 ORDER BY f.rank DESC LIMIT ?2",
            )?;
            let rows =
                stmt.query_map(rusqlite::params![query, limit as i64], map_search_result)?;
            for row in rows {
                results.push(row?);
            }
        }
        Ok(results)
    })
    .await
}
