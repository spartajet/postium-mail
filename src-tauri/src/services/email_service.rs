use sea_orm::*;
use anyhow::{anyhow, Result};
use crate::models::{email, attachment, EmailEntity, AttachmentEntity};

/// 邮件列表响应
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmailListResponse {
    pub emails: Vec<email::EmailListItem>,
    pub total: u64,
    pub page: usize,
    pub page_size: usize,
}

/// 获取邮件列表（分页）
pub async fn list(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    page: usize,
    limit: usize,
) -> Result<EmailListResponse> {
    tracing::info!("查询邮件列表: account_id={}, folder='{}', page={}, limit={}", account_id, folder, page, limit);

    let page_size = limit;
    let offset = page * page_size;

    // 特殊处理 "starred" 虚拟文件夹
    let is_starred_folder = folder == "starred";

    // 构建基础查询
    let mut query = EmailEntity::find()
        .filter(email::Column::AccountId.eq(account_id));

    // 根据文件夹类型应用不同的过滤条件
    if is_starred_folder {
        // 星标文件夹：查询所有星标邮件
        tracing::debug!("查询星标邮件");
        query = query.filter(email::Column::IsStarred.eq(true));
    } else {
        // 普通文件夹：按文件夹名称过滤（大小写不敏感）
        // 支持小写（新数据）和大写（旧数据）两种格式
        let folder_upper = folder.to_uppercase();
        tracing::debug!("查询文件夹: '{}' 或 '{}'", folder, folder_upper);
        query = query.filter(
            sea_orm::Condition::any()
                .add(email::Column::Folder.eq(folder))
                .add(email::Column::Folder.eq(folder_upper))
        );
    }

    // 获取总数
    let total = query.clone()
        .count(db)
        .await
        .map_err(|e| anyhow!("获取邮件总数失败: {}", e))?;

    tracing::info!("邮件总数: {}", total);

    // 获取邮件列表
    let emails = query
        .order_by_desc(email::Column::ReceivedAt)
        .paginate(db, page_size as u64)
        .fetch_page(offset as u64)
        .await
        .map_err(|e| anyhow!("获取邮件列表失败: {}", e))?;

    tracing::info!("返回邮件数量: {}", emails.len());

    // 转换为列表项
    let mut items = Vec::new();
    for email in emails {
        // 获取附件数量
        let attachment_count = AttachmentEntity::find()
            .filter(attachment::Column::EmailId.eq(email.id))
            .count(db)
            .await
            .map_err(|e| anyhow!("获取附件数量失败: {}", e))?;

        // 生成摘要
        let snippet = email.body_text
            .as_ref()
            .map(|t| {
                t.chars()
                    .take(200)
                    .collect::<String>()
                    .trim()
                    .to_string()
            });

        items.push(email::EmailListItem {
            id: email.id,
            account_id: email.account_id,
            folder: email.folder,
            subject: email.subject,
            sender_name: email.sender_name,
            sender_email: email.sender_email,
            snippet,
            has_attachment: attachment_count > 0,
            attachment_count: attachment_count as i32,
            is_read: email.is_read,
            is_starred: email.is_starred,
            is_draft: email.is_draft,
            sent_at: email.sent_at,
            received_at: email.received_at,
        });
    }

    Ok(EmailListResponse {
        emails: items,
        total,
        page,
        page_size,
    })
}

