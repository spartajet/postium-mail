use crate::domain::folders::{FolderCategory, FolderRegistry, RemoteFolder};
use crate::domain::{auth::AuthManager, providers::pool::PROVIDER_POOL};
use crate::error::MailError;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::infrastructure::storage::repository::{
    account_repo, attachment_repo, email_repo, sync_repo,
};
use crate::infrastructure::storage::{DbConn, search};
use crate::service::attachment_service::{AttachmentDto, list_dtos_by_email};
use crate::service::mail_operation::{
    MailOperationService, MailRemoteOperator, RealMailRemoteOperator,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

// ═════════════════════════════════════════════════════════════════════════
// 邮件服务模块 (Email Service)
// ═════════════════════════════════════════════════════════════════════════
//
// 本模块负责邮件的核心业务逻辑，包括：
// 1. 邮件的查询和展示（列表、详情、分类）
// 2. 邮件的全文搜索
// 3. 邮件状态管理（已读/未读、星标、删除）
// 4. 邮件的发送（支持 SMTP 和 OAuth2）
// 5. 邮件文件夹操作
//
// 设计特点：
// - 使用分类（Category）抽象，将 IMAP 文件夹映射到前端显示的分类
// - 支持跨文件夹的星标邮件查询
// - 集成全文搜索（FTS）功能
// - 支持密码和 OAuth2 两种认证方式的邮件发送
// ═════════════════════════════════════════════════════════════════════════

// ═════════════════════════════════════════════════════════════════════════
// 邮件分类枚举
// ═════════════════════════════════════════════════════════════════════════

/// 邮件分类（前端侧边栏导航使用）
///
/// 每个变体对应一个 IMAP 文件夹组（通过 Provider 的 StandardFolder 映射）。
/// 这样设计的好处是前端不需要关心具体的 IMAP 文件夹名称，而是使用统一的分类。
///
/// # 变体说明
///
/// - `Inbox`: 收件箱，显示新收到的邮件
/// - `Starred`: 星标邮件，查询所有文件夹中标记为星标的邮件（跨文件夹）
/// - `Sent`: 已发送，显示已发送的邮件
/// - `Drafts`: 草稿箱，显示未发送的草稿
/// - `Spam`: 垃圾邮件，显示被标记为垃圾的邮件
/// - `Trash`: 已删除，显示已删除但未永久删除的邮件
/// - `Archive`: 归档，显示已归档的邮件
///
/// # 使用示例
///
/// ```rust,ignore
/// // 查询收件箱邮件
/// let response = email_service.list_by_category(
///     account_id,
///     EmailCategory::Inbox,
///     page,
///     limit
/// ).await?;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmailCategory {
    /// 收件箱
    Inbox,
    /// 星标邮件（跨文件夹查询）
    Starred,
    /// 已发送
    Sent,
    /// 草稿箱
    Drafts,
    /// 垃圾邮件
    Spam,
    /// 已删除
    Trash,
    /// 归档
    Archive,
}

// ═════════════════════════════════════════════════════════════════════════
// 数据传输对象 (DTO)
// ═════════════════════════════════════════════════════════════════════════

/// 邮件数据传输对象
///
/// 这是邮件列表项的标准格式，用于前端展示邮件列表。
/// 不包含邮件正文，只包含摘要信息。
///
/// # 字段说明
///
/// - `id`: 数据库主键
/// - `account_id`: 所属账号 ID
/// - `folder`: 所在文件夹名称（IMAP 文件夹）
/// - `uid`: IMAP 服务器上的 UID（用于同步）
/// - `subject`: 邮件主题
/// - `sender_name`: 发送人姓名
/// - `sender_email`: 发送人邮箱
/// - `preview`: 邮件内容预览（前 100 字符）
/// - `is_read`: 是否已读
/// - `is_starred`: 是否星标
/// - `sent_at`: 发送时间（Unix 时间戳，秒）
/// - `has_attachments`: 是否有附件（根据附件表真实计算）
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDto {
    /// 数据库主键
    pub id: i32,
    /// 所属账号 ID
    pub account_id: i32,
    /// IMAP 文件夹名称
    pub folder: String,
    /// IMAP 服务器 UID
    pub uid: u32,
    /// 邮件主题
    pub subject: Option<String>,
    /// 发送人姓名
    pub sender_name: Option<String>,
    /// 发送人邮箱
    pub sender_email: String,
    /// 账号邮箱
    pub account_email: Option<String>,
    /// 账号显示名
    pub account_display_name: Option<String>,
    /// 邮件内容预览
    pub preview: Option<String>,
    /// 是否已读
    pub is_read: bool,
    /// 是否星标
    pub is_starred: bool,
    /// 发送时间
    pub sent_at: i64,
    /// 是否有附件
    pub has_attachments: bool,
}

