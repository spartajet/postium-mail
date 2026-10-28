//! 联系人仓库模块（Contact Repository）
//!
//! 负责 `contacts` 表及其关联表 `contact_groups` / `contact_group_members`
//! 的数据访问，提供联系人的完整生命周期管理：
//!
//! - 查询：分页列表（关键词/分组/星标过滤）、按 ID / 邮箱查询单条
//! - 写入：创建（同邮箱大小写不敏感去重）、更新
//! - 状态管理：星标切换
//! - 删除：物理删除（分组成员关系随外键级联清理）

use crate::error::MailError;
use crate::infrastructure::storage::database::DbConn;
use serde::{Deserialize, Serialize};
use specta::Type;

/// 联系人 DTO（完整展示视图）
///
/// `extra_emails` 与 `phones` 在数据库中以 JSON 字符串存储，读出时反序列化为列表。
/// `groups` 由仓储在查询后通过第二条 SQL 批量组装，避免 N+1。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContactDto {
    pub id: i32,
    pub email: String,
    pub extra_emails: Vec<String>,
    pub name: Option<String>,
    pub phones: Vec<String>,
    pub notes: Option<String>,
    pub org: Option<String>,
    pub is_starred: bool,
    pub frequency: i64,
    pub last_contacted_at: Option<i64>,
    pub source: String,
    pub dav_account_id: Option<i32>,
    pub dav_uid: Option<String>,
    pub dav_href: Option<String>,
    pub dav_etag: Option<String>,
    pub groups: Vec<ContactGroupDto>,
}

/// 联系人分组 DTO
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContactGroupDto {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
}

/// 联系人分页列表响应
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContactListResponse {
    pub contacts: Vec<ContactDto>,
    pub total: u64,
    pub page: usize,
    pub limit: usize,
}

/// 用于创建或更新联系人的写入数据结构
///
/// 不包含主键与系统字段（星标/频次/来源/CardDAV 元数据等），仅承载用户可编辑内容。
/// `extra_emails` / `phones` 为 `None` 时表示清空该字段。
#[derive(Debug, Clone, Default)]
pub struct ContactWrite {
    pub email: String,
    pub extra_emails: Option<Vec<String>>,
    pub name: Option<String>,
    pub phones: Option<Vec<String>>,
    pub notes: Option<String>,
    pub org: Option<String>,
}

/// 联系人行查询的公共列清单
const CONTACT_COLUMNS: &str = "id, email, extra_emails, name, phones, notes, org,
       is_starred, frequency, last_contacted_at, source,
       dav_account_id, dav_uid, dav_href, dav_etag";

/// 将数据库行映射为 `ContactDto`
///
/// `extra_emails` / `phones` 从 JSON 字符串反序列化，`NULL`、空串或解析失败均回退为空列表。
/// `groups` 初始为空，由 [`attach_groups`] 批量填充。
fn map_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<ContactDto> {
    Ok(ContactDto {
        id: row.get("id")?,
        email: row.get("email")?,
        extra_emails: parse_json_strings(row.get("extra_emails")?),
        name: row.get("name")?,
        phones: parse_json_strings(row.get("phones")?),
        notes: row.get("notes")?,
        org: row.get("org")?,
        is_starred: row.get::<_, i64>("is_starred")? != 0,
        frequency: row.get("frequency")?,
        last_contacted_at: row.get("last_contacted_at")?,
        source: row.get("source")?,
        dav_account_id: row.get("dav_account_id")?,
        dav_uid: row.get("dav_uid")?,
        dav_href: row.get("dav_href")?,
        dav_etag: row.get("dav_etag")?,
        groups: Vec::new(),
    })
}

/// 将 JSON 字符串列解析为 `Vec<String>`，失败时返回空列表
fn parse_json_strings(raw: Option<String>) -> Vec<String> {
    raw.as_deref()
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

/// 将字符串列表序列化为 JSON 字符串；空列表序列化为 `NULL`（与读取侧的空默认对齐）
fn serialize_json_strings(values: Option<Vec<String>>) -> Result<Option<String>, MailError> {
    Ok(match values {
        Some(list) if !list.is_empty() => Some(serde_json::to_string(&list)?),
        _ => None,
    })
}

/// 生成指定数量的 SQL 占位符字符串，如 `?,?,?`（用于 `IN (...)` 子句）
fn placeholders(count: usize) -> String {
    std::iter::repeat("?")
        .take(count)
        .collect::<Vec<_>>()
        .join(",")
}

/// 判断错误是否为 UNIQUE 约束冲突（SQLITE_CONSTRAINT_UNIQUE，扩展码 2067）
fn is_unique_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(ffi, Some(msg))
            if ffi.code == rusqlite::ErrorCode::ConstraintViolation
                && (ffi.extended_code == 2067 || msg.contains("UNIQUE"))
    )
}

