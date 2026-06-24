//! 附件仓库模块（Attachment Repository）
//!
//! 负责 `attachments` 表的数据访问，管理邮件附件的存储和查询。
//! 支持单条插入、批量插入、按邮件查询和删除附件。
//!
//! 主要操作：
//! - 按邮件 ID 查询附件列表
//! - 创建、批量创建附件
//! - 根据 ID 查询单个附件
//! - 按邮件 ID 删除附件

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::attachments;

/// 批量插入附件时使用的 SQL 语句常量
pub const INSERT_ATTACHMENT_SQL: &str = "INSERT INTO attachments (
        email_id, filename, content_type, size, section_path, disposition, content_id, path, created_at
     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";

/// 用于创建附件的写入数据结构
///
/// 不包含自增主键 `id`，调用批量插入时需先插入邮件并获取 `email_id` 后再赋值。
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

/// 将数据库行映射为 `attachments::Model`
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

/// 在预编译语句上执行单次附件插入。
///
/// 供批量插入场景复用预编译语句，避免重复 prepare 带来的性能开销。
///
/// # 参数
///
/// - `stmt`: 已预编译的插入语句
/// - `model`: 待插入的附件数据
pub fn execute_attachment_insert(
    stmt: &mut rusqlite::Statement<'_>,
    model: &AttachmentWrite,
) -> rusqlite::Result<()> {
    stmt.execute(rusqlite::params![
        model.email_id,
        &model.filename,
        &model.content_type,
        model.size,
        &model.section_path,
        &model.disposition,
        &model.content_id,
        &model.path,
        model.created_at,
    ])?;
    Ok(())
}

/// 查询指定邮件的所有附件，按 ID 升序排列。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
///
/// # 返回
///
/// 该邮件的附件列表。
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

/// 创建单个附件并返回创建后的完整记录。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `model`: 待创建的附件数据
///
/// # 返回
///
/// 创建成功后的附件记录（包含自增 ID）。
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

/// 批量插入附件。
///
/// 在单个事务中复用预编译语句逐条插入，空列表直接返回。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `models`: 待插入的附件列表
///
/// # 返回
///
/// 插入成功返回 `Ok(())`。
pub async fn bulk_insert(db: &DbConn, models: Vec<AttachmentWrite>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }

    db.transaction(move |tx| {
        let mut stmt = tx.prepare(INSERT_ATTACHMENT_SQL)?;
        for model in models {
            execute_attachment_insert(&mut stmt, &model)?;
        }
        Ok(())
    })
    .await
}

/// 根据 ID 查询附件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `id`: 附件 ID
///
/// # 返回
///
/// 找到则返回 `Some(model)`，不存在则返回 `None`。
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

/// 删除指定邮件的所有附件。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `email_id`: 邮件 ID
///
/// # 返回
///
/// 删除成功返回 `Ok(())`。
pub async fn delete_by_email(db: &DbConn, email_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM attachments WHERE email_id = ?1", [email_id])?;
        Ok(())
    })
    .await
}