/// 邮件详情
///
/// 包含邮件的完整信息，包括正文内容。
///
/// # 字段说明
///
/// - `email`: 邮件基本信息（EmailDto 的扁平化版本）
/// - `recipient_emails`: 收件人邮箱列表（逗号分隔）
/// - `cc_emails`: 抄送邮箱列表（可选，逗号分隔）
/// - `body_text`: 纯文本正文
/// - `body_html`: HTML 格式正文
/// - `attachments`: 附件列表
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDetail {
    /// 邮件基本信息
    #[serde(flatten)]
    pub email: EmailDto,
    /// 收件人邮箱列表（逗号分隔）
    pub recipient_emails: String,
    /// 抄送邮箱列表（可选）
    pub cc_emails: Option<String>,
    /// 纯文本正文
    pub body_text: Option<String>,
    /// HTML 正文
    pub body_html: Option<String>,
    /// 附件列表
    pub attachments: Vec<AttachmentDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReloadEmailResult {
    Reloaded { email: EmailDetail },
    Removed { email_id: i32 },
}

/// 邮件列表响应
///
/// 分页查询的响应格式，包含邮件列表和分页信息。
///
/// # 字段说明
///
/// - `emails`: 当前页的邮件列表
/// - `total`: 总邮件数
/// - `page`: 当前页码（从 1 开始）
/// - `limit`: 每页数量
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailListResponse {
    /// 当前页的邮件列表
    pub emails: Vec<EmailDto>,
    /// 总邮件数
    pub total: u64,
    /// 当前页码
    pub page: usize,
    /// 每页数量
    pub limit: usize,
}

/// 发送邮件请求
///
/// 前端调用发送邮件 API 时传递的参数。
///
/// # 字段说明
///
/// - `account_id`: 发送账号 ID
/// - `to`: 收件人邮箱列表
/// - `cc`: 抄送邮箱列表
/// - `bcc`: 密送邮箱列表
/// - `subject`: 邮件主题
/// - `body_html`: HTML 格式的邮件正文
/// - `body_text`: 纯文本格式的邮件正文
///
/// # 使用示例
///
/// ```typescript,ignore
/// const req: SendEmailRequest = {
///     account_id: 1,
///     to: ['user@example.com'],
///     cc: ['cc@example.com'],
///     bcc: [],
///     subject: 'Hello',
///     body_html: '<p>Hello World</p>',
///     body_text: 'Hello World'
/// };
/// await sendEmail(req);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SendEmailRequest {
    /// 发送账号 ID
    pub account_id: i32,
    /// 收件人邮箱列表
    pub to: Vec<String>,
    /// 抄送邮箱列表
    pub cc: Vec<String>,
    /// 密送邮箱列表
    pub bcc: Vec<String>,
    /// 邮件主题
    pub subject: String,
    /// HTML 正文
    pub body_html: String,
    /// 纯文本正文
    pub body_text: String,
}

// ═════════════════════════════════════════════════════════════════════════
// 邮件服务实现
// ═════════════════════════════════════════════════════════════════════════

