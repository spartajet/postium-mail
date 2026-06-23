use std::collections::HashMap;

use crate::domain::sync::FolderStat;
use crate::error::MailError;
use crate::infrastructure::protocols::types::{EmailHeader, WholeEmailDto};
use crate::infrastructure::protocols::utils::{
    extract_email_from_address, extract_name_from_address, serialize_addresses,
};
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::models::{attachments, emails};
use sea_orm::sea_query::Expr;
use sea_orm::*;

pub async fn list_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    let query = emails::Entity::find()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .filter(emails::Column::IsDeleted.eq(false));

    let total = query.clone().count(db).await?;

    let items = query
        .order_by_desc(emails::Column::SentAt)
        .offset(Some((page.saturating_sub(1).saturating_mul(limit)) as u64))
        .limit(Some(limit as u64))
        .all(db)
        .await?;

    Ok((items, total))
}

/// 按多个文件夹查询邮件（用于 category → 多文件夹映射）
pub async fn list_by_folders(
    db: &DbConn,
    account_id: i32,
    folders: &[String],
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    let query = emails::Entity::find()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.is_in(folders))
        .filter(emails::Column::IsDeleted.eq(false));

    let total = query.clone().count(db).await?;

    let items = query
        .order_by_desc(emails::Column::SentAt)
        .offset(Some((page.saturating_sub(1).saturating_mul(limit)) as u64))
        .limit(Some(limit as u64))
        .all(db)
        .await?;

    Ok((items, total))
}

/// 查询星标邮件（跨所有文件夹）
pub async fn list_starred(
    db: &DbConn,
    account_id: i32,
    page: usize,
    limit: usize,
) -> Result<(Vec<emails::Model>, u64), MailError> {
    let query = emails::Entity::find()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::IsStarred.eq(true))
        .filter(emails::Column::IsDeleted.eq(false));

    let total = query.clone().count(db).await?;

    let items = query
        .order_by_desc(emails::Column::SentAt)
        .offset(Some((page.saturating_sub(1).saturating_mul(limit)) as u64))
        .limit(Some(limit as u64))
        .all(db)
        .await?;

    Ok((items, total))
}

pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<emails::Model>, MailError> {
    Ok(emails::Entity::find_by_id(id).one(db).await?)
}

pub async fn get_by_uid(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: u32,
) -> Result<Option<emails::Model>, MailError> {
    Ok(emails::Entity::find()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .filter(emails::Column::Uid.eq(uid))
        .one(db)
        .await?)
}

pub async fn bulk_insert(db: &DbConn, models: Vec<emails::ActiveModel>) -> Result<(), MailError> {
    if models.is_empty() {
        return Ok(());
    }
    emails::Entity::insert_many(models).exec(db).await?;
    Ok(())
}

pub async fn update(
    db: &DbConn,
    id: i32,
    model: emails::ActiveModel,
) -> Result<emails::Model, MailError> {
    let mut model = model;
    model.id = Set(id);
    Ok(model.update(db).await?)
}

pub async fn mark_as_read(db: &DbConn, id: i32, is_read: bool) -> Result<(), MailError> {
    emails::Entity::update_many()
        .col_expr(emails::Column::IsRead, Expr::value(is_read))
        .filter(emails::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

pub async fn toggle_star(db: &DbConn, id: i32) -> Result<bool, MailError> {
    let email = emails::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(MailError::EmailNotFound(id))?;
    let new_state = !email.is_starred.unwrap_or(false);
    emails::Entity::update_many()
        .col_expr(emails::Column::IsStarred, Expr::value(new_state))
        .filter(emails::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(new_state)
}

pub async fn soft_delete(db: &DbConn, ids: Vec<i32>) -> Result<usize, MailError> {
    let result = emails::Entity::update_many()
        .col_expr(emails::Column::IsDeleted, Expr::value(true))
        .filter(emails::Column::Id.is_in(ids))
        .exec(db)
        .await?;
    Ok(result.rows_affected as usize)
}

pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<(), MailError> {
    emails::Entity::update_many()
        .col_expr(emails::Column::Folder, Expr::value(folder.to_string()))
        .filter(emails::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(())
}

/// 按 account_id 聚合文件夹统计（单条 SQL，替代 N+1 查询）
pub async fn folder_stats_by_account(
    db: &DbConn,
    account_id: i32,
) -> Result<Vec<FolderStat>, MailError> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        folder: String,
        total: i64,
        unread: i64,
    }

    let rows = Row::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
            SELECT folder,
                   COUNT(*) as total,
                   SUM(CASE WHEN is_read = 0 OR is_read IS NULL THEN 1 ELSE 0 END) as unread
            FROM emails
            WHERE account_id = ? AND (is_deleted = 0 OR is_deleted IS NULL)
            GROUP BY folder
            "#,
        [account_id.into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| FolderStat {
            folder: r.folder,
            total: r.total as usize,
            unread: r.unread as usize,
        })
        .collect())
}

/// 根据账号和文件夹获取邮件 ID
pub async fn get_ids_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Vec<i32>, MailError> {
    let email_ids = emails::Entity::find()
        .select_only()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .column(emails::Column::Id)
        .into_tuple::<i32>()
        .all(db)
        .await?;

    Ok(email_ids)
}

/// 根据账号和文件夹删除邮件
pub async fn delete_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<u64, MailError> {
    let delete_result = emails::Entity::delete_many()
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .exec(db)
        .await?;

    Ok(delete_result.rows_affected)
}

pub async fn delete_by_account<C>(db: &C, account_id: i32) -> Result<u64, MailError>
where
    C: ConnectionTrait,
{
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        r#"
        DELETE FROM attachments
        WHERE email_id IN (
            SELECT id FROM emails WHERE account_id = ?
        )
        "#,
        [account_id.into()],
    ))
    .await?;

    let delete_result = emails::Entity::delete_many()
        .filter(emails::Column::AccountId.eq(account_id))
        .exec(db)
        .await?;

    Ok(delete_result.rows_affected)
}

