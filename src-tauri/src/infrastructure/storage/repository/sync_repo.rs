//! 同步状态仓库模块（Sync Repository）
//!
//! 负责 `sync_state` 和 `sync_errors` 两张表的数据访问，
//! 记录邮件同步的进度状态和错误信息。
//!
//! 主要操作：
//! - 同步状态（sync_state）：记录/查询每个账号每个文件夹的 IMAP uidvalidity、uidnext、last_sync_uid
//! - 同步错误（sync_errors）：记录同步过程中的错误、查询未解决的错误
//! - 按账号清理同步状态和错误记录

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::{sync_errors, sync_state};
use crate::infrastructure::storage::row::opt_int_to_bool;

/// 将数据库行映射为 `sync_state::Model`
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
        history_synced_since: row.get("history_synced_since")?,
        history_before_uid: row.get("history_before_uid")?,
        history_exhausted: opt_int_to_bool(row.get("history_exhausted")?),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// 将数据库行映射为 `sync_errors::Model`
///
/// 将整数形式的 `resolved` 字段转换为 `Option<bool>`。
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

/// 写入或更新文件夹的同步状态（upsert）。
///
/// 基于账号 ID + 文件夹的唯一约束进行 upsert 操作。
/// 对于 `uidnext`、`uidvalidity`、`last_sync_uid` 字段，传入 `None` 时保留原值（使用 COALESCE）。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `uidnext`: IMAP uidnext 值，`None` 表示不更新
/// - `uidvalidity`: IMAP uidvalidity 值，`None` 表示不更新
/// - `last_sync_uid`: 上次同步到的最大 UID，`None` 表示不更新
///
/// # 返回
///
/// 写入/更新成功返回 `Ok(())`。
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

/// [`upsert_sync_state`] 的别名
///
/// 供旧代码使用的一致命名封装。
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

/// 查询指定账号和文件夹的同步状态。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
pub async fn get_sync_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>, MailError> {
    let folder = folder.to_string();
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, account_id, folder, folder_nick_name, uidvalidity, uidnext,
                    synced_at, last_sync_uid, history_synced_since, history_before_uid,
                    history_exhausted,
                    created_at, updated_at
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

/// [`get_sync_state`] 的别名
///
/// 供旧代码使用的一致命名封装。
pub async fn get_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Option<sync_state::Model>, MailError> {
    get_sync_state(db, account_id, folder).await
}

/// 查询账号本地同步状态中实际存在的文件夹名称。
pub async fn distinct_folders_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<String>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT folder
             FROM sync_state
             WHERE account_id = ?1
             ORDER BY folder",
        )?;
        let folders = stmt
            .query_map([account_id], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(folders)
    })
    .await
}

/// 更新文件夹的同步状态（实际委托给 [`upsert_sync_state`]）
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `uidnext`: IMAP uidnext 值
/// - `uidvalidity`: IMAP uidvalidity 值
/// - `last_sync_uid`: 上次同步到的最大 UID
///
/// # 返回
///
/// 更新成功返回 `Ok(())`。
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

/// 更新文件夹的历史同步窗口状态。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称
/// - `history_synced_since`: 历史同步已覆盖到的最早时间戳（Unix 秒）
/// - `history_before_uid`: 下一次历史回填查询的 UID 上界
/// - `history_exhausted`: 是否确认不存在更早历史邮件
pub async fn update_history_state(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    history_synced_since: Option<i64>,
    history_before_uid: Option<u32>,
    history_exhausted: bool,
) -> Result<(), MailError> {
    let folder = folder.to_string();
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, history_synced_since, history_before_uid,
                history_exhausted, synced_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6)
             ON CONFLICT(account_id, folder) DO UPDATE SET
                history_synced_since = excluded.history_synced_since,
                history_before_uid = excluded.history_before_uid,
                history_exhausted = excluded.history_exhausted,
                synced_at = excluded.synced_at,
                updated_at = excluded.updated_at",
            rusqlite::params![
                account_id,
                folder,
                history_synced_since,
                history_before_uid,
                if history_exhausted { 1 } else { 0 },
                now,
            ],
        )?;
        Ok(())
    })
    .await
}

/// 记录一条同步错误。
///
/// 将错误信息持久化到 `sync_errors` 表，`resolved` 初始为 0（未解决）。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
/// - `folder`: 文件夹名称，可为 `None`（非文件夹级错误）
/// - `error_type`: 错误类型标识
/// - `error_message`: 错误详情
/// - `uid`: 关联的邮件 UID，可为 `None`
///
/// # 返回
///
/// 记录成功返回 `Ok(())`。
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

/// [`record_error`] 的别名
///
/// 供旧代码使用的一致命名封装。
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

/// 查询指定账号下所有未解决的同步错误，按 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 该账号未解决（`resolved = 0` 或 `NULL`）的错误列表。
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

/// 删除指定账号的所有同步状态和错误记录。
///
/// 在事务中同时清理 `sync_errors` 和 `sync_state` 两张表。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 删除成功返回 `Ok(())`。
pub async fn delete_by_account(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    db.transaction(move |tx| delete_by_account_tx(tx, account_id))
        .await
}

/// 在指定事务中删除账号的同步状态和错误记录（事务版本）
///
/// 先清理 `sync_errors`，再清理 `sync_state`。
///
/// # 参数
///
/// - `tx`: 数据库事务
/// - `account_id`: 账号 ID
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