/// 邮件服务
///
/// 负责所有邮件相关的业务逻辑。
///
/// # 字段
///
/// - `auth`: 认证管理器，用于获取发送邮件的凭证
/// - `db`: 数据库连接，用于持久化操作
///
/// # 主要功能
///
/// - 邮件查询（列表、详情、分类）
/// - 邮件搜索（全文搜索）
/// - 邮件发送（支持 SMTP 和 OAuth2）
/// - 邮件状态更新（已读、星标、删除、移动）
pub struct EmailService {
    /// 认证管理器（线程安全）
    auth: Arc<AuthManager>,
    /// 数据库连接
    db: DbConn,
    /// 远端优先邮件操作服务
    mail_operation: MailOperationService,
}

impl EmailService {
    /// 创建新的邮件服务实例
    ///
    /// # 参数
    ///
    /// - `auth`: 认证管理器（使用 Arc 包装以便共享）
    /// - `db`: 数据库连接
    ///
    /// # 返回
    ///
    /// 返回初始化好的 EmailService 实例
    pub fn new(auth: Arc<AuthManager>, db: DbConn) -> Self {
        let remote = Arc::new(RealMailRemoteOperator::new(auth.clone()));
        Self::new_with_mail_remote(auth, db, remote)
    }

    pub fn new_with_mail_remote(
        auth: Arc<AuthManager>,
        db: DbConn,
        remote: Arc<dyn MailRemoteOperator>,
    ) -> Self {
        let mail_operation = MailOperationService::new(db.clone(), auth.clone(), remote);
        Self {
            auth,
            db,
            mail_operation,
        }
    }

    /// 获取指定文件夹的邮件列表（分页）
    ///
    /// 直接按 IMAP 文件夹名称查询邮件。
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    /// - `folder`: IMAP 文件夹名称（如 "INBOX", "Sent"）
    /// - `page`: 页码（从 1 开始）
    /// - `limit`: 每页数量
    ///
    /// # 返回
    ///
    /// 返回包含邮件列表和分页信息的 EmailListResponse
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let response = email_service.list(1, "INBOX", 1, 20).await?;
    /// println!("总邮件数: {}", response.total);
    /// for email in response.emails {
    ///     println!("{} - {}", email.subject, email.sender_email);
    /// }
    /// ```
    pub async fn list(
        &self,
        account_id: i32,
        folder: &str,
        page: usize,
        limit: usize,
    ) -> Result<EmailListResponse, MailError> {
        // 从数据库查询邮件（分页）
        let (emails, total) =
            email_repo::list_by_folder(&self.db, account_id, folder, page, limit).await?;
        Ok(EmailListResponse {
            emails: convert_models_with_attachments(&self.db, emails).await?,
            total,
            page,
            limit,
        })
    }