/// 为一批联系人批量组装所属分组（第二条 SQL，避免 N+1 查询）
///
/// 通过 `contact_group_members` JOIN `contact_groups` 一次取回所有联系人的分组，
/// 再按 `contact_id` 分发到各 DTO。
fn attach_groups(
    conn: &rusqlite::Connection,
    contacts: &mut [ContactDto],
) -> rusqlite::Result<()> {
    if contacts.is_empty() {
        return Ok(());
    }

    let ids: Vec<i32> = contacts.iter().map(|contact| contact.id).collect();
    let sql = format!(
        "SELECT m.contact_id, g.id, g.name, g.color
         FROM contact_groups g
         JOIN contact_group_members m ON m.group_id = g.id
         WHERE m.contact_id IN ({})
         ORDER BY g.id ASC",
        placeholders(ids.len())
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
        Ok((
            row.get::<_, i32>(0)?,
            ContactGroupDto {
                id: row.get(1)?,
                name: row.get(2)?,
                color: row.get(3)?,
            },
        ))
    })?;

    let mut groups_by_contact: std::collections::HashMap<i32, Vec<ContactGroupDto>> =
        std::collections::HashMap::new();
    for row in rows {
        let (contact_id, group) = row?;
        groups_by_contact.entry(contact_id).or_default().push(group);
    }
    for contact in contacts.iter_mut() {
        contact.groups = groups_by_contact.remove(&contact.id).unwrap_or_default();
    }
    Ok(())
}

/// 分页查询联系人。
///
/// 支持按关键词（匹配 `name` / `email` 的 `LIKE '%q%'`）、所属分组、
/// 仅星标三个过滤条件，可任意组合。
///
/// 排序规则：`is_starred DESC, frequency DESC, name ASC`（追加 `id ASC` 保证稳定）。
/// 返回前通过第二条 SQL 批量组装每个联系人的分组列表。
///
/// # 参数
///
/// - `db`: 数据库连接
/// - `query`: 关键词（`None` 或空串表示不过滤）
/// - `group_id`: 分组 ID（仅返回该分组成员）
/// - `starred_only`: 仅返回星标联系人
/// - `page`: 页码（从 1 开始，0 视为第 1 页）
/// - `limit`: 每页数量
pub async fn list(
    db: &DbConn,
    query: Option<&str>,
    group_id: Option<i32>,
    starred_only: bool,
    page: usize,
    limit: usize,
) -> Result<ContactListResponse, MailError> {
    let query = query.map(str::to_string);
    db.call(move |conn| {
        let mut clauses: Vec<&'static str> = Vec::new();
        let mut values: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(keyword) = &query {
            if !keyword.is_empty() {
                clauses.push("(name LIKE ? OR email LIKE ?)");
                let pattern = format!("%{keyword}%");
                values.push(pattern.clone().into());
                values.push(pattern.into());
            }
        }
        if let Some(group_id) = group_id {
            clauses.push(
                "id IN (SELECT contact_id FROM contact_group_members WHERE group_id = ?)",
            );
            values.push(group_id.into());
        }
        if starred_only {
            clauses.push("is_starred = 1");
        }

        let where_sql = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };

        let total = conn.query_row(
            &format!("SELECT COUNT(*) FROM contacts {where_sql}"),
            rusqlite::params_from_iter(values.iter()),
            |row| row.get::<_, i64>(0),
        )? as u64;

        let offset = page.saturating_sub(1).saturating_mul(limit) as i64;
        let select_sql = format!(
            "SELECT {CONTACT_COLUMNS}
             FROM contacts
             {where_sql}
             ORDER BY is_starred DESC, frequency DESC, name ASC, id ASC
             LIMIT ? OFFSET ?"
        );
        let mut select_values = values;
        select_values.push((limit as i64).into());
        select_values.push(offset.into());

        let mut stmt = conn.prepare(&select_sql)?;
        let mut contacts: Vec<ContactDto> = stmt
            .query_map(rusqlite::params_from_iter(select_values.iter()), map_contact)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        attach_groups(conn, &mut contacts)?;

        Ok(ContactListResponse {
            contacts,
            total,
            page,
            limit,
        })
    })
    .await
}

/// 根据 ID 查询联系人（含分组）。
///
/// 找到返回 `Some(dto)`，不存在返回 `None`。
pub async fn get(db: &DbConn, id: i32) -> Result<Option<ContactDto>, MailError> {
    db.call(move |conn| {
        let sql = format!("SELECT {CONTACT_COLUMNS} FROM contacts WHERE id = ?1");
        let mut contact = match conn.query_row(&sql, [id], map_contact) {
            Ok(contact) => Some(contact),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return Err(err),
        };
        if let Some(contact) = contact.as_mut() {
            attach_groups(conn, std::slice::from_mut(contact))?;
        }
        Ok(contact)
    })
    .await
}

