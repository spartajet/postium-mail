//! 联系人仓库模块（Contact Repository）
//!
//! 负责 `contacts` 表及其关联表 `contact_groups` / `contact_group_members`
//! 的数据访问，提供联系人的完整生命周期管理：
//!
//! - 查询：分页列表（关键词/分组/星标过滤）、按 ID / 邮箱查询单条
//! - 写入：创建（同邮箱大小写不敏感去重）、更新、合并（N→1，[`merge`]）
//! - 状态管理：星标切换
//! - 分组管理：[`group_insert`] / [`group_rename`] / [`group_delete`] /
//!   [`group_list`]（含 LEFT JOIN 成员计数）、[`group_set_members`]
//!   （事务内全量替换联系人的分组成员关系）
//! - 自动收集：[`upsert_from_emails`] 批量并入往来联系人、
//!   [`backfill_from_emails`] 存量邮件聚合回填（均幂等，用户手工 /
//!   CardDAV 同步数据优先于自动收集）
//! - 删除：物理删除（分组成员关系随外键级联清理）

use crate::error::MailError;
use rusqlite::OptionalExtension as _;
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

/// 分组列表项（含成员计数）
///
/// `count` 由 [`group_list`] 通过 LEFT JOIN `contact_group_members` 聚合，
/// 无成员的分组计数为 0（LEFT JOIN 保证分组本身仍出现在结果中）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ContactGroupWithCount {
    pub id: i32,
    pub name: String,
    pub color: Option<String>,
    pub count: u64,
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
/// 所属分组成员关系由外键 `ON DELETE CASCADE` 自动清理。删除的条目若带
/// CardDAV 归属（`dav_account_id` + `dav_href`），写入 `dav_deletes` 墓碑
/// 供同步推送阶段向服务器发 DELETE（同 href 重删以最新时间覆盖）。
pub async fn delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        let existing: Option<(Option<i32>, Option<String>)> = conn
            .query_row(
                "SELECT dav_account_id, dav_href FROM contacts WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (account_id, href) = existing.unwrap_or((None, None));
        conn.execute("DELETE FROM contacts WHERE id = ?1", [id])?;
        if let (Some(account_id), Some(href)) = (account_id, href) {
            conn.execute(
                "INSERT INTO dav_deletes (account_id, href, deleted_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(account_id, href) DO UPDATE SET deleted_at = excluded.deleted_at",
                rusqlite::params![account_id, href, chrono::Utc::now().timestamp()],
            )?;
        }
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

/// 创建联系人分组，返回自增 ID。
///
/// 名称首尾空白去除；与现有分组重名（`UNIQUE(name)` 冲突）被映射为
/// `MailError::DatabaseError("分组已存在: ...")`（与 [`insert`] 的重复映射风格一致）。
pub async fn group_insert(db: &DbConn, name: &str) -> Result<i32, MailError> {
    let name = name.trim().to_string();
    let duplicate_message = format!("分组已存在: {name}");

    let inserted = db
        .call(move |conn| {
            match conn.execute("INSERT INTO contact_groups (name) VALUES (?1)", [&name]) {
                Ok(_) => Ok(Some(conn.last_insert_rowid() as i32)),
                Err(err) if is_unique_violation(&err) => Ok(None),
                Err(err) => Err(err),
            }
        })
        .await?;

    inserted.ok_or_else(|| MailError::DatabaseError(duplicate_message))
}

/// 重命名分组。
///
/// 名称首尾空白去除；改为已有分组名称时 `UNIQUE(name)` 冲突同样映射为
/// `MailError::DatabaseError("分组已存在: ...")`。ID 不存在时不报错（影响 0 行）。
pub async fn group_rename(db: &DbConn, id: i32, name: &str) -> Result<(), MailError> {
    let name = name.trim().to_string();
    let duplicate_message = format!("分组已存在: {name}");

    let outcome = db
        .call(move |conn| {
            match conn.execute(
                "UPDATE contact_groups SET name = ?2 WHERE id = ?1",
                rusqlite::params![id, name],
            ) {
                Ok(_) => Ok(None),
                Err(err) if is_unique_violation(&err) => Ok(Some(duplicate_message)),
                Err(err) => Err(err),
            }
        })
        .await?;

    match outcome {
        Some(message) => Err(MailError::DatabaseError(message)),
        None => Ok(()),
    }
}

/// 删除分组。
///
/// 分组成员关系由外键 `ON DELETE CASCADE` 自动清理，成员联系人本身不受影响。
pub async fn group_delete(db: &DbConn, id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM contact_groups WHERE id = ?1", [id])?;
        Ok(())
    })
    .await
}