    /// 按分类获取邮件列表（分页）
    ///
    /// 使用邮件分类（Category）查询，系统会自动将分类映射到对应的 IMAP 文件夹。
    ///
    /// # 特殊处理
    ///
    /// - `Starred`: 跨所有文件夹查询星标邮件（is_starred = true）
    /// - 其他分类: 映射到对应的标准 IMAP 文件夹
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    /// - `category`: 邮件分类（Inbox, Starred, Sent 等）
    /// - `page`: 页码（从 1 开始）
    /// - `limit`: 每页数量
    ///
    /// # 返回
    ///
    /// 返回包含邮件列表和分页信息的 EmailListResponse
    ///
    /// # 工作流程
    ///
    /// 1. 如果是 `Starred`，直接查询所有星标邮件（跨文件夹）
    /// 2. 如果是其他分类：
    ///    - 获取账号信息
    ///    - 获取 Provider 的文件夹映射
    ///    - 将分类转换为 IMAP 文件夹列表
    ///    - 查询这些文件夹中的邮件
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 查询收件箱
    /// let inbox = email_service.list_by_category(
    ///     1,
    ///     EmailCategory::Inbox,
    ///     1,
    ///     20
    /// ).await?;
    ///
    /// // 查询星标邮件
    /// let starred = email_service.list_by_category(
    ///     1,
    ///     EmailCategory::Starred,
    ///     1,
    ///     20
    /// ).await?;
    /// ```
    pub async fn list_by_category(
        &self,
        account_id: i32,
        category: EmailCategory,
        page: usize,
        limit: usize,
        unread_only: bool,
    ) -> Result<EmailListResponse, MailError> {
        // ─── 特殊处理：星标邮件（跨文件夹查询） ───
        if category == EmailCategory::Starred {
            let (emails, total) =
                email_repo::list_starred(&self.db, account_id, page, limit, unread_only).await?;
            return Ok(EmailListResponse {
                emails: convert_models_with_attachments(&self.db, emails).await?,
                total,
                page,
                limit,
            });
        }

        // ─── 获取 Provider 的文件夹映射 ───
        let account = account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))?;

        let folders = resolve_category_folders_for_account(&self.db, &account, &category).await?;

        // 如果没有映射的文件夹，返回空结果
        if folders.is_empty() {
            return Ok(EmailListResponse {
                emails: vec![],
                total: 0,
                page,
                limit,
            });
        }

        // ─── 查询这些文件夹中的邮件 ───
        let (emails, total) =
            email_repo::list_by_folders(&self.db, account_id, &folders, page, limit, unread_only)
                .await?;
        Ok(EmailListResponse {
            emails: convert_models_with_attachments(&self.db, emails).await?,
            total,
            page,
            limit,
        })
    }

    pub async fn list_by_category_for_all_accounts(
        &self,
        category: EmailCategory,
        page: usize,
        limit: usize,
        unread_only: bool,
    ) -> Result<EmailListResponse, MailError> {
        if category == EmailCategory::Starred {
            let (emails, total) =
                email_repo::list_starred_all_accounts(&self.db, page, limit, unread_only).await?;
            return Ok(EmailListResponse {
                emails: convert_models_with_account_display(&self.db, emails).await?,
                total,
                page,
                limit,
            });
        }

        let accounts = account_repo::list(&self.db).await?;
        let mut filters = Vec::new();
        let mut failures = Vec::new();

        for account in accounts {
            match resolve_category_folders_for_account(&self.db, &account, &category).await {
                Ok(folders) if !folders.is_empty() => {
                    filters.push(email_repo::AccountFolderFilter {
                        account_id: account.id,
                        folders,
                    });
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(
                        account_id = account.id,
                        error = %err,
                        "所有账号分类查询跳过异常账号"
                    );
                    failures.push(err);
                }
            }
        }

        if filters.is_empty() {
            if let Some(err) = failures.into_iter().next() {
                return Err(err);
            }
            return Ok(EmailListResponse {
                emails: Vec::new(),
                total: 0,
                page,
                limit,
            });
        }

        let (emails, total) =
            email_repo::list_by_account_folder_filters(&self.db, filters, page, limit, unread_only)
                .await?;
        Ok(EmailListResponse {
            emails: convert_models_with_account_display(&self.db, emails).await?,
            total,
            page,
            limit,
        })
    }

    /// 获取邮件详情
    ///
    /// 查询指定 ID 的邮件完整信息，包括正文内容。
    ///
    /// # 参数
    ///
    /// - `id`: 邮件 ID
    ///
    /// # 返回
    ///
    /// 返回邮件详情（EmailDetail），包含正文内容。
    /// 如果邮件不存在，返回 `EmailNotFound` 错误。
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let detail = email_service.get(123).await?;
    /// println!("主题: {}", detail.email.subject);
    /// println!("正文: {}", detail.body_html);
    /// ```
    pub async fn get(&self, id: i32) -> Result<EmailDetail, MailError> {
        // 从数据库查询邮件
        let email = email_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::EmailNotFound(id))?;
        let attachments = list_dtos_by_email(&self.db, id).await?;

        Ok(email_model_to_detail(email, attachments))
    }

    pub async fn reload_email(&self, email_id: i32) -> Result<ReloadEmailResult, MailError> {
        let email = email_repo::get_by_id(&self.db, email_id)
            .await?
            .ok_or(MailError::EmailNotFound(email_id))?;
        let account = account_repo::get_by_id(&self.db, email.account_id)
            .await?
            .ok_or(MailError::AccountNotFound(email.account_id))?;

        match self
            .mail_operation
            .remote()
            .reload_email(&account, &email.folder, email.uid)
            .await?
        {
            Some(remote_email) => {
                let updated = email_repo::replace_email_with_attachments(
                    &self.db,
                    email_id,
                    account.id,
                    &email.folder,
                    remote_email,
                )
                .await?;
                let attachments = list_dtos_by_email(&self.db, email_id).await?;
                Ok(ReloadEmailResult::Reloaded {
                    email: email_model_to_detail_with_account(updated, attachments, &account),
                })
            }
            None => {
                email_repo::delete_one_with_attachments(&self.db, email_id).await?;
                Ok(ReloadEmailResult::Removed { email_id })
            }
        }
    }

    /// 全文搜索邮件
    ///
    /// 使用 SQLite FTS (Full-Text Search) 进行邮件搜索。
    /// 支持搜索邮件主题、发送人、正文等字段。
    ///
    /// # 参数
    ///
    /// - `query`: 搜索关键词（支持空格分隔多个关键词）
    /// - `account_id`: 可选，限制搜索范围到指定账号
    /// - `limit`: 可选，返回结果的最大数量（默认 50）
    ///
    /// # 返回
    ///
    /// 返回搜索结果列表，包含匹配的邮件信息。
    ///
    /// # 搜索范围
    ///
    /// - 邮件主题 (subject)
    /// - 发送人 (sender_name, sender_email)
    /// - 收件人 (recipient_emails)
    /// - 邮件正文 (body_text)
    /// - 预览文本 (preview)
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 搜索所有账号中的邮件
    /// let results = email_service.search("项目 会议").await?;
    ///
    /// // 搜索特定账号
    /// let results = email_service.search(
    ///     "项目 会议",
    ///     Some(1),  // account_id
    ///     Some(20)  // limit
    /// ).await?;
    /// ```
    pub async fn search(
        &self,
        query: &str,
        account_id: Option<i32>,
        limit: Option<u64>,
    ) -> Result<Vec<search::SearchResult>, MailError> {
        search::search_fts(&self.db, query, account_id, limit.unwrap_or(50)).await
    }

    /// 标记邮件为已读或未读
    ///
    /// 更新邮件的阅读状态，该状态也会同步到 IMAP 服务器。
    ///
    /// # 参数
    ///
    /// - `id`: 邮件 ID
    /// - `is_read`: true 表示标记为已读，false 表示标记为未读
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 注意事项
    ///
    /// - 此操作会更新数据库
    /// - 同步服务会将状态同步到 IMAP 服务器
    /// - 前端需要根据状态更新 UI 显示
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 标记为已读
    /// email_service.mark_as_read(123, true).await?;
    ///
    /// // 标记为未读
    /// email_service.mark_as_read(123, false).await?;
    /// ```
    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), MailError> {
        self.mail_operation.mark_as_read(id, is_read).await
    }

    /// 切换邮件星标状态
    ///
    /// 将邮件在星标和非星标状态之间切换。
    ///
    /// # 参数
    ///
    /// - `id`: 邮件 ID
    ///
    /// # 返回
    ///
    /// 返回切换后的星标状态（true 表示已星标）。
    ///
    /// # 使用场景
    ///
    /// - 用户点击星标按钮时调用
    /// - 快速标记重要邮件
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let is_starred = email_service.toggle_star(123).await?;
    /// println!("星标状态: {}", is_starred);
    /// ```
    pub async fn toggle_star(&self, id: i32) -> Result<bool, MailError> {
        self.mail_operation.toggle_star(id).await
    }

    /// 删除邮件（远端优先移动到 Trash）
    ///
    /// 先在远端将邮件移动到服务商映射的 Trash 文件夹，远端成功后再更新本地文件夹。
    /// 第一阶段不执行永久删除或 EXPUNGE。
    ///
    /// # 参数
    ///
    /// - `ids`: 要删除的邮件 ID 列表。第一阶段仅支持单封邮件。
    ///
    /// # 返回
    ///
    /// 返回实际移动到 Trash 的邮件数量。
    ///
    /// # 工作流程
    ///
    /// 1. 查询邮件和账号信息
    /// 2. 解析服务商映射的 Trash 文件夹
    /// 3. 调用远端移动操作
    /// 4. 远端成功后更新本地邮件文件夹
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let count = email_service.delete(vec![123]).await?;
    /// println!("已删除 {} 封邮件", count);
    /// ```
    pub async fn delete(&self, ids: Vec<i32>) -> Result<usize, MailError> {
        self.mail_operation.delete(ids).await
    }

    /// 将邮件移动到指定文件夹
    ///
    /// 在 IMAP 文件夹之间移动邮件。
    ///
    /// # 参数
    ///
    /// - `id`: 邮件 ID
    /// - `folder`: 目标文件夹名称（如 "INBOX", "Sent", "Trash"）
    ///
    /// # 返回
    ///
    /// 成功时返回 Ok(())，失败时返回错误。
    ///
    /// # 使用场景
    ///
    /// - 用户拖拽邮件到文件夹
    /// - 将邮件移到归档
    /// - 将垃圾邮件移回收件箱
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 将邮件移到归档
    /// email_service.move_to_folder(123, "Archive").await?;
    ///
    /// // 将垃圾邮件移回收件箱
    /// email_service.move_to_folder(123, "INBOX").await?;
    /// ```
    pub async fn move_to_folder(&self, id: i32, folder: &str) -> Result<(), MailError> {
        self.mail_operation.move_to_folder(id, folder).await
    }

    pub async fn archive(&self, id: i32) -> Result<(), MailError> {
        self.mail_operation.archive(id).await
    }

    /// 发送邮件
    ///
    /// 通过 SMTP 协议发送邮件，支持密码认证和 OAuth2 认证。
    ///
    /// # 参数
    ///
    /// - `req`: 发送邮件请求，包含收件人、主题、正文等信息
    ///
    /// # 返回
    ///
    /// 成功时返回服务器返回的消息 ID，失败时返回错误。
    ///
    /// # 工作流程
    ///
    /// 1. 查询发送账号信息
    /// 2. 获取 Provider 的 SMTP 配置
    /// 3. 根据认证类型获取凭证（密码或 OAuth2 token）
    /// 4. 调用 SMTP 协议发送邮件
    /// 5. 记录日志
    ///
    /// # 支持的认证方式
    ///
    /// - **密码认证**: 使用用户密码登录 SMTP 服务器
    /// - **OAuth2 认证**: 使用 OAuth2 access_token（通过 XOAUTH2 机制）
    ///
    /// # 错误处理
    ///
    /// - 账号不存在: `AccountNotFound`
    /// - Provider 不支持: `ProviderNotSupported`
    /// - 认证失败: `AuthenticationFailed`
    /// - 网络错误: `NetworkError`
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let req = SendEmailRequest {
    ///     account_id: 1,
    ///     to: vec!["user@example.com".to_string()],
    ///     cc: vec![],
    ///     bcc: vec![],
    ///     subject: "测试邮件".to_string(),
    ///     body_html: "<p>你好，这是一封测试邮件。</p>".to_string(),
    ///     body_text: "你好，这是一封测试邮件。".to_string(),
    /// };
    ///
    /// let message_id = email_service.send(req).await?;
    /// println!("邮件已发送，ID: {}", message_id);
    /// ```
    pub async fn send(&self, req: SendEmailRequest) -> Result<String, MailError> {
        tracing::info!(account_id = req.account_id, to = req.to.len(), "发送邮件");

        // ─── 查询发送账号 ───
        let account = account_repo::get_by_id(&self.db, req.account_id)
            .await?
            .ok_or(MailError::AccountNotFound(req.account_id))?;

        // ─── 获取 Provider 配置 ───
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".to_string(),
            ))?
            .clone();

        let provider = provider_pool
            .get(&account.provider)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;

        // ─── 获取 SMTP 配置 ───
        let smtp_config = provider.smtp_config(&account.email);

        // ─── 构建"发件人"字段 ───
        // 格式: "显示名称 <邮箱地址>" 或仅邮箱地址
        let from = account
            .display_name
            .map(|n| format!("{} <{}>", n, account.email))
            .unwrap_or_else(|| account.email.clone());

        // ─── 获取认证凭证 ───
        let mail_auth_type = &provider.as_ref().provider_info().auth_type;
        let credentials = self
            .auth
            .get_credentials(&account.email, mail_auth_type, Some(&account.provider))
            .await?;

        // ─── 根据认证类型发送邮件 ───
        let result = match &credentials {
            // 密码认证
            crate::domain::auth::Credentials::Password(pwd) => {
                crate::infrastructure::protocols::smtp::SmtpClient::send_email(
                    &smtp_config,
                    &account.email,
                    pwd,
                    &from,
                    &req.to,
                    &req.cc,
                    &req.bcc,
                    &req.subject,
                    &req.body_html,
                    &req.body_text,
                )
                .await
            }
            // OAuth2 认证（使用 XOAUTH2 机制）
            crate::domain::auth::Credentials::OAuth2 { access_token } => {
                crate::infrastructure::protocols::smtp::SmtpClient::send_email_xoauth2(
                    &smtp_config,
                    &account.email,
                    access_token,
                    &from,
                    &req.to,
                    &req.cc,
                    &req.bcc,
                    &req.subject,
                    &req.body_html,
                    &req.body_text,
                )
                .await
            }
        };

        tracing::info!(account_id = req.account_id, "邮件发送成功");
        result
    }

    /// 删除账号指定文件夹的所有邮件
    ///
    /// 这通常用于文件夹清空操作，如清空垃圾箱。
    ///
    /// # 工作流程
    ///
    /// 1. 获取该文件夹的所有邮件 ID
    /// 2. 删除这些邮件的附件（如果有）
    /// 3. 删除所有邮件记录
    /// 4. 返回删除的邮件数量
    ///
    /// # 参数
    ///
    /// - `account_id`: 账号 ID
    /// - `folder`: 文件夹名称
    ///
    /// # 返回
    ///
    /// 返回删除的邮件数量。
    ///
    /// # 使用场景
    ///
    /// - 用户清空垃圾箱
    /// - 用户清空已删除文件夹
    /// - 删除账号前的数据清理
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 清空垃圾箱
    /// let count = email_service.delete_account_folder_emails(1, "Trash").await?;
    /// println!("已清空垃圾箱，删除了 {} 封邮件", count);
    /// ```
    pub async fn delete_account_folder_emails(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<usize, MailError> {
        let deleted_count =
            email_repo::delete_folder_contents(&self.db, account_id, folder).await? as usize;

        tracing::info!(
            "已删除账号文件夹的所有邮件: account_id={}, folder={}, count={}",
            account_id,
            folder,
            deleted_count
        );

        Ok(deleted_count)
    }
}

