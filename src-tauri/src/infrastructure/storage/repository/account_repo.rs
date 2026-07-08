//! 账号仓库模块（Account Repository）
//!
//! 负责 `accounts` 表的数据访问，包括账号的增删改查（CRUD）操作，
//! 以及更新账号的最后同步时间。
//!
//! 主要操作：
//! - 查询账号列表、按 ID/邮箱查询账号
//! - 创建、更新、删除账号
//! - 记录账号的最后同步时间

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::accounts;
use crate::infrastructure::storage::row::{opt_bool_to_int, opt_int_to_bool};

/// 用于创建或更新账号的写入数据结构
///
/// 不包含自增主键 `id`，其余字段与 `accounts` 表一一对应。
/// 布尔字段使用 `Option<bool>`，序列化时转换为 0/1 整数存储。
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

/// 将数据库行映射为 `accounts::Model`
///
/// 同时将整数形式的布尔字段转换为 `Option<bool>`。
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

/// 查询所有账号，按 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
///
/// # 返回
///
/// 账号列表，按 ID 升序排列。
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

/// 根据 ID 查询账号。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 账号 ID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
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

/// 根据邮箱地址查询账号。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email`: 账号邮箱地址
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
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

/// 创建新账号并返回创建后的完整记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `model`: 待创建的账号数据
///
/// # 返回
///
/// 创建成功后的账号记录（包含自增 ID）。
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

/// 根据 ID 更新账号，返回更新后的完整记录。
///
/// 覆盖更新所有字段，调用方需提供完整的 `AccountWrite` 数据。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 待更新的账号 ID
/// - `model`: 新的账号数据
///
/// # 返回
///
/// 更新后的账号记录。
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

/// 根据 ID 删除账号。
///
/// 注意：此操作仅删除 `accounts` 表中的记录，不级联删除关联数据。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 待删除的账号 ID
///
/// # 返回
///
/// 删除成功返回 `Ok(())`。
pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
}

/// 在指定事务中删除账号（事务版本）
///
/// 供需要跨表事务性删除的调用方使用。
///
/// # 参数
///
/// - `tx`: 数据库事务
/// - `id`: 待删除的账号 ID
pub fn delete_tx(tx: &rusqlite::Transaction<'_>, id: i32) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM accounts WHERE id = ?1", [id])?;
    Ok(())
}

/// 更新账号的最后同步时间为当前时间。
///
/// 同时更新 `updated_at` 字段。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 账号 ID
///
/// # 返回
///
/// 更新成功返回 `Ok(())`。
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