/// 根据 ID 获取邮件详情
pub async fn get_detail(db: &DbConn, id: i32) -> Result<email::EmailDetail> {
    let email = EmailEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取邮件失败: {}", e))?
        .ok_or_else(|| anyhow!("邮件不存在"))?;

    // 获取附件列表
    let attachments = AttachmentEntity::find()
        .filter(attachment::Column::EmailId.eq(id))
        .all(db)
        .await
        .map_err(|e| anyhow!("获取附件列表失败: {}", e))?;

    tracing::info!(
        "邮件 {} 的附件数量: {}",
        id,
        attachments.len()
    );

    let attachment_infos: Vec<email::AttachmentInfo> = attachments
        .into_iter()
        .map(|a| {
            tracing::info!(
                "附件详情: id={}, filename='{}', content_type={:?}, size={}, path={:?}",
                a.id,
                a.filename,
                a.content_type,
                a.size,
                a.path
            );
            email::AttachmentInfo {
                id: a.id,
                filename: a.filename,
                content_type: a.content_type,
                size: a.size as i64,
                path: a.path,
            }
        })
        .collect();

    // 解析收件人列表
    let recipients: Vec<email::EmailAddress> = serde_json::from_str(&email.recipient_emails)
        .unwrap_or_default();

    // 解析抄送列表
    let cc: Vec<email::EmailAddress> = email.cc_emails
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // 解析密送列表
    let bcc: Vec<email::EmailAddress> = email.bcc_emails
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(email::EmailDetail {
        id: email.id,
        account_id: email.account_id,
        folder: email.folder,
        uid: email.uid,
        message_id: email.message_id,
        subject: email.subject,
        sender_name: email.sender_name,
        sender_email: email.sender_email,
        recipients,
        cc,
        bcc,
        body_text: email.body_text,
        body_html: email.body_html,
        is_read: email.is_read,
        is_starred: email.is_starred,
        is_draft: email.is_draft,
        sent_at: email.sent_at,
        received_at: email.received_at,
        created_at: email.created_at,
        updated_at: email.updated_at,
        attachments: attachment_infos,
    })
}

/// 全文搜索邮件
pub async fn search(
    db: &DbConn,
    query: &str,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<email::EmailListItem>> {
    let limit = limit.unwrap_or(50);

    // 使用 FTS5 搜索
    let sql = if let Some(_acc_id) = account_id {
        "SELECT e.* FROM emails e \
             INNER JOIN emails_fts f ON e.id = f.rowid \
             WHERE emails_fts MATCH ? AND e.account_id = ? \
             ORDER BY e.received_at DESC \
             LIMIT ?".to_string()
    } else {
        "SELECT e.* FROM emails e \
             INNER JOIN emails_fts f ON e.id = f.rowid \
             WHERE emails_fts MATCH ? \
             ORDER BY e.received_at DESC \
             LIMIT ?".to_string()
    };

    let stmt = if let Some(acc_id) = account_id {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            [query.into(), acc_id.into(), limit.into()],
        )
    } else {
        Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            [query.into(), limit.into()],
        )
    };

    // 执行查询
    let results = EmailEntity::find()
        .from_raw_sql(stmt)
        .all(db)
        .await
        .map_err(|e| anyhow!("搜索邮件失败: {}", e))?;

    // 转换为列表项
    let mut items = Vec::new();
    for email in results {
        let snippet = email.body_text
            .as_ref()
            .map(|t| {
                t.chars()
                    .take(200)
                    .collect::<String>()
                    .trim()
                    .to_string()
            });

        items.push(email::EmailListItem {
            id: email.id,
            account_id: email.account_id,
            folder: email.folder,
            subject: email.subject,
            sender_name: email.sender_name,
            sender_email: email.sender_email,
            snippet,
            has_attachment: false,  // 搜索时不查询附件
            attachment_count: 0,
            is_read: email.is_read,
            is_starred: email.is_starred,
            is_draft: email.is_draft,
            sent_at: email.sent_at,
            received_at: email.received_at,
        });
    }

    Ok(items)
}

/// 更新已读状态
pub async fn update_read_status(db: &DbConn, id: i32, is_read: bool) -> Result<()> {
    let email = EmailEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取邮件失败: {}", e))?
        .ok_or_else(|| anyhow!("邮件不存在"))?;

    let mut email: email::ActiveModel = email.into();
    email.is_read = Set(is_read);

    email.update(db)
        .await
        .map_err(|e| anyhow!("更新已读状态失败: {}", e))?;

    Ok(())
}