pub(crate) async fn local_folder_registry_inputs(
    db: &DbConn,
    account_id: i32,
) -> Result<(Vec<RemoteFolder>, Vec<(String, FolderCategory)>), MailError> {
    let mut names = sync_repo::distinct_folders_by_account(db, account_id).await?;
    names.extend(email_repo::distinct_folders_by_account(db, account_id).await?);
    let remote = names
        .into_iter()
        .map(|n| RemoteFolder {
            name: n,
            special_use: vec![],
            no_select: false,
        })
        .collect();
    let known_categories = sync_repo::folder_categories_by_account(db, account_id)
        .await?
        .into_iter()
        .filter_map(|(folder, category)| FolderCategory::parse(&category).map(|cat| (folder, cat)))
        .collect();
    Ok((remote, known_categories))
}

async fn resolve_category_folders_for_account(
    db: &DbConn,
    account: &crate::infrastructure::storage::models::accounts::Model,
    category: &EmailCategory,
) -> Result<Vec<String>, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported(
            "未找到provider pool".into(),
        ))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let (local_names, known_categories) = local_folder_registry_inputs(db, account.id).await?;
    let registry = FolderRegistry::builder()
        .remote_folders(local_names)
        .known_categories(known_categories)
        .provider_mapping(provider.folder_mapping())
        .build();
    let cat = match FolderCategory::from_email_category(category) {
        Some(cat) => cat,
        None => return Ok(Vec::new()),
    };
    Ok(registry.resolve(cat))
}

