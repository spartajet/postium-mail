//! 邮件服务层
//!
//! 包含邮件相关的 DTO 类型定义和数据访问逻辑。
//!
//! # 核心功能
//!
//! - **邮件查询**: 分页列表、详情获取、搜索过滤
//! - **状态管理**: 已读/未读、星标标记
//! - **批量操作**: 批量删除、移动到文件夹
//! - **统计功能**: 按文件夹统计邮件数量和未读数
//! - **同步支持**: 从 IMAP 同步邮件数据的辅助方法

use sea_orm::prelude::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::error::{Result, StorageError};
use crate::protocols::imap::EmailHeader;
use crate::providers::StandardFolder;
use crate::storage::models::{attachment, email};
use crate::sync::change::EmailFlags;

// ============================================================================
// DTO 类型定义
// ============================================================================

/// 邮件地址
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub email: String,
    pub name: Option<String>,
}

/// 附件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub id: i32,
    pub filename: String,
    pub content_type: Option<String>,
    pub size: i64,
    pub path: Option<String>,
}

/// 邮件详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailDetail {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: Option<i32>,
    pub message_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub recipients: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub sent_at: i64,
    pub received_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub attachments: Vec<AttachmentInfo>,
}

/// 邮件列表项（用于列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailListItem {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub snippet: Option<String>,
    pub has_attachment: bool,
    pub attachment_count: i32,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub sent_at: i64,
    pub received_at: i64,
}

/// 邮件状态信息（轻量级，仅包含核心状态字段）
#[derive(Debug, Clone, Serialize, Deserialize, sea_orm::FromQueryResult)]
pub struct EmailStatus {
    pub id: i32,
    pub uid: Option<i32>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_draft: bool,
    pub is_answered: bool,
    pub is_deleted: bool,
}

/// 邮件列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailListResponse {
    /// 邮件列表
    #[serde(rename = "emails")]
    pub items: Vec<EmailListItem>,
    /// 总邮件数
    pub total: u64,
    /// 总页数
    pub total_pages: u64,
    /// 当前页码
    pub page: u64,
    /// 每页大小
    #[serde(rename = "page_size")]
    pub page_size: u64,
}

/// 发送邮件请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub subject: String,
    pub body_html: String,
    pub body_text: Option<String>,
    pub attachments: Vec<String>,
    pub in_reply_to: Option<String>,
}

/// 邮件搜索参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSearchParams {
    pub query: String,
    pub account_id: Option<i32>,
    pub folder: Option<String>,
    pub is_read: Option<bool>,
    pub is_starred: Option<bool>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub email_id: i32,
    pub subject: Option<String>,
    pub snippet: String,
    pub score: f64,
}

// ============================================================================
// Repository 实现
// ============================================================================

/// 获取邮件列表（分页）
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称（"starred" 表示星标文件夹）
/// * `page` - 页码（从 0 开始）
/// * `page_size` - 每页数量
pub async fn list(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    page: u64,
    page_size: u64,
    folder_mapping: &StandardFolder,
) -> Result<EmailListResponse> {
    let mut query = email::Entity::find().filter(email::Column::AccountId.eq(account_id));

    // 获取映射的 IMAP 文件夹列表
    let imap_folders = get_imap_folder_list(folder, folder_mapping);

    // 特殊处理星标文件夹
    if folder == "starred" {
        tracing::debug!("查询星标邮件");
        query = query.filter(email::Column::IsStarred.eq(true));
    } else if imap_folders.is_empty() {
        // 未知文件夹，直接使用原值查询（向后兼容）
        tracing::debug!("查询未知文件夹: {}", folder);
        query = query.filter(email::Column::Folder.eq(folder));
    } else {
        // 使用 OR 条件查询所有映射的文件夹
        tracing::debug!("查询标准文件夹: {}, IMAP 文件夹列表: {:?}", folder, imap_folders);
        let mut condition = Condition::any();
        for imap_folder in imap_folders {
            condition = condition.add(email::Column::Folder.eq(imap_folder));
        }
        query = query.filter(condition);
    }

    // 按接收时间倒序
    query = query.order_by_desc(email::Column::ReceivedAt);

    // 获取总数
    let total = query
        .clone()
        .count(db)
        .await
        .map_err(|e| StorageError::Database(format!("统计邮件失败: {}", e)))?;

    // 计算总页数
    let total_pages = total.div_ceil(page_size);

    // 分页查询
    let emails = query
        .paginate(db, page_size)
        .fetch_page(page)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件列表失败: {}", e)))?;

    // 转换为列表项
    let email_ids: Vec<i32> = emails.iter().map(|e| e.id).collect();

    // 获取附件数量
    let attachment_counts = get_attachment_counts(db, &email_ids).await?;

    let items: Vec<EmailListItem> = emails
        .into_iter()
        .map(|e| {
            let attachment_count = attachment_counts.get(&e.id).copied().unwrap_or(0);
            // 将 IMAP 文件夹名映射为前端标准文件夹名
            let standard_folder = folder_mapping.find_standard_type(&e.folder);
            EmailListItem {
                id: e.id,
                account_id: e.account_id,
                folder: standard_folder.to_string(),
                subject: e.subject,
                sender_name: e.sender_name,
                sender_email: e.sender_email,
                snippet: e.body_text.clone().map(|t| {
                    // 生成摘要（前 100 个字符）
                    t.chars().take(100).collect()
                }),
                has_attachment: attachment_count > 0,
                attachment_count,
                is_read: e.is_read,
                is_starred: e.is_starred,
                is_draft: e.is_draft,
                sent_at: e.sent_at,
                received_at: e.received_at,
            }
        })
        .collect();

    Ok(EmailListResponse {
        items,
        total,
        total_pages,
        page,
        page_size,
    })
}