/// 根据邮箱查询联系人（大小写不敏感，含分组）。
///
/// 邮箱在入库时已统一小写，此处仍以 `lower()` 双侧比较以兼容历史数据。
pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<ContactDto>, MailError> {
    let email = email.trim().to_lowercase();
    db.call(move |conn| {
        let sql = format!(
            "SELECT {CONTACT_COLUMNS} FROM contacts WHERE lower(email) = ?1"
        );
        let mut contact = match conn.query_row(&sql, [&email], map_contact) {
            Ok(contact) => Some(contact),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return Err(err),
        };
        if let Some(contact) = contact.as_mut() {
            attach_groups(conn, std::slice::from_mut(contact))?;
        }
        Ok(contact)
    })
    .await
}

/// 创建联系人，返回自增 ID。
///
/// 邮箱入库前统一小写化（配合 `idx_contacts_email` 上的 `lower(email)` 唯一索引）；
/// 同邮箱（忽略大小写）已存在时，UNIQUE 冲突被映射为
/// `MailError::DatabaseError("联系人已存在: ...")`。
pub async fn insert(db: &DbConn, write: ContactWrite) -> Result<i32, MailError> {
    let email = write.email.trim().to_lowercase();
    let extra_emails = serialize_json_strings(write.extra_emails)?;
    let phones = serialize_json_strings(write.phones)?;
    let duplicate_message = format!("联系人已存在: {email}");

    let inserted = db
        .call(move |conn| {
            match conn.execute(
                "INSERT INTO contacts (
                    email, extra_emails, name, phones, notes, org, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                rusqlite::params![
                    email,
                    extra_emails,
                    write.name,
                    phones,
                    write.notes,
                    write.org,
                    chrono::Utc::now().timestamp(),
                ],
            ) {
                Ok(_) => Ok(Some(conn.last_insert_rowid() as i32)),
                Err(err) if is_unique_violation(&err) => Ok(None),
                Err(err) => Err(err),
            }
        })
        .await?;

    inserted.ok_or_else(|| MailError::DatabaseError(duplicate_message))
}

/// 根据 ID 更新联系人的可编辑字段。
///
/// 覆盖 `email`（小写化）/`extra_emails`/`name`/`phones`/`notes`/`org` 并刷新
/// `updated_at`；星标、频次、来源与 CardDAV 元数据保持不变。ID 不存在时不报错。
pub async fn update(db: &DbConn, id: i32, write: ContactWrite) -> Result<(), MailError> {
    let email = write.email.trim().to_lowercase();
    let extra_emails = serialize_json_strings(write.extra_emails)?;
    let phones = serialize_json_strings(write.phones)?;

    db.call(move |conn| {
        conn.execute(
            "UPDATE contacts
             SET email = ?1, extra_emails = ?2, name = ?3, phones = ?4,
                 notes = ?5, org = ?6, updated_at = ?7
             WHERE id = ?8",
            rusqlite::params![
                email,
                extra_emails,
                write.name,
                phones,
                write.notes,
                write.org,
                chrono::Utc::now().timestamp(),
                id,
            ],
        )?;
        Ok(())
    })
    .await
}

/// 根据 ID 删除联系人。
///
/// 所属分组成员关系由外键 `ON DELETE CASCADE` 自动清理。
pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM contacts WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
}

/// 设置联系人的星标状态。
pub async fn set_starred(db: &DbConn, id: i32, starred: bool) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute(
            "UPDATE contacts SET is_starred = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![
                if starred { 1 } else { 0 },
                chrono::Utc::now().timestamp(),
                id,
            ],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn crud_roundtrip_and_dedup() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let id = insert(&db, ContactWrite { email: "A@Postium.Test".into(), name: Some("张三".into()), ..Default::default() }).await.unwrap();
        assert!(insert(&db, ContactWrite { email: "a@postium.test".into(), ..Default::default() }).await.is_err(), "大小写变体应去重");
        let dto = get(&db, id).await.unwrap().unwrap();
        assert_eq!(dto.email, "a@postium.test");
        assert_eq!(dto.name.as_deref(), Some("张三"));
        assert_eq!(list(&db, Some("张"), None, false, 0, 20).await.unwrap().total, 1);
        assert_eq!(list(&db, Some("a@po"), None, false, 0, 20).await.unwrap().total, 1);
        set_starred(&db, id, true).await.unwrap();
        assert_eq!(list(&db, None, None, true, 0, 20).await.unwrap().total, 1);
        delete(&db, id).await.unwrap();
        assert_eq!(list(&db, None, None, false, 0, 20).await.unwrap().total, 0);
    }
}