// ═════════════════════════════════════════════════════════════════════════
// 辅助函数
// ═════════════════════════════════════════════════════════════════════════

/// 内部辅助函数：将数据库模型转换为 DTO
///
/// 将数据库查询返回的 emails::Model 转换为前端可展示的 EmailDto。
///
/// # 参数
///
/// - `emails`: 数据库邮件模型向量
///
/// # 返回
///
/// 返回转换后的 EmailDto 向量
///
/// # 转换说明
///
/// - 处理 Optional 类型的默认值
/// - 附件标志根据附件表真实计算
/// - 保留所有重要字段信息
async fn convert_models_with_attachments(
    db: &DbConn,
    emails: Vec<emails::Model>,
) -> Result<Vec<EmailDto>, MailError> {
    let ids = emails.iter().map(|email| email.id).collect::<Vec<_>>();
    let with_attachments = attachment_repo::has_for_email_ids(db, ids).await?;
    Ok(emails
        .into_iter()
        .map(|email| {
            let has_attachments = with_attachments.contains(&email.id);
            email_model_to_dto(&email, has_attachments)
        })
        .collect())
}

async fn convert_models_with_account_display(
    db: &DbConn,
    emails: Vec<emails::Model>,
) -> Result<Vec<EmailDto>, MailError> {
    let mut dtos = convert_models_with_attachments(db, emails).await?;
    let accounts = account_repo::list(db).await?;

    for dto in &mut dtos {
        if let Some(account) = accounts.iter().find(|account| account.id == dto.account_id) {
            dto.account_email = Some(account.email.clone());
            dto.account_display_name = account.display_name.clone();
        }
    }

    Ok(dtos)
}

