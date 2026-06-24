//! 全文搜索（FTS）模块
//!
//! 基于 SQLite 的 FTS5 虚拟表（`emails_fts`）提供邮件全文搜索功能。
//! 搜索结果按相关性排序（rank 越高越相关），并自动排除已删除的邮件。
//! 支持按账号过滤，返回邮件的基本信息和预览文本。

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 全文搜索的返回结果
///
/// 包含邮件基本信息及 FTS 相关性排名（rank），rank 越高表示匹配度越高。
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

/// 将查询行映射为 `SearchResult`。
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

/// 执行全文搜索，返回按相关性排序的邮件列表。
///
/// 在 FTS 虚拟表上执行 MATCH 查询，关联 `emails` 表获取邮件详情。
/// 自动排除已逻辑删除的邮件（`is_deleted = 0`）。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `query`: FTS5 查询字符串（支持 MATCH 语法）
/// - `account_id`: 可选的账号 ID 过滤，`None` 表示搜索所有账号
/// - `limit`: 返回结果的最大数量
///
/// # 返回
///
/// 按相关性降序排列的搜索结果列表。
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
