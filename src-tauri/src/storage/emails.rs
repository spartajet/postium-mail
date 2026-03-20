//! 邮件数据访问层
//!
//! 提供邮件的 CRUD 操作和邮件列表查询功能。
//!
//! # 核心功能
//!
//! - **邮件查询**: 分页列表、详情获取、搜索过滤
//! - **状态管理**: 已读/未读、星标标记
//! - **批量操作**: 批量删除、移动到文件夹
//! - **统计功能**: 按文件夹统计邮件数量和未读数
//! - **同步支持**: 从 IMAP 同步邮件数据的辅助方法
//!
//! # 数据模型
//!
//! ## Email 表结构
//!
//! | 字段 | 类型 | 说明 |
//! |------|------|------|
//! | id | INTEGER | 主键 |
//! | account_id | INTEGER | 所属账号 ID |
//! | folder | TEXT | 文件夹名称 |
//! | uid | INTEGER | IMAP UID |
//! | subject | TEXT | 邮件主题 |
//! | sender_name | TEXT | 发件人名称 |
//! | sender_email | TEXT | 发件人邮箱 |
//! | body_text | TEXT | 纯文本正文 |
//! | body_html | TEXT | HTML 正文 |
//! | is_read | BOOLEAN | 是否已读 |
//! | is_starred | BOOLEAN | 是否星标 |
//! | is_draft | BOOLEAN | 是否草稿 |
//! | sent_at | TIMESTAMP | 发送时间 |
//! | received_at | TIMESTAMP | 接收时间 |
//!
//! # 分页查询
//!
//! 邮件列表支持分页查询：
//!
//! ```text
//! page: 页码（从 0 开始）
//! page_size: 每页数量（默认 50）
//! total: 总邮件数
//! total_pages: 总页数
//! ```
//!
//! # 特殊文件夹
//!
//! ## 星标文件夹
//!
//! `folder = "starred"` 是虚拟文件夹，查询所有 `is_starred = true` 的邮件：
//!
//! ```rust,no_run
//! # use crate::storage::EmailRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! // 获取星标邮件
//! let response = EmailRepository::list(&db, account_id, "starred", 0, 50).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # 同步辅助方法
//!
//! 这些方法专用于 IMAP 同步：
//!
//! - `save_email_from_imap`: 保存从 IMAP 获取的邮件
//! - `email_exists_by_uid`: 检查 UID 是否已存在
//! - `update_email_status`: 更新从 IMAP 同步的邮件状态
//! - `delete_all_by_folder`: 清空文件夹所有邮件
//!
//! # 使用示例
//!
//! ## 获取邮件列表
//!
//! ```rust,no_run
//! # use crate::storage::EmailRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let response = EmailRepository::list(
//!     &db,
//!     account_id,
//!     "inbox",    // 文件夹
//!     0,          // 页码
//!     50,         // 每页数量
//! ).await?;
//!
//! println!("邮件总数: {}", response.total);
//! println!("总页数: {}", response.total_pages);
//! for email in response.items {
//!     println!("{}: {}", email.subject, email.sender_email);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## 更新已读状态
//!
//! ```rust,no_run
//! # use crate::storage::EmailRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! EmailRepository::update_read_status(&db, email_id, true).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## 切换星标
//!
//! ```rust,no_run
//! # use crate::storage::EmailRepository;
//! # async fn example() -> anyhow::Result<()> {
//! # let db = todo!();
//! let is_starred = EmailRepository::toggle_star(&db, email_id).await?;
//! println!("星标状态: {}", is_starred);
//! # Ok(())
//! # }
//! ```
//!
//! # 性能优化
//!
//! - 使用索引加速查询（account_id, folder, is_read, is_starred）
//! - 分页查询避免一次加载大量数据
//! - 附件数量通过批量查询获取，减少数据库往返

use sea_orm::ActiveModelTrait;
use sea_orm::{
    ColumnTrait, Condition, DbConn, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use crate::error::{Result, StorageError};
use crate::storage::models::{attachment, email};

/// 邮件仓库
///
/// 处理邮件的数据库查询操作
pub struct EmailRepository;

impl EmailRepository {
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
    ) -> Result<EmailListResponse> {
        let mut query = email::Entity::find().filter(email::Column::AccountId.eq(account_id));

        // 特殊处理星标文件夹
        if folder == "starred" {
            query = query.filter(email::Column::IsStarred.eq(true));
        } else {
            query = query.filter(email::Column::Folder.eq(folder));
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
        let attachment_counts = Self::get_attachment_counts(db, &email_ids).await?;

        let items: Vec<email::EmailListItem> = emails
            .into_iter()
            .map(|e| {
                let attachment_count = attachment_counts.get(&e.id).copied().unwrap_or(0);
                email::EmailListItem {
                    id: e.id,
                    account_id: e.account_id,
                    folder: e.folder,
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

    /// 获取邮件详情（含附件）
    pub async fn get_detail(db: &DbConn, id: i32) -> Result<email::EmailDetail> {
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
        let recipients: Vec<email::EmailAddress> =
            serde_json::from_str(&email_model.recipient_emails).unwrap_or_default();

        // 解析抄送
        let cc: Vec<email::EmailAddress> = email_model
            .cc_emails
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        // 解析密送
        let bcc: Vec<email::EmailAddress> = email_model
            .bcc_emails
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let attachment_infos: Vec<email::AttachmentInfo> = attachments
            .into_iter()
            .map(|a| email::AttachmentInfo {
                id: a.id,
                filename: a.filename,
                content_type: a.content_type,
                size: a.size as i64,
                path: a.path,
            })
            .collect();

        Ok(email::EmailDetail {
            id: email_model.id,
            account_id: email_model.account_id,
            folder: email_model.folder,
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
        let email = email::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

        let mut active_email: email::ActiveModel = email.into();
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
        let email = email::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

        let new_starred = !email.is_starred;

        let mut active_email: email::ActiveModel = email.into();
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

    /// 移动邮件到文件夹
    pub async fn move_to_folder(db: &DbConn, id: i32, folder: &str) -> Result<()> {
        let email = email::Entity::find_by_id(id)
            .one(db)
            .await
            .map_err(|e| StorageError::Database(format!("获取邮件失败: {}", e)))?
            .ok_or_else(|| StorageError::NotFound("邮件不存在".to_string()))?;

        let mut active_email: email::ActiveModel = email.into();
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
        let email = email::Entity::find()
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

        let mut active_email: email::ActiveModel = email.into();

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
}

/// 邮件列表响应
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmailListResponse {
    pub items: Vec<email::EmailListItem>,
    pub total: u64,
    pub total_pages: u64,
    pub page: u64,
    pub page_size: u64,
}