fn email_model_to_dto(email: &emails::Model, has_attachments: bool) -> EmailDto {
    EmailDto {
        id: email.id,
        account_id: email.account_id,
        folder: email.folder.clone(),
        uid: email.uid,
        subject: email.subject.clone(),
        sender_name: email.sender_name.clone(),
        sender_email: email.sender_email.clone(),
        account_email: None,
        account_display_name: None,
        preview: email.preview.clone(),
        is_read: email.is_read.unwrap_or(false),
        is_starred: email.is_starred.unwrap_or(false),
        sent_at: email.sent_at,
        has_attachments,
    }
}

fn email_model_to_detail(email: emails::Model, attachments: Vec<AttachmentDto>) -> EmailDetail {
    let has_attachments = !attachments.is_empty();
    let email_dto = email_model_to_dto(&email, has_attachments);
    EmailDetail {
        email: email_dto,
        recipient_emails: email.recipient_emails,
        cc_emails: email.cc_emails,
        body_text: email.body_text,
        body_html: email.body_html,
        attachments,
    }
}

fn email_model_to_detail_with_account(
    email: emails::Model,
    attachments: Vec<AttachmentDto>,
    account: &accounts::Model,
) -> EmailDetail {
    let mut detail = email_model_to_detail(email, attachments);
    detail.email.account_email = Some(account.email.clone());
    detail.email.account_display_name = account.display_name.clone();
    detail
}
