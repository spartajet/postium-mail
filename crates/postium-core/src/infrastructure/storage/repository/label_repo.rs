//! 标签仓库模块（Label Repository）
//!
//! 负责 `labels` 表及其关联表 `email_labels` 的数据访问。
//! 管理标签的增删改查，以及标签与邮件的关联关系。
//!
//! 主要操作：
//! - 标签的 CRUD（创建、查询、更新、删除）
//! - 标签与邮件的关联管理（添加、移除）
//! - 查询邮件的标签列表、查询标签下的邮件 ID 列表
//! - 按账号批量删除标签及关联关系

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::labels;

/// 用于创建或更新标签的写入数据结构
///
/// 不包含自增主键 `id`，其余字段与 `labels` 表一一对应。
pub struct LabelWrite {
    pub account_id: i32,
    pub name: String,
    pub color: String,
    pub created_at: i64,
}

/// 将数据库行映射为 `labels::Model`
fn map_label(row: &rusqlite::Row<'_>) -> rusqlite::Result<labels::Model> {
    Ok(labels::Model {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        name: row.get("name")?,
        color: row.get("color")?,
        created_at: row.get("created_at")?,
    })
}

/// 查询指定账号下的所有标签，按 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `account_id`: 账号 ID
///
/// # 返回
///
/// 该账号的标签列表。
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

/// 根据 ID 查询标签。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 标签 ID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
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

/// 创建新标签并返回创建后的完整记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `model`: 待创建的标签数据
///
/// # 返回
///
/// 创建成功后的标签记录（包含自增 ID）。
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

/// 根据 ID 更新标签，返回更新后的完整记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 待更新的标签 ID
/// - `model`: 新的标签数据
///
/// # 返回
///
/// 更新后的标签记录。
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

/// 删除标签及其与邮件的关联关系。
///
/// 在事务中先删除 `email_labels` 关联，再删除标签本身。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 待删除的标签 ID
///
/// # 返回
///
/// 删除成功返回 `Ok(())`。
pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.transaction(move |tx| delete_tx(tx, id)).await
}

/// 在指定事务中删除标签及其关联（事务版本）
///
/// 先删除 `email_labels` 关联，再删除标签本身。
///
/// # 参数
///
/// - `tx`: 数据库事务
/// - `id`: 待删除的标签 ID
pub fn delete_tx(tx: &rusqlite::Transaction<'_>, id: i32) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM email_labels WHERE label_id = ?1", [id])?;
    tx.execute("DELETE FROM labels WHERE id = ?1", [id])?;
    Ok(())
}

/// 删除指定账号下的所有标签及其关联关系。
///
/// 在事务中级联清理 `email_labels` 关联表（通过邮件和标签两个维度），
/// 再删除该账号的所有标签。
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

/// 在指定事务中删除账号的所有标签及其关联（事务版本）
///
/// 先级联清理 `email_labels`（通过邮件 ID 和标签 ID 两个维度），
/// 再删除该账号的所有标签。
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
        "DELETE FROM email_labels
         WHERE email_id IN (SELECT id FROM emails WHERE account_id = ?1)
            OR label_id IN (SELECT id FROM labels WHERE account_id = ?1)",
        [account_id],
    )?;
    tx.execute("DELETE FROM labels WHERE account_id = ?1", [account_id])?;
    Ok(())
}

/// 查询指定邮件的所有标签。
///
/// 通过 `email_labels` 关联表 JOIN `labels` 表获取，按标签 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
///
/// # 返回
///
/// 该邮件的标签列表。
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

/// 为邮件添加标签（忽略已存在的重复关联）。
///
/// 使用 `INSERT OR IGNORE` 避免重复关联导致的唯一约束冲突。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
/// - `label_id`: 标签 ID
///
/// # 返回
///
/// 添加成功返回 `Ok(())`。
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

/// 移除邮件上的指定标签关联。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
/// - `label_id`: 标签 ID
///
/// # 返回
///
/// 移除成功返回 `Ok(())`。
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

/// 查询指定标签下的所有邮件 ID，按邮件 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `label_id`: 标签 ID
///
/// # 返回
///
/// 与该标签关联的邮件 ID 列表。
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
