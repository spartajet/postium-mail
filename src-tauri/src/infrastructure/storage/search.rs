use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type, FromQueryResult)]
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

pub async fn search_fts(
    db: &DbConn,
    query: &str,
    account_id: Option<i32>,
    limit: u64,
) -> Result<Vec<SearchResult>, MailError> {
    let stmt = if let Some(aid) = account_id {
        Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank \
             FROM emails_fts f JOIN emails e ON f.rowid = e.id \
             WHERE emails_fts MATCH ? AND e.account_id = ? AND e.is_deleted = 0 \
             ORDER BY f.rank DESC LIMIT ?",
            [query.into(), aid.into(), limit.into()],
        )
    } else {
        Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT e.id, e.account_id, e.folder, e.subject, e.sender_email, e.sent_at, e.preview, f.rank \
             FROM emails_fts f JOIN emails e ON f.rowid = e.id \
             WHERE emails_fts MATCH ? AND e.is_deleted = 0 \
             ORDER BY f.rank DESC LIMIT ?",
            [query.into(), limit.into()],
        )
    };

    let rows = db.query_all_raw(stmt).await?;
    let mut results = Vec::new();
    for row in rows {
        results.push(SearchResult::from_query_result(&row, "")?);
    }
    Ok(results)
}