/// 批量保存邮件头
///
/// 从 IMAP 同步的邮件头批量保存到数据库。
/// 这个方法主要用于快速同步邮件列表，不包含邮件正文和附件。
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `headers` - 邮件头列表
///
/// # 返回
///
/// 返回成功保存的邮件数量
pub async fn save_batch_email_headers(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    headers: &[EmailHeader],
) -> Result<usize, MailError> {
    if headers.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp();

    // 将 EmailHeader 转换为 email::ActiveModel
    let active_emails: Vec<emails::ActiveModel> = headers
        .iter()
        .map(|header| emails::ActiveModel {
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            uid: Set(header.uid),
            subject: Set(Some(header.subject.clone())),
            sender_name: Set(extract_name_from_address(&header.from)),
            sender_email: Set(extract_email_from_address(&header.from)),
            recipient_emails: Set(serialize_addresses(&header.to)),
            cc_emails: Set(if header.cc.is_empty() {
                None
            } else {
                Some(serialize_addresses(&header.cc))
            }),
            bcc_emails: Set(None),
            body_text: Set(None),
            body_html: Set(None),
            message_id: Set(None),
            is_read: Set(Some(header.flags.seen)),
            is_starred: Set(Some(header.flags.flagged)),
            is_draft: Set(Some(header.flags.draft)),
            is_answered: Set(Some(header.flags.answered)),
            is_deleted: Set(Some(header.flags.deleted)),
            sent_at: Set(header.date.timestamp()),
            received_at: Set(header.date.timestamp()),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .collect();

    // 批量插入邮件
    emails::Entity::insert_many(active_emails)
        .exec(db)
        .await
        .map_err(|e| MailError::DatabaseError(format!("批量保存邮件头失败: {}", e)))?;

    // 查询刚插入的邮件，获取 uid -> id 映射（只查两列，避免 SELECT *）
    let uids: Vec<u32> = headers.iter().map(|h| h.uid).collect();
    let uid_id_pairs: Vec<(i32, u32)> = emails::Entity::find()
        .select_only()
        .column(emails::Column::Id)
        .column(emails::Column::Uid)
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .filter(emails::Column::Uid.is_in(uids))
        .into_tuple::<(i32, u32)>()
        .all(db)
        .await
        .map_err(|e| MailError::DatabaseError(format!("查询已插入邮件失败: {}", e)))?;

    let uid_to_id: HashMap<u32, i32> = uid_id_pairs
        .into_iter()
        .map(|(id, uid)| (uid, id))
        .collect();

    // 收集所有附件
    let mut active_attachments: Vec<attachments::ActiveModel> = Vec::new();
    for header in headers {
        if header.attachments.is_empty() {
            continue;
        }
        let email_id = match uid_to_id.get(&header.uid) {
            Some(id) => *id,
            None => {
                tracing::warn!(uid = header.uid, "未找到邮件ID，跳过附件保存");
                continue;
            }
        };
        for att in &header.attachments {
            active_attachments.push(attachments::ActiveModel {
                email_id: Set(email_id),
                filename: Set(att.filename.clone()),
                content_type: Set(Some(att.content_type.clone())),
                size: Set(att.size as i64),
                section_path: Set(att.section_path.clone()),
                disposition: Set(att.disposition.clone()),
                content_id: Set(att.content_id.clone()),
                path: Set(None),
                created_at: Set(now),
                ..Default::default()
            });
        }
    }

    // 批量插入附件
    if !active_attachments.is_empty() {
        attachments::Entity::insert_many(active_attachments)
            .exec(db)
            .await
            .map_err(|e| MailError::DatabaseError(format!("批量保存附件失败: {}", e)))?;
    }

    Ok(headers.len())
}

pub async fn save_batch_emails(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    emails: &[WholeEmailDto],
) -> Result<usize, MailError> {
    if emails.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp();

    // 将 WholeEmailDto 转换为 emails::ActiveModel
    // WholeEmailDto 的字段已是正确类型，可直接映射，无需地址解析
    let active_emails: Vec<emails::ActiveModel> = emails
        .iter()
        .map(|email| emails::ActiveModel {
            account_id: Set(account_id),
            folder: Set(folder.to_string()),
            uid: Set(email.uid),
            message_id: Set(email.message_id.clone()),
            subject: Set(email.subject.clone()),
            sender_name: Set(email.sender_name.clone()),
            sender_email: Set(email.sender_email.clone()),
            recipient_emails: Set(email.recipient_emails.clone()),
            cc_emails: Set(email.cc_emails.clone()),
            bcc_emails: Set(email.bcc_emails.clone()),
            preview: Set(email.preview.clone()),
            body_text: Set(email.body_text.clone()),
            body_html: Set(email.body_html.clone()),
            is_read: Set(Some(email.is_read)),
            is_starred: Set(Some(email.is_starred)),
            is_draft: Set(Some(email.is_draft)),
            is_answered: Set(Some(email.is_answered)),
            is_deleted: Set(Some(email.is_deleted)),
            sent_at: Set(email.sent_at),
            received_at: Set(email.received_at),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        })
        .collect();

    // 批量插入邮件
    emails::Entity::insert_many(active_emails)
        .exec(db)
        .await
        .map_err(|e| MailError::DatabaseError(format!("批量保存完整邮件失败: {}", e)))?;

    // 查询刚插入的邮件，获取 uid -> id 映射（列顺序 uid, id，直接 collect 无需 map 交换）
    let uids: Vec<u32> = emails.iter().map(|e| e.uid).collect();
    let uid_to_id: HashMap<u32, i32> = emails::Entity::find()
        .select_only()
        .column(emails::Column::Uid)
        .column(emails::Column::Id)
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Folder.eq(folder))
        .filter(emails::Column::Uid.is_in(uids))
        .into_tuple::<(u32, i32)>()
        .all(db)
        .await
        .map_err(|e| MailError::DatabaseError(format!("查询已插入邮件失败: {}", e)))?
        .into_iter()
        .collect();

    // 收集所有附件
    let mut active_attachments: Vec<attachments::ActiveModel> = Vec::new();
    for email in emails {
        if email.attachments.is_empty() {
            continue;
        }
        let email_id = match uid_to_id.get(&email.uid) {
            Some(id) => *id,
            None => {
                tracing::warn!(uid = email.uid, "未找到邮件ID，跳过附件保存");
                continue;
            }
        };
        for att in &email.attachments {
            active_attachments.push(attachments::ActiveModel {
                email_id: Set(email_id),
                filename: Set(att.filename.clone()),
                content_type: Set(Some(att.content_type.clone())),
                size: Set(att.size as i64),
                section_path: Set(att.section_path.clone()),
                disposition: Set(att.disposition.clone()),
                content_id: Set(att.content_id.clone()),
                path: Set(None),
                created_at: Set(now),
                ..Default::default()
            });
        }
    }

    // 批量插入附件
    if !active_attachments.is_empty() {
        attachments::Entity::insert_many(active_attachments)
            .exec(db)
            .await
            .map_err(|e| MailError::DatabaseError(format!("批量保存附件失败: {}", e)))?;
    }

    Ok(emails.len())
}

pub(crate) async fn update_body(
    db: &DatabaseConnection,
    account_id: i32,
    uid: u32,
    body_text: String,
    body_html: String,
) -> Result<(), MailError> {
    let preview: Option<String> = Some(body_text.chars().take(200).collect());
    let now = chrono::Utc::now().timestamp();

    emails::Entity::update_many()
        .col_expr(emails::Column::BodyText, Expr::value(Some(body_text)))
        .col_expr(emails::Column::BodyHtml, Expr::value(Some(body_html)))
        .col_expr(emails::Column::Preview, Expr::value(preview))
        .col_expr(emails::Column::UpdatedAt, Expr::value(now))
        .filter(emails::Column::AccountId.eq(account_id))
        .filter(emails::Column::Uid.eq(uid))
        .exec(db)
        .await
        .map_err(|e| MailError::DatabaseError(format!("更新邮件正文失败: {}", e)))?;

    Ok(())
}