/// 查询全部分组（按 `id` 升序），含各分组成员计数。
///
/// 计数通过 LEFT JOIN `contact_group_members` 聚合（`COUNT(m.contact_id)`），
/// 无成员的分组返回 `count = 0` 而非被过滤掉。
pub async fn group_list(db: &DbConn) -> Result<Vec<ContactGroupWithCount>, MailError> {
    db.call(move |conn| {
        let mut stmt = conn.prepare(
            "SELECT g.id, g.name, g.color, COUNT(m.contact_id) AS member_count
             FROM contact_groups g
             LEFT JOIN contact_group_members m ON m.group_id = g.id
             GROUP BY g.id, g.name, g.color
             ORDER BY g.id ASC",
        )?;
        let groups = stmt
            .query_map([], |row| {
                Ok(ContactGroupWithCount {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    color: row.get("color")?,
                    count: row.get::<_, i64>("member_count")? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(groups)
    })
    .await
}

/// 全量替换联系人的分组成员关系。
///
/// 单事务内执行：先 `DELETE` 该联系人的**全部**成员关系，再按 `group_ids`
/// 去重（排序 + dedup）后以 `INSERT OR IGNORE` 批量插入（主键
/// `(contact_id, group_id)` 兜底防重）。任一步失败整体回滚；
/// 传空切片等效于把联系人移出所有分组。
pub async fn group_set_members(
    db: &DbConn,
    contact_id: i32,
    group_ids: &[i32],
) -> Result<(), MailError> {
    let mut group_ids = group_ids.to_vec();
    group_ids.sort_unstable();
    group_ids.dedup();

    db.transaction(move |tx| {
        tx.execute(
            "DELETE FROM contact_group_members WHERE contact_id = ?1",
            [contact_id],
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO contact_group_members (contact_id, group_id)
                 VALUES (?1, ?2)",
            )?;
            for group_id in &group_ids {
                stmt.execute(rusqlite::params![contact_id, group_id])?;
            }
        }
        Ok(())
    })
    .await
}

/// 合并联系人（N→1）：把 `absorbed_ids` 的信息并入 `primary_id` 后物理删除 absorbed。
///
/// 单事务内完成（任一步失败整体回滚），成功后刷新 primary 的 `updated_at`，
/// 返回合并后的完整 [`ContactDto`]（含分组）。
///
/// 合并规则：
///
/// - `extra_emails`：primary 与各 absorbed 的额外邮箱并集，忽略大小写与首尾
///   空白去重（保留首次出现的原始写法），并排除 primary 主邮箱
/// - `frequency`：求和；`last_contacted_at`：取各方最大值（全空保持 `NULL`）
/// - 分组：并集——absorbed 的成员关系以 `INSERT OR IGNORE` 转移给 primary
/// - `notes`：所有非空备注按 primary 在前的顺序以 `\n\n` 拼接，全空为 `NULL`
/// - `name` / `org`：取 primary 非空值；primary 为空时取首个非空 absorbed 值
/// - CardDAV 元数据（`dav_account_id` / `dav_uid` / `dav_href` / `dav_etag`）：
///   primary 为 `NULL` 的字段由 absorbed（按入参顺序）的首个非空值补齐，
///   已有值不覆盖。服务端资源删除由上层同步层负责（任务 18 墓碑表方案），
///   本函数仅做本地字段的尽力保留
/// - `phones` / `is_starred` / `source` / 主邮箱 / `created_at`：保持 primary 原值
///
/// 防御（返回 [`MailError::DatabaseError`] 且不做任何修改）：
///
/// - `absorbed_ids` 为空
/// - `absorbed_ids` 包含 `primary_id`
/// - `absorbed_ids` 含重复 ID
/// - `primary_id` 或任一 absorbed ID 不存在
pub async fn merge(
    db: &DbConn,
    primary_id: i32,
    absorbed_ids: &[i32],
) -> Result<ContactDto, MailError> {
    let absorbed_ids = absorbed_ids.to_vec();
    let outcome = db
        .transaction(move |tx| {
            type TxResult = Result<ContactDto, String>;
            let fail = |message: String| -> Result<TxResult, rusqlite::Error> {
                Ok(Err(message))
            };

            if absorbed_ids.is_empty() {
                return fail("合并列表不能为空".to_string());
            }
            if absorbed_ids.contains(&primary_id) {
                return fail(format!("合并列表不能包含主联系人: {primary_id}"));
            }
            let mut sorted = absorbed_ids.clone();
            sorted.sort_unstable();
            if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                return fail("合并列表含重复联系人".to_string());
            }

            let select_sql = format!("SELECT {CONTACT_COLUMNS} FROM contacts WHERE id = ?1");
            let load = |id: i32| -> rusqlite::Result<Option<ContactDto>> {
                match tx.query_row(&select_sql, [id], map_contact) {
                    Ok(contact) => Ok(Some(contact)),
                    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                    Err(err) => Err(err),
                }
            };

            let primary = match load(primary_id)? {
                Some(contact) => contact,
                None => return fail(format!("联系人不存在: {primary_id}")),
            };
            let mut absorbed: Vec<ContactDto> = Vec::with_capacity(absorbed_ids.len());
            for id in &absorbed_ids {
                match load(*id)? {
                    Some(contact) => absorbed.push(contact),
                    None => return fail(format!("联系人不存在: {id}")),
                }
            }

            // extra_emails 并集：忽略大小写/首尾空白去重，排除 primary 主邮箱
            let mut seen: std::collections::HashSet<String> =
                std::iter::once(primary.email.trim().to_lowercase()).collect();
            let mut extra_emails: Vec<String> = Vec::new();
            for email in primary
                .extra_emails
                .iter()
                .chain(absorbed.iter().flat_map(|contact| contact.extra_emails.iter()))
            {
                let key = email.trim().to_lowercase();
                if !key.is_empty() && seen.insert(key) {
                    extra_emails.push(email.trim().to_string());
                }
            }
            let extra_emails_json = serde_json::to_string(&extra_emails)
                .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;

            // notes：非空备注以 "\n\n" 拼接（primary 在前）
            let notes_parts: Vec<&str> = std::iter::once(&primary)
                .chain(absorbed.iter())
                .filter_map(|contact| contact.notes.as_deref())
                .filter(|notes| !notes.trim().is_empty())
                .collect();
            let notes = (!notes_parts.is_empty()).then(|| notes_parts.join("\n\n"));

            // name / org：primary 非空优先，否则取首个非空 absorbed 值
            let non_empty = |value: &Option<String>| -> Option<String> {
                value
                    .as_deref()
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_string)
            };
            let name = non_empty(&primary.name)
                .or_else(|| absorbed.iter().find_map(|contact| non_empty(&contact.name)));
            let org = non_empty(&primary.org)
                .or_else(|| absorbed.iter().find_map(|contact| non_empty(&contact.org)));

            // frequency 求和；last_contacted_at 取最大值（全 NULL 保持 NULL）
            let frequency = primary.frequency
                + absorbed.iter().map(|contact| contact.frequency).sum::<i64>();
            let last_contacted_at = std::iter::once(&primary)
                .chain(absorbed.iter())
                .filter_map(|contact| contact.last_contacted_at)
                .max();

            // dav 元数据：primary 为 NULL 的字段由 absorbed 首个非空值补齐，不覆盖已有值
            let mut dav_account_id = primary.dav_account_id;
            let mut dav_uid = primary.dav_uid.clone();
            let mut dav_href = primary.dav_href.clone();
            let mut dav_etag = primary.dav_etag.clone();
            for contact in &absorbed {
                if dav_account_id.is_none() {
                    dav_account_id = contact.dav_account_id;
                }
                if dav_uid.is_none() {
                    dav_uid = contact.dav_uid.clone();
                }
                if dav_href.is_none() {
                    dav_href = contact.dav_href.clone();
                }
                if dav_etag.is_none() {
                    dav_etag = contact.dav_etag.clone();
                }
            }

            // 1) UPDATE primary 各字段（含 updated_at 刷新）
            tx.execute(
                "UPDATE contacts
                 SET extra_emails = ?1, name = ?2, notes = ?3, org = ?4,
                     frequency = ?5, last_contacted_at = ?6,
                     dav_account_id = ?7, dav_uid = ?8, dav_href = ?9, dav_etag = ?10,
                     updated_at = ?11
                 WHERE id = ?12",
                rusqlite::params![
                    extra_emails_json,
                    name,
                    notes,
                    org,
                    frequency,
                    last_contacted_at,
                    dav_account_id,
                    dav_uid,
                    dav_href,
                    dav_etag,
                    chrono::Utc::now().timestamp(),
                    primary_id,
                ],
            )?;

            // 2) 分组并集：absorbed 的成员关系转移给 primary（主键冲突忽略）
            {
                let mut stmt = tx.prepare(
                    "INSERT OR IGNORE INTO contact_group_members (contact_id, group_id)
                     SELECT ?1, group_id FROM contact_group_members WHERE contact_id = ?2",
                )?;
                for id in &absorbed_ids {
                    stmt.execute(rusqlite::params![primary_id, id])?;
                }
            }

            // 3) absorbed 物理删除（成员关系随外键级联清理）
            tx.execute(
                &format!(
                    "DELETE FROM contacts WHERE id IN ({})",
                    placeholders(absorbed_ids.len())
                ),
                rusqlite::params_from_iter(absorbed_ids.iter()),
            )?;

            // 事务内读回合并后的完整 DTO（含分组）
            let mut merged = match load(primary_id)? {
                Some(contact) => contact,
                None => return fail(format!("联系人不存在: {primary_id}")),
            };
            attach_groups(tx, std::slice::from_mut(&mut merged))?;
            Ok(Ok(merged))
        })
        .await?;

    outcome.map_err(MailError::DatabaseError)
}

/// 自动收集管线的输入条目：从一封往来邮件中提取出的对方地址与相关信息。
///
/// 由邮件收发/同步流程构造，交给 [`upsert_from_emails`] 批量并入联系人表。
#[derive(Debug, Clone)]
pub struct IncomingContact {
    /// 对方邮箱地址（并入前统一小写化，配合 `lower(email)` 唯一索引）
    pub email: String,
    /// 邮件中携带的对方名称（如 `sender_name`），可能为空
    pub name: Option<String>,
    /// 本次往来发生的时间戳（Unix 秒）
    pub contacted_at: i64,
}

/// 存量回填的聚合 SQL：展开 `emails` 表的 sender / recipient / cc 三列，
/// 按小写邮箱分组统计频次、名称与最近往来时间。
///
/// - recipient / cc 两列以逗号分隔存储，拼接成 JSON 字符串数组后用 `json_each`
///   展开（元素需带双引号才是合法 JSON：`,`→`","`）；展开前把 `;` 统一替换为
///   `,` 并去除空白。空串拼接为 `[""]`，由外层 `addr!=''` 过滤。
/// - `is_deleted=1`（回收站）邮件与空地址不参与聚合。
/// - token 可能是显示格式（`Name<a@x>`，空白已在 SQL 中去除）：Rust 侧并入
///   前统一归一为裸地址（见 [`backfill_from_emails`]）。
const BACKFILL_AGGREGATE_SQL: &str = "
SELECT lower(addr) AS email, MAX(name) AS name, COUNT(*) AS freq, MAX(sent_at) AS last_at FROM (
  SELECT sender_email AS addr, COALESCE(sender_name,'') AS name, sent_at FROM emails WHERE is_deleted=0
  UNION ALL
  SELECT j.value AS addr, '' AS name, e.sent_at FROM emails e,
    json_each('[\"'||replace(replace(replace(e.recipient_emails,';',','),' ',''),',','\",\"')||'\"]') j
    WHERE e.is_deleted=0
  UNION ALL
  SELECT j.value AS addr, '' AS name, e.sent_at FROM emails e,
    json_each('[\"'||replace(replace(replace(e.cc_emails,';',','),' ',''),',','\",\"')||'\"]') j
    WHERE e.is_deleted=0 AND e.cc_emails IS NOT NULL AND e.cc_emails!=''
) WHERE addr!='' GROUP BY lower(addr)";

/// 将收集/聚合到的一个联系人并入 `contacts` 表（事务内逐条调用）。
///
/// 邮箱忽略大小写匹配（`WHERE lower(email) = ?`，配合唯一索引 `idx_contacts_email`）。
/// 并入规则：
///
/// - **未命中**：插入 `source='collected'` 新条目，`frequency` 取
///   `aggregate_frequency.unwrap_or(1)`，`last_contacted_at = contacted_at`，
///   `name` 有值时带上。
/// - **命中 `source='collected'`**：
///     - `aggregate_frequency = Some(freq)`（backfill 覆盖模式）：`frequency = freq`；
///     - `aggregate_frequency = None`（upsert 增量模式）：`frequency += 1`；
///     - 两种模式下 `last_contacted_at` 均推进到与现有值的较大者，
///       `name` 仅在现有值为空（`NULL` 或空串）时补充，不覆盖已有名称。
/// - **命中 `source IN ('local','dav')`**：仅推进 `last_contacted_at`，
///   `frequency` 与 `name` 一概不动（用户手工创建 / CardDAV 同步的数据优先）。
///
/// # 参数
///
/// - `tx`: 所属事务
/// - `email`: 对方邮箱（内部小写化）
/// - `name`: 邮件中携带的名称（空串视为无名称）
/// - `contacted_at`: 本次往来时间戳
/// - `aggregate_frequency`: `Some(freq)` 表示用聚合频次整体覆盖（backfill），
///   `None` 表示逐次累加（upsert）
fn merge_contact(
    tx: &rusqlite::Transaction<'_>,
    email: &str,
    name: Option<&str>,
    contacted_at: i64,
    aggregate_frequency: Option<i64>,
) -> rusqlite::Result<()> {
    let email = email.trim().to_lowercase();
    let name = name.filter(|name| !name.trim().is_empty());

    let existing: Option<(i32, String)> = match tx.query_row(
        "SELECT id, source FROM contacts WHERE lower(email) = ?1",
        [&email],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ) {
        Ok(row) => Some(row),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(err) => return Err(err),
    };

    match existing {
        None => {
            tx.execute(
                "INSERT INTO contacts (
                    email, name, frequency, last_contacted_at, source, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 'collected', ?5, ?5)",
                rusqlite::params![
                    email,
                    name,
                    aggregate_frequency.unwrap_or(1),
                    contacted_at,
                    chrono::Utc::now().timestamp(),
                ],
            )?;
        }
        Some((id, source)) if source == "collected" => {
            // `name = COALESCE(NULLIF(name, ''), ?)`：现有名称为空（NULL/空串）才补
            let (frequency_sql, frequency_value) = match aggregate_frequency {
                Some(freq) => ("?2", rusqlite::types::Value::Integer(freq)),
                None => ("frequency + 1", rusqlite::types::Value::Null),
            };
            tx.execute(
                &format!(
                    "UPDATE contacts
                     SET frequency = {frequency_sql},
                         last_contacted_at = MAX(COALESCE(last_contacted_at, ?3), ?3),
                         name = COALESCE(NULLIF(name, ''), ?4),
                         updated_at = ?5
                     WHERE id = ?1"
                ),
                rusqlite::params![id, frequency_value, contacted_at, name, chrono::Utc::now().timestamp()],
            )?;
        }
        Some((id, _)) => {
            // local / dav 条目：仅推进最近往来时间
            tx.execute(
                "UPDATE contacts
                 SET last_contacted_at = MAX(COALESCE(last_contacted_at, ?2), ?2)
                 WHERE id = ?1",
                rusqlite::params![id, contacted_at],
            )?;
        }
    }
    Ok(())
}