/// 切换星标状态
pub async fn toggle_star(db: &DbConn, id: i32) -> Result<bool> {
    let email = EmailEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取邮件失败: {}", e))?
        .ok_or_else(|| anyhow!("邮件不存在"))?;

    let new_status = !email.is_starred;

    let mut email: email::ActiveModel = email.into();
    email.is_starred = Set(new_status);

    email.update(db)
        .await
        .map_err(|e| anyhow!("更新星标状态失败: {}", e))?;

    Ok(new_status)
}

/// 批量删除邮件
pub async fn batch_delete(db: &DbConn, ids: Vec<i32>) -> Result<usize> {
    let result = EmailEntity::delete_many()
        .filter(email::Column::Id.is_in(ids))
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除邮件失败: {}", e))?;

    Ok(result.rows_affected as usize)
}

/// 移动邮件到文件夹
pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<()> {
    let email = EmailEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取邮件失败: {}", e))?
        .ok_or_else(|| anyhow!("邮件不存在"))?;

    let mut email: email::ActiveModel = email.into();
    email.folder = Set(folder.to_string());

    email.update(db)
        .await
        .map_err(|e| anyhow!("移动邮件失败: {}", e))?;

    Ok(())
}

/// 检查邮件是否已存在（通过 UID）
pub async fn email_exists_by_uid(
    db: &DbConn,
    account_id: i32,
    uid: i32,
    folder: &str,
) -> bool {
    EmailEntity::find()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Uid.eq(Some(uid)))
        .filter(email::Column::Folder.eq(folder))
        .one(db)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
}

/// 保存从 IMAP 获取的邮件
pub async fn save_email_from_imap(
    db: &DbConn,
    account_id: i32,
    email_data: &crate::services::imap::EmailData,
    folder: &str,
) -> Result<i32> {
    use sea_orm::ActiveValue::*;

    // 解析收件人列表
    let recipients: Vec<email::EmailAddress> = email_data.to
        .split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(email::EmailAddress {
                    name: None,
                    email: s.to_string(),
                })
            }
        })
        .collect();

    let recipient_emails = serde_json::to_string(&recipients)
        .unwrap_or_default();

    // 解析抄送列表
    let cc_list: Vec<email::EmailAddress> = if !email_data.cc.is_empty() {
        email_data.cc
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    None
                } else {
                    Some(email::EmailAddress {
                        name: None,
                        email: s.to_string(),
                    })
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let cc_emails = if !cc_list.is_empty() {
        Some(serde_json::to_string(&cc_list).unwrap_or_default())
    } else {
        None
    };

    // 提取发送者名称（从邮箱地址的用户名部分）
    let sender_name = email_data.from.split('@').next()
        .unwrap_or(&email_data.from)
        .to_string();

    // 创建新邮件
    let new_email = email::ActiveModel {
        id: NotSet,
        account_id: Set(account_id),
        folder: Set(folder.to_string()),
        uid: Set(Some(email_data.uid as i32)),
        // 在 message_id 中包含 folder，确保不同文件夹的相同 UID 不会冲突
        message_id: Set(Some(format!("<{}.{}@postium.imap>", folder, email_data.uid))),
        subject: Set(Some(email_data.subject.clone())),
        sender_name: Set(Some(sender_name)),
        sender_email: Set(email_data.from.clone()),
        recipient_emails: Set(recipient_emails),
        cc_emails: Set(cc_emails),
        bcc_emails: Set(None),
        body_text: Set(Some(email_data.body_text.clone())),
        body_html: Set(Some(email_data.body_html.clone())),
        is_read: Set(email_data.flags.seen),
        is_starred: Set(email_data.flags.flagged),
        is_draft: Set(false),
        sent_at: Set(email_data.date.timestamp()),
        received_at: Set(email_data.date.timestamp()),
        created_at: Set(chrono::Utc::now().timestamp()),
        updated_at: Set(chrono::Utc::now().timestamp()),
    };

    let result = EmailEntity::insert(new_email)
        .exec(db)
        .await
        .map_err(|e| anyhow!("保存邮件失败: {}", e))?;

    let email_id = result.last_insert_id as i32;

    tracing::info!(
        "保存邮件成功: ID={}, UID={}, subject='{}', from='{}', to_count={}, cc_count={}",
        email_id,
        email_data.uid,
        email_data.subject,
        email_data.from,
        recipients.len(),
        cc_list.len()
    );

    // 保存附件信息
    if !email_data.attachments.is_empty() {
        save_attachments(db, email_id, &email_data.attachments).await?;
    }

    Ok(email_id)
}