/// 批量获取邮件状态信息（轻量级查询，仅返回核心状态字段）
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称（必需）
///
/// # 返回
///
/// 返回包含邮件状态信息的列表，仅包含：
/// - id: 邮件 ID
/// - uid: IMAP UID
/// - is_read: 是否已读
/// - is_starred: 是否星标
/// - is_draft: 是否草稿
/// - is_answered: 是否已回复
/// - is_deleted: 是否已删除
///
/// # 示例
///
/// ```rust,no_run
/// use crate::storage::service::email::list_status;
///
/// // 获取收件箱的邮件状态
/// let inbox_statuses = list_status(&db, account_id, "INBOX").await?;
///
/// // 获取已发送文件夹的邮件状态
/// let sent_statuses = list_status(&db, account_id, "Sent").await?;
/// ```
pub async fn list_status_by_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<Vec<EmailStatus>> {
    // 只选择需要的列以提高查询性能
    let results: Vec<EmailStatus> = email::Entity::find()
        .select_only()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .column(email::Column::Id)
        .column(email::Column::Uid)
        .column(email::Column::IsRead)
        .column(email::Column::IsStarred)
        .column(email::Column::IsDraft)
        .column(email::Column::IsAnswered)
        .column(email::Column::IsDeleted)
        .order_by_asc(email::Column::Uid)
        .into_model::<EmailStatus>()
        .all(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件状态失败: {}", e)))?;

    Ok(results)
}

/// 获取邮件详情（含附件）
pub async fn get_detail(
    db: &DbConn,
    id: i32,
    folder_mapping: &StandardFolder,
) -> Result<EmailDetail> {
    let email_model = email::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
        .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

    // 获取附件列表
    let attachments = attachment::Entity::find()
        .filter(attachment::Column::EmailId.eq(id))
        .all(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取附件失败: {}", e)))?;

    // 解析收件人
    let recipients: Vec<EmailAddress> =
        serde_json::from_str(&email_model.recipient_emails).unwrap_or_default();

    // 解析抄送
    let cc: Vec<EmailAddress> = email_model
        .cc_emails
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    // 解析密送
    let bcc: Vec<EmailAddress> = email_model
        .bcc_emails
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let attachment_infos: Vec<AttachmentInfo> = attachments
        .into_iter()
        .map(|a| AttachmentInfo {
            id: a.id,
            filename: a.filename,
            content_type: a.content_type,
            size: a.size as i64,
            path: a.path,
        })
        .collect();

    // 将 IMAP 文件夹名映射为前端标准文件夹名
    let standard_folder = folder_mapping.find_standard_type(&email_model.folder);

    Ok(EmailDetail {
        id: email_model.id,
        account_id: email_model.account_id,
        folder: standard_folder.to_string(),
        uid: email_model.uid,
        message_id: email_model.message_id,
        subject: email_model.subject,
        sender_name: email_model.sender_name,
        sender_email: email_model.sender_email,
        recipients,
        cc,
        bcc,
        body_text: email_model.body_text,
        body_html: email_model.body_html,
        is_read: email_model.is_read,
        is_starred: email_model.is_starred,
        is_draft: email_model.is_draft,
        sent_at: email_model.sent_at,
        received_at: email_model.received_at,
        created_at: email_model.created_at,
        updated_at: email_model.updated_at,
        attachments: attachment_infos,
    })
}

/// 更新已读状态
pub async fn update_read_status(db: &DbConn, id: i32, is_read: bool) -> Result<()> {
    let email_model = email::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
        .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

    let mut active_email: email::ActiveModel = email_model.into();
    active_email.is_read = Set(is_read);
    active_email.updated_at = Set(chrono::Utc::now().timestamp());

    active_email
        .update(db)
        .await
        .map_err(|e| StorageError::Database(format!("更新状态失败: {}", e)))?;

    Ok(())
}

/// 切换星标状态
pub async fn toggle_star(db: &DbConn, id: i32) -> Result<bool> {
    let email_model = email::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
        .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

    let new_starred = !email_model.is_starred;

    let mut active_email: email::ActiveModel = email_model.into();
    active_email.is_starred = Set(new_starred);
    active_email.updated_at = Set(chrono::Utc::now().timestamp());

    active_email
        .update(db)
        .await
        .map_err(|e| StorageError::Database(format!("更新状态失败: {}", e)))?;

    Ok(new_starred)
}

/// 批量删除邮件
pub async fn batch_delete(db: &DbConn, ids: Vec<i32>) -> Result<usize> {
    for id in &ids {
        // 先删除关联的附件记录
        attachment::Entity::delete_many()
            .filter(attachment::Column::EmailId.eq(*id))
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除附件失败: {}", e)))?;

        // 删除邮件
        email::Entity::delete_by_id(*id)
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除邮件失败: {}", e)))?;
    }

    Ok(ids.len())
}

/// 删除指定账号和文件夹的所有邮件
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
///
/// # 返回
///
/// 返回删除的邮件数量
pub async fn delete_account_folder_emails(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<usize> {
    // 1. 先获取该账号、该文件夹的所有邮件 ID
    let email_ids = email::Entity::find()
        .select_only()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .column(email::Column::Id)
        .into_tuple::<i32>()
        .all(db)
        .await
        .map_err(|e| StorageError::Database(format!("查询邮件 ID 失败: {}", e)))?;

    // 2. 删除这些邮件的附件
    for email_id in &email_ids {
        attachment::Entity::delete_many()
            .filter(attachment::Column::EmailId.eq(*email_id))
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除附件失败: {}", e)))?;
    }

    // 3. 删除所有邮件
    let delete_result = email::Entity::delete_many()
        .filter(email::Column::AccountId.eq(account_id))
        .filter(email::Column::Folder.eq(folder))
        .exec(db)
        .await
        .map_err(|e| StorageError::Database(format!("删除邮件失败: {}", e)))?;

    let deleted_count = delete_result.rows_affected as usize;

    tracing::info!(
        "已删除账号文件夹的所有邮件: account_id={}, folder={}, count={}",
        account_id,
        folder,
        deleted_count
    );

    Ok(deleted_count)
}

/// 移动邮件到文件夹
pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<()> {
    let email_model = email::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
        .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

    let mut active_email: email::ActiveModel = email_model.into();
    active_email.folder = Set(folder.to_string());
    active_email.updated_at = Set(chrono::Utc::now().timestamp());

    active_email
        .update(db)
        .await
        .map_err(|e| StorageError::Database(format!("移动邮件失败: {}", e)))?;

    Ok(())
}

/// 按文件夹统计邮件数量
pub async fn count_by_folder(db: &DbConn, account_id: i32, folder: &str) -> Result<i64> {
    let count = if folder == "starred" {
        email::Entity::find()
            .filter(
                Condition::all()
                    .add(email::Column::AccountId.eq(account_id))
                    .add(email::Column::IsStarred.eq(true)),
            )
            .count(db)
            .await?
    } else {
        email::Entity::find()
            .filter(
                Condition::all()
                    .add(email::Column::AccountId.eq(account_id))
                    .add(email::Column::Folder.eq(folder)),
            )
            .count(db)
            .await?
    };

    Ok(count as i64)
}

/// 按文件夹统计未读邮件数量
pub async fn count_unread_by_folder(db: &DbConn, account_id: i32, folder: &str) -> Result<i64> {
    let count = if folder == "starred" {
        email::Entity::find()
            .filter(
                Condition::all()
                    .add(email::Column::AccountId.eq(account_id))
                    .add(email::Column::IsStarred.eq(true))
                    .add(email::Column::IsRead.eq(false)),
            )
            .count(db)
            .await?
    } else {
        email::Entity::find()
            .filter(
                Condition::all()
                    .add(email::Column::AccountId.eq(account_id))
                    .add(email::Column::Folder.eq(folder))
                    .add(email::Column::IsRead.eq(false)),
            )
            .count(db)
            .await?
    };

    Ok(count as i64)
}

// ========== 同步辅助方法 ==========

/// 保存邮件从 IMAP
#[allow(clippy::too_many_arguments)]
pub async fn save_email_from_imap(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: i32,
    subject: Option<String>,
    sender_name: Option<String>,
    sender_email: String,
    recipient_emails: String,
    body_text: Option<String>,
    body_html: Option<String>,
    sent_at: i64,
    received_at: i64,
) -> Result<i32> {
    let now = chrono::Utc::now().timestamp();
    let active_email = email::ActiveModel {
        account_id: Set(account_id),
        folder: Set(folder.to_string()),
        uid: Set(Some(uid)),
        subject: Set(subject),
        sender_name: Set(sender_name),
        sender_email: Set(sender_email),
        recipient_emails: Set(recipient_emails),
        body_text: Set(body_text),
        body_html: Set(body_html),
        sent_at: Set(sent_at),
        received_at: Set(received_at),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let email = active_email
        .insert(db)
        .await
        .map_err(|e| StorageError::Database(format!("保存邮件失败: {}", e)))?;

    Ok(email.id)
}

/// 检查邮件是否已存在（按 UID）
pub async fn email_exists_by_uid(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: i32,
) -> Result<bool> {
    let exists = email::Entity::find()
        .filter(
            Condition::all()
                .add(email::Column::AccountId.eq(account_id))
                .add(email::Column::Folder.eq(folder))
                .add(email::Column::Uid.eq(uid)),
        )
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("查询邮件失败: {}", e)))?
        .is_some();

    Ok(exists)
}

/// 更新邮件状态（从 IMAP）
pub async fn update_email_status(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uid: i32,
    is_read: Option<bool>,
    is_starred: Option<bool>,
) -> Result<()> {
    let email_model = email::Entity::find()
        .filter(
            Condition::all()
                .add(email::Column::AccountId.eq(account_id))
                .add(email::Column::Folder.eq(folder))
                .add(email::Column::Uid.eq(uid)),
        )
        .one(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
        .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

    let mut active_email: email::ActiveModel = email_model.into();

    if let Some(read) = is_read {
        active_email.is_read = Set(read);
    }

    if let Some(starred) = is_starred {
        active_email.is_starred = Set(starred);
    }

    active_email.updated_at = Set(chrono::Utc::now().timestamp());

    active_email
        .update(db)
        .await
        .map_err(|e| StorageError::Database(format!("更新状态失败: {}", e)))?;

    Ok(())
}

/// 删除文件夹下所有邮件
pub async fn delete_all_by_folder(db: &DbConn, account_id: i32, folder: &str) -> Result<usize> {
    let emails = email::Entity::find()
        .filter(
            Condition::all()
                .add(email::Column::AccountId.eq(account_id))
                .add(email::Column::Folder.eq(folder)),
        )
        .all(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取邮件列表失败: {}", e)))?;

    for email in &emails {
        // 删除附件
        attachment::Entity::delete_many()
            .filter(attachment::Column::EmailId.eq(email.id))
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除附件失败: {}", e)))?;
    }

    let ids: Vec<i32> = emails.iter().map(|e| e.id).collect();

    for id in ids {
        email::Entity::delete_by_id(id)
            .exec(db)
            .await
            .map_err(|e| StorageError::Database(format!("删除邮件失败: {}", e)))?;
    }

    Ok(emails.len())
}

/// 批量更新邮件标志
///
/// 根据邮件 UID 批量更新邮件的标志状态（已读、星标、已回复、已删除）。
/// 这是一个高效的方法，避免逐个邮件更新时的多次数据库操作。
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `updates` - 更新列表，    ///   格式: `(uid, EmailFlags)` 或 `(uid, seen, flagged, answered, deleted, draft)`
///
/// # 返回
///
/// 返回更新的邮件数量
pub async fn batch_update_flags(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    updates: &[(u32, EmailFlags)],
) -> Result<usize> {
    if updates.is_empty() {
        return Ok(0);
    }

    let mut updated_count = 0;
    let now = chrono::Utc::now().timestamp();

    for (uid, flags) in updates {
        // 构建更新
        let result = email::Entity::update_many()
            .filter(email::Column::AccountId.eq(account_id))
            .filter(email::Column::Folder.eq(folder))
            .filter(email::Column::Uid.eq(*uid as i32))
            .col_expr(email::Column::IsRead, Expr::val(flags.seen))
            .col_expr(email::Column::IsStarred, Expr::val(flags.flagged))
            .col_expr(email::Column::IsAnswered, Expr::val(flags.answered))
            .col_expr(email::Column::IsDraft, Expr::val(flags.draft))
            .col_expr(email::Column::IsDeleted, Expr::val(flags.deleted))
            .col_expr(email::Column::UpdatedAt, Expr::val(now))
            .exec(db)
            .await
            .map_err(|e| {
                tracing::warn!("批量更新邮件标志失败: uid={}, error={}", uid, e);
                StorageError::Database(format!("批量更新邮件标志失败: uid={}, error={}", uid, e))
            })?;

        updated_count += 1;
    }

    tracing::debug!(
        "批量更新邮件标志完成: account_id={}, folder={}, updated_count={}",
        account_id,
        folder,
        updated_count
    );

    Ok(updated_count)
}

// ========== 辅助方法 ==========

/// 获取附件数量
async fn get_attachment_counts(
    db: &DbConn,
    email_ids: &[i32],
) -> Result<std::collections::HashMap<i32, i32>> {
    if email_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let attachments = attachment::Entity::find()
        .filter(attachment::Column::EmailId.is_in(email_ids.to_vec()))
        .all(db)
        .await
        .map_err(|e| StorageError::Database(format!("获取附件失败: {}", e)))?;

    let mut counts = std::collections::HashMap::new();
    for att in attachments {
        *counts.entry(att.email_id).or_insert(0) += 1;
    }

    Ok(counts)
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
) -> Result<usize> {
    if headers.is_empty() {
        return Ok(0);
    }

    let now = chrono::Utc::now().timestamp();

    // 将 EmailHeader 转换为 email::ActiveModel
    let active_emails: Vec<email::ActiveModel> = headers
        .iter()
        .map(|header| {
            email::ActiveModel {
                account_id: Set(account_id),
                folder: Set(folder.to_string()),
                uid: Set(Some(header.uid as i32)),
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
                is_read: Set(header.flags.seen),
                is_starred: Set(header.flags.flagged),
                is_draft: Set(false), // EmailHeader 的 EmailFlags 没有 draft 字段
                is_answered: Set(header.flags.answered),
                is_deleted: Set(header.flags.deleted),
                sent_at: Set(header.date.timestamp()),
                received_at: Set(header.date.timestamp()),
                created_at: Set(now),
                updated_at: Set(now),
                ..Default::default()
            }
        })
        .collect();

    // 批量插入
    let insert_result = email::Entity::insert_many(active_emails)
        .exec(db)
        .await
        .map_err(|e| StorageError::Database(format!("批量保存邮件头失败: {}", e)))?;

    Ok(headers.len())
}

/// 批量更新邮件状态（根据 ID）
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `status` - 邮件状态列表，必须包含有效的 id 字段
///
/// # 返回
///
/// 返回成功更新的邮件数量
///
/// # 示例
///
/// ```rust,no_run
/// use crate::storage::service::email::{batch_update_email_status, EmailStatus};
///
/// let statuses = vec![
///     EmailStatus {
///         id: 1,
///         uid: Some(101),
///         is_read: true,
///         is_starred: false,
///         is_draft: false,
///         is_answered: false,
///         is_deleted: false,
///     },
///     // ... 更多状态
/// ];
///
/// let updated = batch_update_email_status(&db, &statuses).await?;
/// ```
pub async fn batch_update_email_status(db: &DbConn, status: &[EmailStatus]) -> Result<usize> {
    if status.is_empty() {
        return Ok(0);
    }

    let mut updated_count = 0;
    let now = chrono::Utc::now().timestamp();

    for item_status in status {
        // 通过 ID 更新邮件状态
        let result = email::Entity::update_many()
            .filter(email::Column::Id.eq(item_status.id))
            .col_expr(email::Column::IsRead, Expr::val(item_status.is_read))
            .col_expr(email::Column::IsStarred, Expr::val(item_status.is_starred))
            .col_expr(email::Column::IsDraft, Expr::val(item_status.is_draft))
            .col_expr(
                email::Column::IsAnswered,
                Expr::val(item_status.is_answered),
            )
            .col_expr(email::Column::IsDeleted, Expr::val(item_status.is_deleted))
            .col_expr(email::Column::UpdatedAt, Expr::val(now))
            .exec(db)
            .await
            .map_err(|e| {
                tracing::warn!("批量更新邮件状态失败: id={}, error={}", item_status.id, e);
                StorageError::Database(format!(
                    "批量更新邮件状态失败: id={}, error={}",
                    item_status.id, e
                ))
            })?;

        // 如果 rows_affected > 0，说明更新成功
        if result.rows_affected > 0 {
            updated_count += 1;
        }
    }

    tracing::debug!(
        "批量更新邮件状态完成: updated_count={}, total={}",
        updated_count,
        status.len()
    );

    Ok(updated_count)
}

/// 批量删除邮件（根据 ID 列表）
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `ids` - 要删除的邮件 ID 列表
///
/// # 返回
///
/// 返回成功删除的邮件数量
///
/// # 注意
///
/// 此方法会同时删除邮件及其关联的附件
///
/// # 示例
///
/// ```rust,no_run
/// use crate::storage::service::email::batch_delete_by_ids;
///
/// let ids = vec![1, 2, 3, 4, 5];
/// let deleted = batch_delete_by_ids(&db, &ids).await?;
/// ```
pub async fn batch_delete_by_ids(db: &DbConn, ids: &[i32]) -> Result<usize> {
    if ids.is_empty() {
        return Ok(0);
    }

    // 1. 先删除关联的附件
    let deleted_attachments = attachment::Entity::delete_many()
        .filter(attachment::Column::EmailId.is_in(ids.iter().copied()))
        .exec(db)
        .await
        .map_err(|e| StorageError::Database(format!("批量删除附件失败: {}", e)))?;

    tracing::debug!(
        "批量删除邮件附件: attachment_count={}",
        deleted_attachments.rows_affected
    );

    // 2. 删除邮件
    let result = email::Entity::delete_many()
        .filter(email::Column::Id.is_in(ids.iter().copied()))
        .exec(db)
        .await
        .map_err(|e| StorageError::Database(format!("批量删除邮件失败: {}", e)))?;

    let deleted_count = result.rows_affected;

    tracing::debug!(
        "批量删除邮件完成: deleted_count={}, requested={}",
        deleted_count,
        ids.len()
    );

    Ok(deleted_count as usize)
}

/// 从地址字符串中提取名称
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回名称部分（如果有）
fn extract_name_from_address(address: &str) -> Option<String> {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<')
        && let Some(end) = address.find('>')
    {
        let name_part = &address[..start].trim();
        if !name_part.is_empty() {
            return Some(name_part.to_string());
        }
    }
    None
}

/// 从地址字符串中提取邮箱
///
/// # 参数
///
/// * `address` - 地址字符串（如 "John Doe <john@example.com>" 或 "john@example.com"）
///
/// # 返回
///
/// 返回邮箱部分
fn extract_email_from_address(address: &str) -> String {
    // 地址格式: "Name <email>" 或 "email"
    if let Some(start) = address.find('<')
        && let Some(end) = address.find('>')
    {
        return address[start + 1..end].to_string();
    }
    address.to_string()
}

/// 将地址列表序列化为 JSON 数组
///
/// # 参数
///
/// * `addresses` - 地址列表（逗号分隔）
///
/// # 返回
///
/// 返回 JSON 数组字符串，如 `["email1@example.com","email2@example.com"]`
fn serialize_addresses(addresses: &str) -> String {
    if addresses.is_empty() {
        return "[]".to_string();
    }

    // 分割地址并提取邮箱部分
    let emails: Vec<String> = addresses
        .split(',')
        .map(|addr| extract_email_from_address(addr.trim()))
        .map(|email| format!("\"{}\"", email))
        .collect();

    format!("[{}]", emails.join(","))
}

/// 文件夹统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderStat {
    pub id: i32,
    pub account_id: i32,
    pub name: String,
    pub imap_name: String,
    pub email_count: i64,
    pub unread_count: i64,
}

/// 获取账号的所有文件夹统计信息
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder_mapping` - 文件夹映射配置
///
/// # 返回
///
/// 返回所有文件夹的统计信息列表
pub async fn get_folder_stats(
    db: &DbConn,
    account_id: i32,
    folder_mapping: &StandardFolder,
) -> Result<Vec<FolderStat>> {
    let mut stats = Vec::new();

    // 统计标准文件夹（累加所有映射的 IMAP 文件夹）

    // 收件箱
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.inbox {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "inbox".to_string(),
        imap_name: folder_mapping.inbox.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 已发送
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.sent {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "sent".to_string(),
        imap_name: folder_mapping.sent.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 草稿箱
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.drafts {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "drafts".to_string(),
        imap_name: folder_mapping.drafts.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 垃圾邮件
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.spam {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "spam".to_string(),
        imap_name: folder_mapping.spam.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 废纸篓
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.trash {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "trash".to_string(),
        imap_name: folder_mapping.trash.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 归档
    let (mut email_count, mut unread_count) = (0i64, 0i64);
    for imap_name in &folder_mapping.archive {
        email_count += count_by_folder(db, account_id, imap_name).await?;
        unread_count += count_unread_by_folder(db, account_id, imap_name).await?;
    }
    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "archive".to_string(),
        imap_name: folder_mapping.archive.first().cloned().unwrap_or_default(),
        email_count,
        unread_count,
    });

    // 添加星标邮件统计（虚拟文件夹）
    let starred_count = count_by_folder(db, account_id, "starred").await?;
    let starred_unread = count_unread_by_folder(db, account_id, "starred").await?;

    stats.push(FolderStat {
        id: 0,
        account_id,
        name: "starred".to_string(),
        imap_name: "starred".to_string(),
        email_count: starred_count,
        unread_count: starred_unread,
    });

    Ok(stats)
}

/// 将前端标准文件夹名映射到 IMAP 文件夹名列表
///
/// # 参数
///
/// * `folder` - 前端传递的文件夹名（如 "inbox", "sent"）
/// * `folder_mapping` - 服务商的文件夹映射配置
///
/// # 返回
///
/// 返回对应的 IMAP 文件夹名列表（如 ["Sent", "Sent Items"]）
/// 如果是 "starred" 或未知文件夹，返回空列表
fn get_imap_folder_list<'a>(folder: &str, folder_mapping: &'a StandardFolder) -> Vec<&'a str> {
    match folder {
        "inbox" => folder_mapping.inbox.iter().map(|s| s.as_str()).collect(),
        "sent" => folder_mapping.sent.iter().map(|s| s.as_str()).collect(),
        "drafts" => folder_mapping.drafts.iter().map(|s| s.as_str()).collect(),
        "spam" => folder_mapping.spam.iter().map(|s| s.as_str()).collect(),
        "trash" => folder_mapping.trash.iter().map(|s| s.as_str()).collect(),
        "archive" => folder_mapping.archive.iter().map(|s| s.as_str()).collect(),
        _ => vec![], // "starred" 或其他自定义文件夹
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_address() {
        let addr = EmailAddress {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };
        assert_eq!(addr.email, "test@example.com");
        assert_eq!(addr.name, Some("Test User".to_string()));
    }

    #[test]
    fn test_send_email_request_default() {
        let req = SendEmailRequest {
            account_id: 1,
            to: vec![],
            cc: vec![],
            bcc: vec![],
            subject: "Test".to_string(),
            body_html: "<p>Test</p>".to_string(),
            body_text: None,
            attachments: vec![],
            in_reply_to: None,
        };
        assert_eq!(req.subject, "Test");
    }
}