/// 批量并入自动收集到的联系人（单事务）。
///
/// 对每个 [`IncomingContact`] 逐条套用 [`merge_contact`] 的增量规则：
/// 命中 `collected` 条目则 `frequency += 1` 并推进 `last_contacted_at`（name 为空才补）；
/// 命中 `local` / `dav` 条目则只推进 `last_contacted_at`；未命中则插入
/// `source='collected'`（`frequency` 初始为 1，name 有值带上）。
/// 任一条目失败则整个事务回滚。地址为空白的条目被跳过。
pub async fn upsert_from_emails(
    db: &DbConn,
    items: &[IncomingContact],
) -> Result<(), MailError> {
    let items = items.to_vec();
    db.transaction(move |tx| {
        for item in &items {
            if item.email.trim().is_empty() {
                continue;
            }
            merge_contact(
                tx,
                &item.email,
                item.name.as_deref(),
                item.contacted_at,
                None,
            )?;
        }
        Ok(())
    })
    .await
}

/// 存量回填：聚合 `emails` 表的 sender / recipient / cc 三列（见
/// [`BACKFILL_AGGREGATE_SQL`]），把聚合结果逐行按 [`merge_contact`] 的
/// 覆盖规则并入联系人表（单事务）。
///
/// recipient / cc 列可能存显示格式（`"Name <a@x>"`，SQL 展开后成
/// `Name<a@x>` 单 token），因此并入前对每个 token 复用
/// [`split_display_addresses`](crate::domain::sync::split_display_addresses)
/// 的尖括号提取规则归一为裸地址，并跨 token 重新聚合（频次求和、时间取
/// 最大），同一地址的裸/显示两种形态合并为一条。
///
/// 幂等性：`collected` 条目的 `frequency` 用聚合值**整体覆盖**（而非累加），
/// `last_contacted_at` 只会前进；`local` / `dav` 条目仅推进 `last_contacted_at`。
/// 因此重复执行结果稳定。
///
/// # 返回
///
/// 聚合并处理的联系人地址数量（归一去重后，不含被跳过的空地址）。
pub async fn backfill_from_emails(db: &DbConn) -> Result<usize, MailError> {
    db.transaction(move |tx| {
        struct Aggregated {
            email: String,
            name: Option<String>,
            freq: i64,
            last_at: i64,
        }

        let aggregated: Vec<Aggregated> = {
            let mut stmt = tx.prepare(BACKFILL_AGGREGATE_SQL)?;
            let rows = stmt.query_map([], |row| {
                Ok(Aggregated {
                    email: row.get("email")?,
                    name: row.get::<_, Option<String>>("name")?,
                    freq: row.get("freq")?,
                    last_at: row.get("last_at")?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        // 归一再聚合：token → 裸地址后同址合并（频次求和、时间取最大、
        // 名称取首个非空）。保留 SQL 行序（索引表），保证处理顺序确定。
        let mut index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut merged: Vec<(String, Option<String>, i64, i64)> = Vec::new();
        for item in &aggregated {
            // SQL 展开后 token 内已无逗号/空白，归一结果至多一个地址
            let Some(email) =
                crate::domain::sync::split_display_addresses(&item.email).into_iter().next()
            else {
                continue;
            };
            let name = item.name.as_deref().filter(|name| !name.trim().is_empty());
            match index.get(&email).copied() {
                Some(i) => {
                    let (_, merged_name, freq, last_at) = &mut merged[i];
                    if merged_name.is_none() {
                        *merged_name = name.map(str::to_string);
                    }
                    *freq += item.freq;
                    *last_at = (*last_at).max(item.last_at);
                }
                None => {
                    index.insert(email.clone(), merged.len());
                    merged.push((email, name.map(str::to_string), item.freq, item.last_at));
                }
            }
        }

        let mut count = 0;
        for (email, name, freq, last_at) in &merged {
            merge_contact(tx, email, name.as_deref(), *last_at, Some(*freq))?;
            count += 1;
        }
        Ok(count)
    })
    .await
}


/// CardDAV 绑定 DTO（不含密码——密码经 AuthManager 托管，键 `dav:{account_id}`）。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DavBindingDto {
    pub id: i32,
    pub account_id: i32,
    pub url: String,
    pub username: String,
    pub addressbook_href: Option<String>,
    pub sync_token: Option<String>,
    pub last_sync_at: Option<i64>,
}

/// 行映射。
fn binding_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DavBindingDto> {
    Ok(DavBindingDto {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        url: row.get("url")?,
        username: row.get("username")?,
        addressbook_href: row.get("addressbook_href")?,
        sync_token: row.get("sync_token")?,
        last_sync_at: row.get("last_sync_at")?,
    })
}

/// 新增或覆盖账号的 CardDAV 绑定（UNIQUE account_id；密码不入库）。
/// 账号不存在时 FK 约束报错。
pub async fn binding_upsert(
    db: &DbConn,
    account_id: i32,
    url: &str,
    username: &str,
    addressbook_href: Option<&str>,
) -> Result<(), MailError> {
    let url = url.to_string();
    let username = username.to_string();
    let href = addressbook_href.map(str::to_string);
    db.call(move |conn| {
        conn.execute(
            "INSERT INTO dav_bindings (account_id, url, username, addressbook_href)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(account_id) DO UPDATE SET
                url = excluded.url,
                username = excluded.username,
                addressbook_href = excluded.addressbook_href",
            rusqlite::params![account_id, url, username, href],
        )?;
        Ok(())
    })
    .await
}

/// 全部绑定。
pub async fn binding_get_all(db: &DbConn) -> Result<Vec<DavBindingDto>, MailError> {
    db.call(|conn| {
        Ok(conn
            .prepare("SELECT * FROM dav_bindings ORDER BY account_id")?
            .query_map([], binding_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    })
    .await
}

/// 按账号取绑定。
pub async fn binding_get(db: &DbConn, account_id: i32) -> Result<Option<DavBindingDto>, MailError> {
    db.call(move |conn| {
        Ok(conn
            .query_row(
                "SELECT * FROM dav_bindings WHERE account_id = ?1",
                [account_id],
                binding_from_row,
            )
            .optional()?)
    })
    .await
}

/// 删除账号绑定。
pub async fn binding_delete(db: &DbConn, account_id: i32) -> Result<(), MailError> {
    db.call(move |conn| {
        conn.execute("DELETE FROM dav_bindings WHERE account_id = ?1", [account_id])?;
        Ok(())
    })
    .await
}

/// 同步完成回写：sync-token 与时间戳。
pub async fn binding_touch_synced(
    db: &DbConn,
    account_id: i32,
    sync_token: Option<String>,
) -> Result<(), MailError> {
    let now = chrono::Utc::now().timestamp();
    db.call(move |conn| {
        conn.execute(
            "UPDATE dav_bindings SET sync_token = ?1, last_sync_at = ?2 WHERE account_id = ?3",
            rusqlite::params![sync_token, now, account_id],
        )?;
        Ok(())
    })
    .await
}

// ── CardDAV 同步引擎所需查询/落库 ────────────────────────────────────────────

/// 拉取脏条目（推送候选）：`updated_at > since` 且 source 为 local/dav
/// （collected 从不推送），且 CardDAV 归属为空或属于该账号。
#[derive(Debug, Clone)]
pub struct DavDirtyContact {
    pub contact: ContactDto,
    /// 变更时间戳（412 冲突时与服务器 REV 比较）
    pub updated_at: i64,
    /// 服务器原卡文本（dav 条目合并推送的基底；local 条目为 None）
    pub raw_vcard: Option<String>,
}

/// 按账号 + UID 查找 dav 条目。
pub async fn find_by_uid(
    db: &DbConn,
    account_id: i32,
    uid: &str,
) -> Result<Option<ContactDto>, MailError> {
    let account = account_id;
    let uid = uid.to_string();
    db.call(move |conn| {
        conn.query_row(
            &format!(
                "SELECT {CONTACT_COLUMNS} FROM contacts WHERE dav_account_id = ?1 AND dav_uid = ?2"
            ),
            rusqlite::params![account, uid],
            map_contact,
        )
        .optional()
        .map_err(Into::into)
    })
    .await
}

/// 账号名下的全部 dav 条目（服务器消失检测用）。
pub async fn dav_contacts_for_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<ContactDto>, MailError> {
    let account = account_id;
    db.call(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {CONTACT_COLUMNS} FROM contacts
             WHERE source = 'dav' AND dav_account_id = ?1"
        ))?;
        let mut rows = stmt
            .query_map([account], map_contact)?
            .collect::<Result<Vec<_>, _>>()?;
        attach_groups(conn, &mut rows)?;
        Ok(rows)
    })
    .await
    .map_err(Into::into)
}

/// 推送候选（见 [`DavDirtyContact`]）。
pub async fn list_dav_dirty(
    db: &DbConn,
    account_id: i32,
    since: i64,
) -> Result<Vec<DavDirtyContact>, MailError> {
    let account = account_id;
    db.call(move |conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT {CONTACT_COLUMNS}, updated_at, raw_vcard FROM contacts
             WHERE updated_at > ?1
               AND source IN ('local', 'dav')
               AND (dav_account_id IS NULL OR dav_account_id = ?2)"
        ))?;
        let mut rows = stmt
            .query_map(rusqlite::params![since, account], |row| {
                let contact = map_contact(row)?;
                Ok(DavDirtyContact {
                    contact,
                    updated_at: row.get("updated_at")?,
                    raw_vcard: row.get("raw_vcard")?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut dtos: Vec<ContactDto> = rows.iter().map(|d| d.contact.clone()).collect();
        attach_groups(conn, &mut dtos)?;
        for (dirty, dto) in rows.iter_mut().zip(dtos) {
            dirty.contact = dto;
        }
        Ok(rows)
    })
    .await
    .map_err(Into::into)
}

/// 拉取落库（已有条目）：结构化字段跟随服务器卡片 + CardDAV 元数据 + raw_vcard。
///
/// `sync_ts` 为本次同步游标（binding 上次 last_sync_at）：写入该值而非
/// `now`，使拉到的行不会被本账号下一轮推送当作脏条目回推。
#[allow(clippy::too_many_arguments)]
pub async fn dav_pull_apply(
    db: &DbConn,
    id: i32,
    write: &ContactWrite,
    account_id: i32,
    uid: &str,
    href: &str,
    etag: &str,
    raw_vcard: &str,
    sync_ts: i64,
) -> Result<(), MailError> {
    let email = write.email.trim().to_lowercase();
    let extra_emails = serialize_json_strings(write.extra_emails.clone())?;
    let phones = serialize_json_strings(write.phones.clone())?;
    let write = write.clone();
    let uid = uid.to_string();
    let href = href.to_string();
    let etag = etag.to_string();
    let raw_vcard = raw_vcard.to_string();
    db.call(move |conn| {
        conn.execute(
            "UPDATE contacts
             SET email = ?1, extra_emails = ?2, name = ?3, phones = ?4, notes = ?5, org = ?6,
                 dav_account_id = ?7, dav_uid = ?8, dav_href = ?9, dav_etag = ?10,
                 raw_vcard = ?11, source = 'dav', updated_at = ?12
             WHERE id = ?13",
            rusqlite::params![
                email,
                extra_emails,
                write.name,
                phones,
                write.notes,
                write.org,
                account_id,
                uid,
                href,
                etag,
                raw_vcard,
                sync_ts,
                id,
            ],
        )?;
        Ok(())
    })
    .await
}

/// 拉取落库（新条目）：source='dav' 直插，返回新 ID。
#[allow(clippy::too_many_arguments)]
pub async fn dav_pull_insert(
    db: &DbConn,
    write: &ContactWrite,
    account_id: i32,
    uid: &str,
    href: &str,
    etag: &str,
    raw_vcard: &str,
    sync_ts: i64,
) -> Result<i32, MailError> {
    let email = write.email.trim().to_lowercase();
    let extra_emails = serialize_json_strings(write.extra_emails.clone())?;
    let phones = serialize_json_strings(write.phones.clone())?;
    let write = write.clone();
    let uid = uid.to_string();
    let href = href.to_string();
    let etag = etag.to_string();
    let raw_vcard = raw_vcard.to_string();
    let duplicate_message = format!("联系人已存在: {email}");
    let inserted = db
        .call(move |conn| {
            match conn.execute(
                "INSERT INTO contacts (
                    email, extra_emails, name, phones, notes, org, source,
                    dav_account_id, dav_uid, dav_href, dav_etag, raw_vcard,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'dav', ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                rusqlite::params![
                    email,
                    extra_emails,
                    write.name,
                    phones,
                    write.notes,
                    write.org,
                    account_id,
                    uid,
                    href,
                    etag,
                    raw_vcard,
                    sync_ts,
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

/// 推送落库：来源升级为 'dav' + 新 UID/href/etag + raw_vcard（同 [`dav_pull_apply`]
/// 的 `sync_ts` 游标语义，避免自推自拉）。
#[allow(clippy::too_many_arguments)]
pub async fn dav_push_mark(
    db: &DbConn,
    id: i32,
    account_id: i32,
    uid: &str,
    href: &str,
    etag: &str,
    raw_vcard: &str,
    sync_ts: i64,
) -> Result<(), MailError> {
    let uid = uid.to_string();
    let href = href.to_string();
    let etag = etag.to_string();
    let raw_vcard = raw_vcard.to_string();
    db.call(move |conn| {
        conn.execute(
            "UPDATE contacts
             SET source = 'dav', dav_account_id = ?1, dav_uid = ?2, dav_href = ?3,
                 dav_etag = ?4, raw_vcard = ?5, updated_at = ?6
             WHERE id = ?7",
            rusqlite::params![account_id, uid, href, etag, raw_vcard, sync_ts, id],
        )?;
        Ok(())
    })
    .await
}

/// `since` 之后本地删除的 href 列表（推送删除阶段用）。
pub async fn dav_tombstones(
    db: &DbConn,
    account_id: i32,
    since: i64,
) -> Result<Vec<String>, MailError> {
    let account = account_id;
    db.call(move |conn| {
        let mut stmt = conn
            .prepare("SELECT href FROM dav_deletes WHERE account_id = ?1 AND deleted_at > ?2")?;
        let rows = stmt
            .query_map(rusqlite::params![account, since], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// 服务器确认删除后清除墓碑。
pub async fn dav_tombstone_remove(
    db: &DbConn,
    account_id: i32,
    href: &str,
) -> Result<(), MailError> {
    let href = href.to_string();
    db.call(move |conn| {
        conn.execute(
            "DELETE FROM dav_deletes WHERE account_id = ?1 AND href = ?2",
            rusqlite::params![account_id, href],
        )?;
        Ok(())
    })
    .await
}

/// 建组或复用同名分组（同步拉取 CATEGORIES 入组用；与导入路径同一语义）。
pub async fn group_ensure(db: &DbConn, name: &str) -> Result<i32, MailError> {
    let name = name.trim().to_string();
    match group_insert(db, &name).await {
        Ok(id) => Ok(id),
        Err(MailError::DatabaseError(message)) if message.contains("分组已存在") => group_list(db)
            .await?
            .into_iter()
            .find(|group| group.name == name)
            .map(|group| group.id)
            .ok_or(MailError::DatabaseError(message)),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[tokio::test]
    async fn dav_binding_upsert_idempotent_and_touch() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        db.call(|conn| conn.execute("INSERT INTO accounts (name, email, provider, account_type, created_at, updated_at) VALUES ('a', 'a@x', 'custom', 'personal', 0, 0)", [])).await.unwrap();
        binding_upsert(&db, 1, "https://dav.example/", "user", None).await.unwrap();
        // 同 account_id 二次 upsert 覆盖 URL 不报错不双行
        binding_upsert(&db, 1, "https://dav2.example/", "user", Some("/ab/")).await.unwrap();
        let all = binding_get_all(&db).await.unwrap();
        assert_eq!(all.len(), 1, "同账号应覆盖不双行");
        assert_eq!(all[0].url, "https://dav2.example/");
        assert_eq!(all[0].addressbook_href.as_deref(), Some("/ab/"));

        let one = binding_get(&db, 1).await.unwrap().expect("按账号应查到");
        assert_eq!(one.username, "user");

        // touch 更新 token/时间
        binding_touch_synced(&db, 1, Some("token-1".into())).await.unwrap();
        let one = binding_get(&db, 1).await.unwrap().unwrap();
        assert_eq!(one.sync_token.as_deref(), Some("token-1"));
        assert!(one.last_sync_at.is_some());

        // 删除后消失
        binding_delete(&db, 1).await.unwrap();
        assert!(binding_get(&db, 1).await.unwrap().is_none());
        assert!(binding_get_all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn dav_binding_requires_existing_account() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        // accounts 表为空：FK 约束应报错
        assert!(binding_upsert(&db, 99, "https://x/", "u", None).await.is_err());
    }

    #[test]
    fn dav_password_roundtrip_in_memory_auth() {
        let auth = crate::domain::auth::AuthManager::in_memory();
        let key = "dav:7";
        auth.save_password(key, "app-secret").unwrap();
        assert_eq!(auth.get_password(key).unwrap(), "app-secret");
        auth.delete_password(key).unwrap();
        assert!(auth.get_password(key).is_err());
    }

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

    /// 在 `accounts` 表插入测试账号，返回自增 id（emails 表外键需要）。
    fn insert_test_account(conn: &rusqlite::Connection) -> rusqlite::Result<i32> {
        conn.execute(
            "INSERT INTO accounts (name, email, provider, account_type, created_at, updated_at)
             VALUES ('联系人测试账号', 'contacts-test@postium.test', 'custom', 'personal', 0, 0)",
            [],
        )?;
        Ok(conn.last_insert_rowid() as i32)
    }

    /// 在 `emails` 表插入一行最小夹具（仅必填列与聚合相关列，其余取默认值/NULL）。
    fn insert_email_row(
        conn: &rusqlite::Connection,
        account_id: i32,
        uid: i64,
        sender_name: Option<&str>,
        sender_email: &str,
        recipient_emails: &str,
        cc_emails: Option<&str>,
        sent_at: i64,
    ) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO emails (
                account_id, folder, uid, sender_name, sender_email,
                recipient_emails, cc_emails,
                sent_at, received_at, created_at, updated_at
            ) VALUES (?1, 'INBOX', ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7, ?7)",
            rusqlite::params![
                account_id,
                uid,
                sender_name,
                sender_email,
                recipient_emails,
                cc_emails,
                sent_at
            ],
        )?;
        Ok(())
    }

    #[tokio::test]
    async fn upsert_from_emails_increments_collected_and_preserves_dav() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();

        // 三封同一发件人（含大小写变体）：第一封无名，后两封有名
        upsert_from_emails(
            &db,
            &[
                IncomingContact { email: "Alice@Example.com".into(), name: None, contacted_at: 100 },
                IncomingContact { email: "alice@example.com".into(), name: Some("Alice".into()), contacted_at: 200 },
                IncomingContact { email: "ALICE@example.com".into(), name: Some("Alice Prime".into()), contacted_at: 300 },
            ],
        )
        .await
        .unwrap();

        let alice = get_by_email(&db, "alice@example.com").await.unwrap().expect("collected 条目应存在");
        assert_eq!(alice.source, "collected");
        assert_eq!(alice.frequency, 3, "三封邮件 → frequency=3");
        assert_eq!(alice.name.as_deref(), Some("Alice"), "name 只在为空时补齐，不随后续条目覆盖");
        assert_eq!(alice.last_contacted_at, Some(300), "last_contacted_at 取最大值");
        assert_eq!(list(&db, None, None, false, 0, 20).await.unwrap().total, 1, "大小写变体应去重为一条");

        // dav 条目：直接 SQL 插入 source='dav'、frequency=99
        db.call(|conn| {
            conn.execute(
                "INSERT INTO contacts (email, name, frequency, source, created_at, updated_at)
                 VALUES ('bob@example.com', 'Bob Dav', 99, 'dav', 0, 0)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        upsert_from_emails(
            &db,
            &[IncomingContact { email: "BOB@example.com".into(), name: Some("Bob Collected".into()), contacted_at: 500 }],
        )
        .await
        .unwrap();

        let bob = get_by_email(&db, "bob@example.com").await.unwrap().unwrap();
        assert_eq!(bob.source, "dav");
        assert_eq!(bob.frequency, 99, "dav 条目 frequency 不动");
        assert_eq!(bob.name.as_deref(), Some("Bob Dav"), "dav 条目 name 不被覆盖");
        assert_eq!(bob.last_contacted_at, Some(500), "dav 条目仅推进 last_contacted_at");
    }

    #[tokio::test]
    async fn backfill_from_emails_aggregates_and_is_idempotent() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();

        db.call(|conn| {
            let account_id = insert_test_account(conn)?;
            insert_email_row(conn, account_id, 1, Some("Ann"), "a@x", "", None, 100)?;
            insert_email_row(conn, account_id, 2, Some("Ann"), "A@X", "b@x,c@x", None, 200)?;
            Ok(())
        })
        .await
        .unwrap();

        let processed = backfill_from_emails(&db).await.unwrap();
        assert_eq!(processed, 3, "聚合出 3 个联系人地址");

        let response = list(&db, None, None, false, 0, 20).await.unwrap();
        assert_eq!(response.total, 3, "backfill 后应有 3 个联系人");

        let alice = get_by_email(&db, "a@x").await.unwrap().unwrap();
        assert_eq!(alice.source, "collected");
        assert_eq!(alice.frequency, 2, "a@x 作为两封邮件的 sender（大小写归一）");
        assert_eq!(alice.name.as_deref(), Some("Ann"), "sender_name 应带入");
        assert_eq!(alice.last_contacted_at, Some(200));

        let bob = get_by_email(&db, "b@x").await.unwrap().unwrap();
        assert_eq!(bob.frequency, 1);
        assert_eq!(bob.name, None, "收件人无名称，name 为空");
        assert_eq!(bob.last_contacted_at, Some(200));

        let carol = get_by_email(&db, "c@x").await.unwrap().unwrap();
        assert_eq!(carol.frequency, 1);

        // 幂等：再跑一次，collected 条目 frequency 不累加
        backfill_from_emails(&db).await.unwrap();
        let response = list(&db, None, None, false, 0, 20).await.unwrap();
        assert_eq!(response.total, 3, "重复回填不新增联系人");
        let alice = get_by_email(&db, "a@x").await.unwrap().unwrap();
        assert_eq!(alice.frequency, 2, "覆盖式回填，frequency 不因重跑而累加");
        let bob = get_by_email(&db, "b@x").await.unwrap().unwrap();
        assert_eq!(bob.frequency, 1);
    }

    /// M-1：recipient / cc 列可能存显示格式（`"Name <a@x>"`），backfill 应
    /// 提取尖括号内的裸地址，而非把整串当邮箱落库；同一地址的裸/显示两种
    /// 形态应合并为一条且频次求和。
    #[tokio::test]
    async fn backfill_extracts_display_format_addresses() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();

        db.call(|conn| {
            let account_id = insert_test_account(conn)?;
            // 收件人存显示格式、抄送混合显示格式与裸地址
            insert_email_row(
                conn,
                account_id,
                1,
                Some("Zoe"),
                "zoe@x",
                "Bob <b@x>",
                Some("Carol <c@x>, d@x"),
                100,
            )?;
            // b@x 再以裸地址出现一次（另一封邮件的收件人）
            insert_email_row(conn, account_id, 2, Some("Zoe"), "zoe@x", "b@x", None, 200)?;
            Ok(())
        })
        .await
        .unwrap();

        let processed = backfill_from_emails(&db).await.unwrap();
        assert_eq!(processed, 4, "归一后应聚合出 4 个裸地址（zoe/b/c/d），显示格式不单列");

        let response = list(&db, None, None, false, 0, 20).await.unwrap();
        assert_eq!(response.total, 4);
        let emails: Vec<&str> = response.contacts.iter().map(|c| c.email.as_str()).collect();
        assert!(
            emails.iter().all(|e| !e.contains('<') && !e.contains('>')),
            "不应把显示格式整串当邮箱落库: {emails:?}"
        );
        for want in ["zoe@x", "b@x", "c@x", "d@x"] {
            assert!(emails.contains(&want), "缺少裸地址 {want}: {emails:?}");
        }

        // 同一地址的显示格式 + 裸地址两次出现 → 合并为一条、频次求和
        let bob = get_by_email(&db, "b@x").await.unwrap().unwrap();
        assert_eq!(bob.frequency, 2, "裸/显示两种形态应合并计数");
        assert_eq!(bob.last_contacted_at, Some(200));
    }

    #[tokio::test]
    async fn group_crud_duplicate_rename_and_count() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let family_id = group_insert(&db, "家人").await.unwrap();
        let work_id = group_insert(&db, "工作").await.unwrap();

        // 重名（UNIQUE(name)）→ DatabaseError("分组已存在: ...")
        let err = group_insert(&db, "家人").await.unwrap_err();
        assert!(
            matches!(err, MailError::DatabaseError(ref msg) if msg.contains("分组已存在")),
            "重名建组应报分组已存在，实际: {err:?}"
        );

        // LEFT JOIN 计数：无成员的分组也出现且 count=0
        let groups = group_list(&db).await.unwrap();
        assert_eq!(groups.len(), 2, "应有 2 个分组");
        assert_eq!(groups[0].id, family_id, "按 id 升序");
        assert!(groups.iter().all(|group| group.count == 0), "初始计数均为 0");
        assert_eq!(groups[0].name, "家人");
        assert_eq!(groups[0].color, None);

        // 成员计入对应分组
        let cid = insert(
            &db,
            ContactWrite { email: "group-crud@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();
        group_set_members(&db, cid, &[family_id]).await.unwrap();
        let groups = group_list(&db).await.unwrap();
        assert_eq!(
            groups.iter().find(|group| group.id == family_id).unwrap().count,
            1,
            "成员应计入家人分组"
        );
        assert_eq!(
            groups.iter().find(|group| group.id == work_id).unwrap().count,
            0,
            "工作分组无成员"
        );

        // 改名
        group_rename(&db, work_id, "同事").await.unwrap();
        let groups = group_list(&db).await.unwrap();
        let work = groups.iter().find(|group| group.id == work_id).unwrap();
        assert_eq!(work.name, "同事", "改名应生效");

        // 改成已存在的名字 → 同样的重名错误
        let err = group_rename(&db, work_id, "家人").await.unwrap_err();
        assert!(
            matches!(err, MailError::DatabaseError(ref msg) if msg.contains("分组已存在")),
            "重名改组应报分组已存在，实际: {err:?}"
        );
    }

    #[tokio::test]
    async fn group_set_members_replaces_membership_and_delete_cascades() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let family_id = group_insert(&db, "家人").await.unwrap();
        let work_id = group_insert(&db, "工作").await.unwrap();
        let friend_id = group_insert(&db, "朋友").await.unwrap();

        let cid = insert(
            &db,
            ContactWrite { email: "member@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();

        // 初始无分组
        assert_eq!(get(&db, cid).await.unwrap().unwrap().groups.len(), 0);

        // 重复 group_id 去重；get() 的 dto.groups 按 g.id 升序填充
        group_set_members(&db, cid, &[work_id, family_id, work_id]).await.unwrap();
        let dto = get(&db, cid).await.unwrap().unwrap();
        let ids: Vec<i32> = dto.groups.iter().map(|group| group.id).collect();
        assert_eq!(ids, vec![family_id, work_id], "去重后按分组 id 升序");
        assert_eq!(dto.groups[0].name, "家人");

        // 全量替换：只剩朋友分组
        group_set_members(&db, cid, &[friend_id]).await.unwrap();
        let dto = get(&db, cid).await.unwrap().unwrap();
        let ids: Vec<i32> = dto.groups.iter().map(|group| group.id).collect();
        assert_eq!(ids, vec![friend_id], "set_members 为全量替换语义");

        // 空数组清空成员关系
        group_set_members(&db, cid, &[]).await.unwrap();
        assert_eq!(get(&db, cid).await.unwrap().unwrap().groups.len(), 0, "空数组应清空");

        // 删组后 FK 级联清理成员关系，dto.groups 中该组消失
        group_set_members(&db, cid, &[family_id, friend_id]).await.unwrap();
        group_delete(&db, family_id).await.unwrap();
        let dto = get(&db, cid).await.unwrap().unwrap();
        let ids: Vec<i32> = dto.groups.iter().map(|group| group.id).collect();
        assert_eq!(ids, vec![friend_id], "删组后成员关系应级联消失");

        // group_list 不再包含被删分组，且计数正确
        let groups = group_list(&db).await.unwrap();
        assert_eq!(groups.len(), 2, "删除后应剩 2 个分组");
        assert!(groups.iter().all(|group| group.id != family_id));
        assert_eq!(groups.iter().find(|group| group.id == friend_id).unwrap().count, 1);
        assert_eq!(groups.iter().find(|group| group.id == work_id).unwrap().count, 0);
    }

    /// 断言结果是 `MailError::DatabaseError`
    fn assert_database_error<T: std::fmt::Debug>(result: Result<T, MailError>, hint: &str) {
        match result {
            Err(MailError::DatabaseError(_)) => {}
            other => panic!("{hint}应报 DatabaseError，实际: {other:?}"),
        }
    }

    #[tokio::test]
    async fn merge_unions_fields_and_deletes_absorbed() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let family_id = group_insert(&db, "家人").await.unwrap();
        let work_id = group_insert(&db, "工作").await.unwrap();
        let friend_id = group_insert(&db, "朋友").await.unwrap();

        let primary_id = insert(
            &db,
            ContactWrite {
                email: "primary@postium.test".into(),
                extra_emails: Some(vec![
                    "P.Extra@Postium.Test".into(),
                    "shared@postium.test".into(),
                ]),
                name: Some("主联系人".into()),
                phones: Some(vec!["13800000000".into()]),
                notes: Some("主备注".into()),
                org: Some("Postium".into()),
            },
        )
        .await
        .unwrap();

        let absorbed1_id = insert(
            &db,
            ContactWrite {
                email: "absorbed1@postium.test".into(),
                // SHARED 大写变体验证忽略大小写去重；primary 主邮箱混入验证排除
                extra_emails: Some(vec![
                    "a1.extra@postium.test".into(),
                    "SHARED@postium.test".into(),
                    "primary@postium.test".into(),
                ]),
                name: Some("被合并一".into()),
                notes: Some("被合并一备注".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let absorbed2_id = insert(
            &db,
            ContactWrite {
                email: "absorbed2@postium.test".into(),
                name: Some("被合并二".into()),
                notes: Some("   ".into()), // 空白备注按空处理
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // 分组：primary=家人+工作；absorbed1=工作(重叠)+朋友；absorbed2=朋友
        group_set_members(&db, primary_id, &[family_id, work_id])
            .await
            .unwrap();
        group_set_members(&db, absorbed1_id, &[work_id, friend_id])
            .await
            .unwrap();
        group_set_members(&db, absorbed2_id, &[friend_id])
            .await
            .unwrap();

        // frequency / last_contacted_at 无写入 API，SQL 夹具（updated_at 置 0 便于断言刷新）
        db.call(move |conn| {
            conn.execute(
                "UPDATE contacts SET frequency = 5, last_contacted_at = 1000, updated_at = 0 WHERE id = ?1",
                [primary_id],
            )?;
            conn.execute(
                "UPDATE contacts SET frequency = 3, last_contacted_at = 2000 WHERE id = ?1",
                [absorbed1_id],
            )?;
            conn.execute(
                "UPDATE contacts SET frequency = 2, last_contacted_at = NULL WHERE id = ?1",
                [absorbed2_id],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        let merged = merge(&db, primary_id, &[absorbed1_id, absorbed2_id])
            .await
            .unwrap();

        assert_eq!(merged.id, primary_id);
        assert_eq!(merged.email, "primary@postium.test");

        // extra_emails 并集去重（忽略大小写，保留首次出现的原始写法），排除主邮箱
        let mut extras = merged.extra_emails.clone();
        extras.sort();
        assert_eq!(
            extras,
            vec![
                "P.Extra@Postium.Test".to_string(),
                "a1.extra@postium.test".to_string(),
                "shared@postium.test".to_string(),
            ],
            "extra_emails 应为并集去重结果"
        );
        assert!(
            !merged
                .extra_emails
                .iter()
                .any(|email| email.eq_ignore_ascii_case("primary@postium.test")),
            "主邮箱不得出现在 extra_emails"
        );

        assert_eq!(merged.frequency, 10, "frequency 求和 5+3+2");
        assert_eq!(
            merged.last_contacted_at,
            Some(2000),
            "last_contacted_at 取 max（NULL 不参与）"
        );
        assert_eq!(
            merged.notes.as_deref(),
            Some("主备注\n\n被合并一备注"),
            "notes 双非空以 \\n\\n 拼接，空白备注跳过"
        );
        assert_eq!(merged.name.as_deref(), Some("主联系人"), "name 取 primary");
        assert_eq!(merged.org.as_deref(), Some("Postium"), "org 取 primary");
        assert_eq!(
            merged.phones,
            vec!["13800000000".to_string()],
            "phones 不在合并规则内，保持 primary"
        );

        // 分组并集（家人+工作+朋友），attach_groups 按 id 升序
        let mut expected_group_ids = vec![family_id, work_id, friend_id];
        expected_group_ids.sort_unstable();
        let group_ids: Vec<i32> = merged.groups.iter().map(|group| group.id).collect();
        assert_eq!(group_ids, expected_group_ids, "分组应并集");

        // absorbed 消失
        assert!(get(&db, absorbed1_id).await.unwrap().is_none());
        assert!(get(&db, absorbed2_id).await.unwrap().is_none());
        assert_eq!(list(&db, None, None, false, 0, 20).await.unwrap().total, 1);

        // 各分组计数仍为 1（成员转移到 primary）
        let groups = group_list(&db).await.unwrap();
        for group_id in expected_group_ids {
            assert_eq!(
                groups.iter().find(|group| group.id == group_id).unwrap().count,
                1
            );
        }

        // updated_at 已刷新（夹具置 0 → 合并后 > 0）
        let updated_at: i64 = db
            .call(move |conn| {
                Ok(conn.query_row(
                    "SELECT updated_at FROM contacts WHERE id = ?1",
                    [primary_id],
                    |row| row.get(0),
                )?)
            })
            .await
            .unwrap();
        assert!(updated_at > 0, "updated_at 应刷新为 now");
    }

    #[tokio::test]
    async fn merge_defensive_rejects_invalid_lists() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let primary_id = insert(
            &db,
            ContactWrite { email: "p@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();
        let absorbed_id = insert(
            &db,
            ContactWrite { email: "a@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();

        // absorbed 含 primary 自身
        assert_database_error(
            merge(&db, primary_id, &[absorbed_id, primary_id]).await,
            "absorbed 含 primary",
        );
        // absorbed 含重复
        assert_database_error(
            merge(&db, primary_id, &[absorbed_id, absorbed_id]).await,
            "absorbed 含重复",
        );
        // absorbed 含不存在 id
        assert_database_error(
            merge(&db, primary_id, &[absorbed_id, 9999]).await,
            "absorbed 含不存在 id",
        );
        // primary 不存在
        assert_database_error(merge(&db, 8888, &[absorbed_id]).await, "primary 不存在");
        // 空列表
        assert_database_error(merge(&db, primary_id, &[]).await, "空合并列表");

        // 全部拒绝后数据原样：absorbed 未被删除、频次未变
        assert!(get(&db, absorbed_id).await.unwrap().is_some());
        assert_eq!(list(&db, None, None, false, 0, 20).await.unwrap().total, 2);
        let primary = get(&db, primary_id).await.unwrap().unwrap();
        assert_eq!(primary.frequency, 0, "防御失败不得改动 primary");
    }

    #[tokio::test]
    async fn merge_fills_empty_name_org_and_transfers_dav_fields() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();

        // dav_account_id 有外键约束，先建测试账号
        let dav_account_id = db
            .call(|conn| insert_test_account(conn))
            .await
            .unwrap();

        // primary 无 name/org、无 dav 字段；absorbed 为 dav 条目
        let primary_id = insert(
            &db,
            ContactWrite { email: "plain@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();
        let absorbed_id = db
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO contacts (
                        email, name, org, frequency, source,
                        dav_account_id, dav_uid, dav_href, dav_etag, created_at, updated_at
                     ) VALUES ('dav@postium.test', 'Dav 名字', 'Dav 组织', 4, 'dav', ?1, 'uid-1', 'href-1', 'etag-1', 0, 0)",
                    [dav_account_id],
                )?;
                Ok(conn.last_insert_rowid() as i32)
            })
            .await
            .unwrap();

        let merged = merge(&db, primary_id, &[absorbed_id]).await.unwrap();
        assert_eq!(
            merged.name.as_deref(),
            Some("Dav 名字"),
            "primary name 为空时取 absorbed 非空值"
        );
        assert_eq!(merged.org.as_deref(), Some("Dav 组织"));
        assert_eq!(merged.frequency, 4, "primary frequency=0 + absorbed 4");
        assert_eq!(merged.source, "local", "source 保持 primary");
        assert_eq!(
            merged.dav_account_id,
            Some(dav_account_id),
            "primary 为 NULL 的 dav 字段由 absorbed 补齐"
        );
        assert_eq!(merged.dav_uid.as_deref(), Some("uid-1"));
        assert_eq!(merged.dav_href.as_deref(), Some("href-1"));
        assert_eq!(merged.dav_etag.as_deref(), Some("etag-1"));

        // 反向：primary 已有 dav 字段时不被 absorbed 覆盖
        let primary2_id = db
            .call(move |conn| {
                conn.execute(
                    "INSERT INTO contacts (
                        email, source, dav_account_id, dav_uid, dav_href, dav_etag,
                        created_at, updated_at
                     ) VALUES ('keep@postium.test', 'dav', ?1, 'keep-uid', 'keep-href', 'keep-etag', 0, 0)",
                    [dav_account_id],
                )?;
                Ok(conn.last_insert_rowid() as i32)
            })
            .await
            .unwrap();
        let absorbed2_id = insert(
            &db,
            ContactWrite { email: "other@postium.test".into(), ..Default::default() },
        )
        .await
        .unwrap();
        db.call(move |conn| {
            conn.execute(
                "UPDATE contacts SET dav_account_id = ?1, dav_uid = 'other-uid',
                     dav_href = 'other-href', dav_etag = 'other-etag' WHERE id = ?2",
                rusqlite::params![dav_account_id, absorbed2_id],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        let merged2 = merge(&db, primary2_id, &[absorbed2_id]).await.unwrap();
        assert_eq!(
            merged2.dav_account_id,
            Some(dav_account_id),
            "primary 已有 dav 字段不覆盖"
        );
        assert_eq!(merged2.dav_uid.as_deref(), Some("keep-uid"));
        assert_eq!(merged2.dav_href.as_deref(), Some("keep-href"));
        assert_eq!(merged2.dav_etag.as_deref(), Some("keep-etag"));
    }

    #[tokio::test]
    async fn backfill_preserves_local_entry_and_collects_cc() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();

        db.call(|conn| {
            conn.execute(
                "INSERT INTO contacts (email, name, frequency, source, created_at, updated_at)
                 VALUES ('b@x', 'Bob Local', 7, 'local', 0, 0)",
                [],
            )?;
            let account_id = insert_test_account(conn)?;
            insert_email_row(conn, account_id, 1, Some("Ann"), "a@x", "b@x", Some("c@x"), 300)?;
            Ok(())
        })
        .await
        .unwrap();

        backfill_from_emails(&db).await.unwrap();

        let bob = get_by_email(&db, "b@x").await.unwrap().unwrap();
        assert_eq!(bob.source, "local");
        assert_eq!(bob.frequency, 7, "local 条目 frequency 不被覆盖");
        assert_eq!(bob.name.as_deref(), Some("Bob Local"), "local 条目 name 不动");
        assert_eq!(bob.last_contacted_at, Some(300), "local 条目推进 last_contacted_at");

        let ann = get_by_email(&db, "a@x").await.unwrap().unwrap();
        assert_eq!(ann.frequency, 1);

        let carol = get_by_email(&db, "c@x").await.unwrap().expect("cc 收件人应被收集");
        assert_eq!(carol.frequency, 1);
        assert_eq!(carol.source, "collected");
    }
}