/// 保存邮件附件
async fn save_attachments(
    db: &DbConn,
    email_id: i32,
    attachments: &[crate::services::imap::EmailAttachment],
) -> Result<()> {
    use sea_orm::ActiveValue::*;

    for attachment_data in attachments {
        let new_attachment = attachment::ActiveModel {
            id: NotSet,
            email_id: Set(email_id),
            filename: Set(attachment_data.filename.clone()),
            content_type: Set(Some(attachment_data.content_type.clone())),
            size: Set(attachment_data.size as i32),
            path: Set(None),
            created_at: Set(chrono::Utc::now().timestamp()),
        };

        AttachmentEntity::insert(new_attachment)
            .exec(db)
            .await
            .map_err(|e| anyhow!("保存附件失败: {}", e))?;

        tracing::info!(
            "保存附件成功: email_id={}, filename='{}', size={}",
            email_id,
            attachment_data.filename,
            attachment_data.size
        );
    }

    Ok(())
}

/// 统计文件夹的邮件数量
pub async fn count_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<i32> {
    let count = EmailEntity::find()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .count(db)
        .await
        .map_err(|e| anyhow!("统计文件夹邮件数量失败: {}", e))?;

    Ok(count as i32)
}

/// 统计文件夹的未读邮件数量
pub async fn count_unread_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<i32> {
    let count = EmailEntity::find()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .filter(email::Column::IsRead.eq(false))
        .count(db)
        .await
        .map_err(|e| anyhow!("统计未读邮件数量失败: {}", e))?;

    Ok(count as i32)
}

/// 更新邮件状态（从 IMAP 同步状态更新）
pub async fn update_email_status(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: i32,
    flags: &crate::services::imap::EmailFlags,
) -> Result<()> {
    // 查找邮件
    let email = EmailEntity::find()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .filter(email::Column::Uid.eq(Some(uid)))
        .one(db)
        .await
        .map_err(|e| anyhow!("查找邮件失败: {}", e))?
        .ok_or_else(|| anyhow!("邮件不存在: account_id={}, folder={}, uid={}", account_id, folder, uid))?;

    // 更新状态
    let mut email: email::ActiveModel = email.into();
    email.is_read = Set(flags.seen);
    email.is_starred = Set(flags.flagged);
    email.updated_at = Set(chrono::Utc::now().timestamp());

    email.update(db)
        .await
        .map_err(|e| anyhow!("更新邮件状态失败: {}", e))?;

    tracing::debug!(
        "更新邮件状态: account_id={}, folder={}, uid={}, seen={}, flagged={}",
        account_id, folder, uid, flags.seen, flags.flagged
    );

    Ok(())
}

/// 删除指定文件夹的所有邮件（用于UIDVALIDITY变化时）
pub async fn delete_all_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<u64> {
    let delete_result = EmailEntity::delete_many()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除文件夹邮件失败: {}", e))?;

    tracing::info!(
        "删除文件夹 {} 的所有邮件: account_id={}, 删除数量={}",
        folder,
        account_id,
        delete_result.rows_affected
    );

    Ok(delete_result.rows_affected)
}
