//!
//! 邮件操作命令模块
//!
//! 本模块提供与邮件收发、搜索、管理相关的 Tauri 命令。
//! 这些命令允许前端用户对邮件执行完整的操作，包括：
//!
//! 主要功能分类：
//! 1. 邮件列表获取：
//!    - list_emails: 获取指定文件夹中的邮件列表（支持分页）
//!    - list_emails_by_category: 按分类获取邮件（收件箱、已发送、草稿箱等）
//!
//! 2. 邮件详情和搜索：
//!    - get_email: 获取单封邮件的完整详情（包含正文、附件等）
//!    - search_emails: 全文搜索邮件（支持跨账号搜索）
//!
//! 3. 邮件状态操作：
//!    - mark_as_read: 标记邮件为已读/未读
//!    - toggle_star: 切换邮件的星标状态（收藏）
//!
//! 4. 邮件管理：
//!    - delete_emails: 删除一封或多封邮件（第一阶段移动到垃圾箱）
//!    - move_email_to_folder: 移动邮件到指定文件夹
//!
//! 5. 邮件发送：
//!    - send_email: 发送新邮件（支持附件、抄送、密送等）
//!
//! 架构说明：
//! - 这些命令充当应用层的控制器，接收前端请求并转发给 EmailService
//! - 命令函数不包含业务逻辑，只负责参数验证、日志记录和结果返回
//! - 使用 tauri::State 注入 EmailService 实例，实现依赖注入
//!
//! 数据流：
//! 前端 invoke 调用 → 命令函数 → EmailService → 数据库/IMAP/SMTP
//!
//! 性能优化：
//! - list_emails 和 list_emails_by_category 支持分页，避免一次性加载大量邮件
//! - search_emails 支持限制结果数量
//! - 邮件列表只返回基本信息，详情需要单独调用 get_email
//!
//! 错误处理：
//! - 所有命令返回 Result<T, MailError>
//! - MailError 包含详细的错误信息，便于前端展示给用户
//!
//! 安全说明：
//! - 邮件内容在传输过程中通过 Tauri IPC 通道加密
//! - 敏感操作（如删除）建议在前端添加确认对话框
//!
//! 依赖项：
//! - EmailService: 邮件服务层，处理邮件的业务逻辑
//! - EmailListResponse: 邮件列表响应（包含邮件数组和总数）
//! - EmailDetail: 邮件详细信息
//! - EmailCategory: 邮件分类枚举
//! - SendEmailRequest: 发送邮件请求
//! - SearchResult: 搜索结果
//!

use crate::error::MailError;
use crate::infrastructure::storage::search::SearchResult;
use crate::service::attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto};
use crate::service::email_service::{
    EmailCategory, EmailDetail, EmailListResponse, ReloadEmailResult, SendEmailRequest,
    SendEmailResponse,
};

///
/// 获取邮件列表
///
/// 从指定账号的文件夹中获取邮件列表，支持分页功能。
/// 此命令是邮件应用的核心功能，用于在收件箱、已发送、草稿箱等文件夹中浏览邮件。
///
/// 功能说明：
/// 1. 从数据库查询指定文件夹中的邮件
/// 2. 支持分页查询，避免一次性加载过多数据
/// 3. 返回邮件的基本信息（不包含正文，详情需调用 get_email）
/// 4. 支持按日期、未读状态等排序（由服务层处理）
///
/// 参数：
/// - service: EmailService 实例，通过 Tauri 的 State 机制注入
/// - account_id: 账号 ID，指定要查询哪个账号
/// - folder: 文件夹名称，如 "INBOX"、"Sent"、"Drafts" 等
/// - page: 页码，从 0 开始
/// - limit: 每页数量，建议 20-50 之间
///
/// 返回值：
/// - Ok(EmailListResponse): 邮件列表响应，包含：
///   - emails: 邮件数组，每个元素包含：
///     - id: 邮件唯一标识
///     - subject: 邮件主题
///     - from: 发件人
///     - to: 收件人列表
///     - date: 邮件日期
///     - is_read: 是否已读
///     - is_starred: 是否加星标
///     - has_attachments: 是否有附件
///     - snippet: 邮件摘要（前几行内容）
///   - total: 邮件总数
/// - Err(MailError):
///   - NotFound: 账号或文件夹不存在
///   - DatabaseError: 数据库查询错误
///
/// 使用场景：
/// 1. 用户打开收件箱/已发送等文件夹时
/// 2. 用户滚动到底部加载更多邮件时
/// 3. 刷新邮件列表时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const result = await invoke('list_emails', {
///   account_id: 1,
///   folder: 'INBOX',
///   page: 0,
///   limit: 20
/// });
/// console.log(`共 ${result.total} 封邮件，当前显示 ${result.emails.length} 封`);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn list_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<EmailListResponse, MailError> {
    // 记录调试日志，包含关键查询参数
    tracing::debug!(account_id, folder = %folder, page, limit, "命令: 列出邮件");

    // 调用服务层获取邮件列表
    // 该方法会：
    // 1. 验证账号和文件夹存在
    // 2. 计算分页偏移量（offset = page * limit）
    // 3. 从数据库查询邮件
    // 4. 返回邮件列表和总数
    let result = service.list(account_id, &folder, page, limit).await?;

    // 记录返回结果统计信息
    tracing::debug!(
        account_id,
        folder = %folder,
        total = result.total,
        returned = result.emails.len(),
        "邮件列表"
    );

    Ok(result)
}

///
/// 按分类获取邮件列表
///
/// 根据邮件分类（收件箱、已发送、已加星标、已归档等）获取邮件列表。
/// 与 list_emails 不同，此命令不指定文件夹，而是根据业务逻辑跨文件夹查询。
///
/// 支持的分类：
/// - Inbox: 收件箱（聚合所有收件邮件）
/// - Sent: 已发送邮件
/// - Starred: 已加星标的邮件
/// - Archived: 已归档的邮件
/// - Drafts: 草稿箱
/// - All: 所有邮件
/// - Spam: 垃圾邮件
///
/// 功能说明：
/// 1. 根据分类类型确定查询条件
/// 2. 可能跨多个文件夹查询（如 All、Starred）
/// 3. 支持分页，避免一次性加载过多数据
/// 4. 返回邮件的基本信息
///
/// 参数：
/// - service: EmailService 实例
/// - account_id: 账号 ID
/// - category: 邮件分类（EmailCategory 枚举）
/// - page: 页码，从 0 开始
/// - limit: 每页数量
///
/// 返回值：
/// - Ok(EmailListResponse): 邮件列表响应
/// - Err(MailError): 查询失败时的错误信息
///
/// 使用场景：
/// 1. 用户点击"收件箱"、"已发送"等侧边栏菜单时
/// 2. 用户点击"星标邮件"查看收藏邮件时
/// 3. 用户查看"所有邮件"时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// // 获取已加星标的邮件
/// const result = await invoke('list_emails_by_category', {
///   account_id: 1,
///   category: 'Starred',
///   page: 0,
///   limit: 20
/// });
/// ```
///
/// 按分类加载邮件列表
#[tauri::command]
#[specta::specta]
pub async fn list_emails_by_category(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    account_id: i32,
    category: EmailCategory,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<EmailListResponse, MailError> {
    // 记录调试日志，使用 ?trait 格式输出枚举值
    tracing::debug!(account_id, category = ?category, page, limit, unread_only, "命令: 按分类列出邮件");

    // 调用服务层按分类查询邮件
    // 该方法会：
    // 1. 根据分类类型确定查询 SQL
    // 2. 执行数据库查询
    // 3. 返回分页结果
    let result = service
        .list_by_category(account_id, category.clone(), page, limit, unread_only)
        .await?;

    // 记录返回结果统计信息
    tracing::debug!(
        account_id,
        category = ?category,
        unread_only,
        total = result.total,
        returned = result.emails.len(),
        "分类邮件列表"
    );

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn list_emails_by_category_for_all_accounts(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    category: EmailCategory,
    page: usize,
    limit: usize,
    unread_only: bool,
) -> Result<EmailListResponse, MailError> {
    tracing::debug!(category = ?category, page, limit, unread_only, "命令: 按分类列出所有账号邮件");
    service
        .list_by_category_for_all_accounts(category, page, limit, unread_only)
        .await
}

///
/// 获取邮件详情
///
/// 根据邮件 ID 获取单封邮件的完整详细信息。
/// 此命令用于在邮件阅读视图中显示邮件的完整内容。
///
/// 功能说明：
/// 1. 从数据库查询邮件的完整信息
/// 2. 返回邮件正文（HTML 和纯文本两种格式）
/// 3. 返回附件列表（文件名、大小、类型等）
/// 4. 返回完整的邮件头（From, To, Cc, Bcc, Date, Subject 等）
/// 5. 自动标记邮件为已读
///
/// 参数：
/// - service: EmailService 实例
/// - id: 邮件 ID
///
/// 返回值：
/// - Ok(EmailDetail): 邮件详细信息，包含：
///   - id: 邮件 ID
///   - subject: 邮件主题
///   - from: 发件人信息（邮箱和名称）
///   - to: 收件人列表
///   - cc: 抄送列表
///   - bcc: 密送列表
///   - date: 邮件发送时间
///   - body_html: HTML 格式的邮件正文
///   - body_text: 纯文本格式的邮件正文
///   - attachments: 附件列表
///   - labels: 邮件标签列表
///   - folder: 所在文件夹
///   - is_read: 是否已读
///   - is_starred: 是否加星标
/// - Err(MailError):
///   - NotFound: 邮件不存在
///   - DatabaseError: 数据库查询错误
///
/// 使用场景：
/// 1. 用户点击邮件列表中的某封邮件时
/// 2. 回复或转发邮件时（需要获取原始邮件内容）
/// 3. 查看邮件附件时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// const email = await invoke('get_email', { id: 123 });
/// console.log('主题:', email.subject);
/// console.log('正文:', email.body_html);
/// console.log('附件数量:', email.attachments.length);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn get_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    id: i32,
) -> Result<EmailDetail, MailError> {
    // 记录调试日志
    tracing::debug!(id, "命令: 获取邮件详情");

    // 调用服务层获取邮件详情
    // 该方法会：
    // 1. 查询邮件完整信息
    // 2. 自动标记为已读
    // 3. 返回详细信息
    service.get(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn reload_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<ReloadEmailResult, MailError> {
    tracing::info!(email_id, "命令: 重新加载邮件");
    service.reload_email(email_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn ensure_attachment_cached(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
) -> Result<AttachmentDto, MailError> {
    service.ensure_cached(attachment_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_attachment_as(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
    target_path: String,
) -> Result<(), MailError> {
    service.save_as(attachment_id, target_path).await
}

#[tauri::command]
#[specta::specta]
pub async fn open_attachment(
    service: tauri::State<'_, AttachmentService>,
    attachment_id: i32,
) -> Result<(), MailError> {
    service.open(attachment_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_inline_attachments(
    service: tauri::State<'_, AttachmentService>,
    email_id: i32,
) -> Result<Vec<InlineAttachmentDto>, MailError> {
    service.resolve_inline_images(email_id).await
}

///
/// 搜索邮件
///
/// 根据关键词全文搜索邮件。
/// 此命令使用全文搜索引擎，支持跨账号、跨文件夹搜索。
///
/// 功能说明：
/// 1. 在邮件的主题、发件人、收件人、正文中搜索关键词
/// 2. 支持跨多个账号搜索（如果未指定 account_id）
/// 3. 支持限制结果数量
/// 4. 返回匹配度高的邮件优先
/// 5. 支持模糊搜索和部分匹配
///
/// 参数：
/// - service: EmailService 实例
/// - query: 搜索关键词，可以是：
///   - 邮件主题的部分内容
///   - 发件人或收件人的邮箱
///   - 邮件正文的关键词
/// - account_id: 可选，指定账号 ID。如果为 None，则搜索所有账号
/// - limit: 可选，限制返回结果数量，默认 50
///
/// 返回值：
/// - Ok(Vec<SearchResult>): 搜索结果列表，每个元素包含：
///   - email_id: 邮件 ID
///   - account_id: 账号 ID
///   - subject: 邮件主题（高亮匹配的关键词）
///   - from: 发件人
///   - date: 邮件日期
///   - snippet: 匹配段落的摘要
///   - score: 匹配分数（相关性）
/// - Err(MailError): 搜索失败时的错误信息
///
/// 搜索示例：
/// - "meeting" - 搜索包含 "meeting" 的所有邮件
/// - "from:john@example.com" - 搜索来自特定发件人的邮件
/// - "subject:report" - 搜索主题包含 "report" 的邮件
///
/// 使用场景：
/// 1. 用户在搜索框输入关键词时
/// 2. 高级搜索功能
/// 3. 查找特定邮件时
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// // 搜索所有账号中包含 "meeting" 的邮件
/// const results = await invoke('search_emails', {
///   query: 'meeting',
///   account_id: null,
///   limit: 20
/// });
/// console.log(`找到 ${results.length} 封邮件`);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn search_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<SearchResult>, MailError> {
    // 记录信息日志
    tracing::info!(query = %query, account_id, limit, "命令: 搜索邮件");

    // 调用服务层执行搜索
    // 该方法会：
    // 1. 解析搜索查询
    // 2. 执行全文搜索
    // 3. 按相关性排序
    // 4. 限制结果数量
    let results = service.search(&query, account_id, limit).await?;

    // 记录搜索结果数量
    tracing::info!(query = %query, count = results.len(), "搜索完成");

    Ok(results)
}

///
/// 标记邮件已读/未读状态
///
/// 设置邮件的已读状态。
/// 此命令用于用户手动标记邮件或系统自动标记邮件。
///
/// 功能说明：
/// 1. 更新邮件的 is_read 字段
/// 2. 如果标记为已读，自动更新文件夹的未读计数
/// 3. 如果标记为未读，增加文件夹的未读计数
/// 4. 支持批量操作（需要在服务层实现）
///
/// 参数：
/// - service: EmailService 实例
/// - email_id: 邮件 ID
/// - is_read: 是否已读
///   - true: 标记为已读
///   - false: 标记为未读
///
/// 返回值：
/// - Ok(()): 标记成功
/// - Err(MailError):
///   - NotFound: 邮件不存在
///   - DatabaseError: 数据库更新错误
///
/// 使用场景：
/// 1. 用户点击邮件列表中的邮件时（自动标记为已读）
/// 2. 用户右键菜单选择"标记为未读"
/// 3. 用户批量操作"全部标记为已读"
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// // 标记邮件为已读
/// await invoke('mark_as_read', {
///   email_id: 123,
///   is_read: true
/// });
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn mark_as_read(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    is_read: bool,
) -> Result<(), MailError> {
    // 记录信息日志
    tracing::info!(email_id, is_read, "命令: 标记已读/未读");

    // 调用服务层更新已读状态
    // 该方法会：
    // 1. 更新邮件的 is_read 字段
    // 2. 更新文件夹的未读计数
    service.mark_as_read(email_id, is_read).await
}

///
/// 切换邮件星标状态
///
/// 切换邮件的收藏（星标）状态。
/// 如果邮件已加星标，则取消；如果未加星标，则添加。
///
/// 功能说明：
/// 1. 查询邮件当前星标状态
/// 2. 切换状态（true ↔ false）
/// 3. 更新数据库
/// 4. 返回新的星标状态
///
/// 参数：
/// - service: EmailService 实例
/// - email_id: 邮件 ID
///
/// 返回值：
/// - Ok(bool): 切换后的星标状态
///   - true: 已加星标
///   - false: 未加星标
/// - Err(MailError):
///   - NotFound: 邮件不存在
///   - DatabaseError: 数据库更新错误
///
/// 使用场景：
/// 1. 用户点击邮件列表中的星标图标时
/// 2. 用户在邮件详情页点击星标按钮时
/// 3. 快捷键切换星标（如按 S 键）
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// // 切换星标状态
/// const isStarred = await invoke('toggle_star', { email_id: 123 });
/// if (isStarred) {
///   console.log('邮件已加星标');
/// } else {
///   console.log('邮件已取消星标');
/// }
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn toggle_star(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<bool, MailError> {
    // 记录信息日志
    tracing::info!(email_id, "命令: 切换星标");

    // 调用服务层切换星标状态
    // 该方法会：
    // 1. 获取当前星标状态
    // 2. 切换状态
    // 3. 更新数据库
    // 4. 返回新状态
    service.toggle_star(email_id).await
}

///
/// 删除邮件
///
/// 第一阶段删除语义为移动到服务商配置的 Trash 文件夹。
/// 不执行永久删除，不执行 EXPUNGE。
///
/// 功能说明：
/// 1. 验证邮件存在
/// 2. 将邮件移动到服务商配置的 Trash 文件夹
/// 3. 远端移动成功后更新本地文件夹
/// 4. 第一阶段仅支持单封删除，避免远端批量移动产生部分成功状态
///
/// 参数：
/// - service: EmailService 实例
/// - email_ids: 要删除的邮件 ID 列表，第一阶段仅支持单封邮件
///
/// 返回值：
/// - Ok(usize): 实际删除的邮件数量
/// - Err(MailError):
///   - NotFound: 部分邮件不存在
///   - DatabaseError: 数据库操作错误
///
/// 使用场景：
/// 1. 用户点击删除按钮删除单封邮件
/// 2. 用户删除当前邮件
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// import { confirm } from '@tauri-apps/api/dialog';
///
/// // 删除单封邮件
/// await invoke('delete_emails', { email_ids: [123] });
///
/// const deleted = await invoke('delete_emails', { email_ids: [123] });
/// console.log(`已删除 ${deleted} 封邮件`);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn delete_emails(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_ids: Vec<i32>,
) -> Result<usize, MailError> {
    // 记录信息日志
    tracing::info!(count = email_ids.len(), ids = ?email_ids, "命令: 删除邮件");

    // 调用服务层删除邮件：第一阶段只移动到服务商 Trash 文件夹。
    let deleted = service.delete(email_ids).await?;

    // 记录删除结果
    tracing::info!(deleted, "邮件已删除");

    Ok(deleted)
}

///
/// 移动邮件到指定文件夹
///
/// 将邮件从当前文件夹移动到目标文件夹。
/// 此命令用于邮件分类和归档。
///
/// 功能说明：
/// 1. 验证邮件和目标文件夹存在
/// 2. 更新邮件的 folder 字段
/// 3. 更新原文件夹和目标文件夹的统计信息
/// 4. 处理特殊文件夹的规则（如垃圾箱、已归档等）
///
/// 常见文件夹名称：
/// - INBOX: 收件箱
/// - Sent: 已发送
/// - Drafts: 草稿箱
/// - Trash: 垃圾箱
/// - Spam: 垃圾邮件
/// - Archive: 归档
/// - 用户自定义的文件夹名称
///
/// 参数：
/// - service: EmailService 实例
/// - email_id: 邮件 ID
/// - folder: 目标文件夹名称
///
/// 返回值：
/// - Ok(()): 移动成功
/// - Err(MailError):
///   - NotFound: 邮件或文件夹不存在
///   - DatabaseError: 数据库更新错误
///   - ValidationError: 移动操作无效（如移动到同一文件夹）
///
/// 使用场景：
/// 1. 用户通过拖放移动邮件到其他文件夹
/// 2. 用户使用右键菜单"移动到..."
/// 3. 用户点击"归档"按钮（移动到 Archive 文件夹）
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
/// // 归档邮件（移动到 Archive 文件夹）
/// await invoke('move_email_to_folder', {
///   email_id: 123,
///   folder: 'Archive'
/// });
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn move_email_to_folder(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
    folder: String,
) -> Result<(), MailError> {
    // 记录信息日志
    tracing::info!(email_id, folder = %folder, "命令: 移动邮件");

    // 调用服务层移动邮件
    // 该方法会：
    // 1. 验证邮件和文件夹存在
    // 2. 更新邮件的文件夹
    // 3. 更新文件夹统计
    service.move_to_folder(email_id, &folder).await
}

///
/// 归档邮件
///
/// 将邮件移动到当前服务商配置的归档文件夹。该操作先写入 IMAP 远端，
/// 远端成功后再更新本地邮件文件夹。
#[tauri::command]
#[specta::specta]
pub async fn archive_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    email_id: i32,
) -> Result<(), MailError> {
    tracing::info!(email_id, "命令: 归档邮件");
    service.archive(email_id).await
}

///
/// 发送邮件
///
/// 创建并发送新邮件。
/// 此命令是邮件发送的核心功能，支持完整的邮件发送功能。
///
/// 功能说明：
/// 1. 构建符合 RFC 标准的邮件
/// 2. 通过 SMTP 协议发送邮件
/// 3. 支持附件上传
/// 4. 支持抄送（CC）和密送（BCC）
/// 5. 支持纯文本和 HTML 两种格式
/// 6. 自动保存到"已发送"文件夹
///
/// 发送流程：
/// 1. 验证邮件内容（主题、收件人等）
/// 2. 编码邮件内容（MIME 编码）
/// 3. 连接 SMTP 服务器
/// 4. 发送邮件
/// 5. 保存到本地数据库（已发送文件夹）
/// 6. 返回发送结果
///
/// 参数：
/// - service: EmailService 实例
/// - request: SendEmailRequest 对象，包含：
///   - account_id: 发送账号的 ID
///   - to: 收件人列表（必填）
///   - cc: 抄送列表（可选）
///   - bcc: 密送列表（可选）
///   - subject: 邮件主题（必填）
///   - body_text: 纯文本正文（可选）
///   - body_html: HTML 正文（可选）
///   - attachments: 附件列表（可选）
///   - in_reply_to: 回复的邮件 ID（可选，用于邮件线程）
///
/// 返回值：
/// - Ok(String): 发送成功，返回邮件的唯一标识（Message-ID）
/// - Err(MailError):
///   - ValidationError: 邮件内容无效（如缺少收件人）
///   - AuthError: SMTP 认证失败
///   - ConnectionError: 无法连接到 SMTP 服务器
///   - SendError: 邮件发送失败
///
/// 安全说明：
/// - 使用 TLS/SSL 加密传输
/// - 密送（BCC）的收件人对其他收件人不可见
/// - 支持数字签名（可选，需在服务层实现）
///
/// 使用场景：
/// 1. 用户撰写新邮件
/// 2. 回复邮件（自动填充原始邮件信息）
/// 3. 转发邮件
/// 4. 发送带附件的邮件
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// const messageId = await invoke('send_email', {
///   request: {
///     account_id: 1,
///     to: ['recipient@example.com'],
///     cc: ['cc@example.com'],
///     subject: '会议邀请',
///     body_html: '<p>请参加明天的会议...</p>',
///     attachments: [
///       {
///         name: 'agenda.pdf',
///         path: '/path/to/agenda.pdf',
///         content_type: 'application/pdf'
///       }
///     ]
///   }
/// });
/// console.log('邮件发送成功，Message-ID:', messageId);
/// ```
///
#[tauri::command]
#[specta::specta]
pub async fn send_email(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    request: SendEmailRequest,
) -> Result<SendEmailResponse, MailError> {
    // 记录信息日志，包含关键的发送信息
    tracing::info!(
        account_id = request.account_id,
        to = request.to.len(),
        cc = request.cc.len(),
        bcc = request.bcc.len(),
        subject = %request.subject,
        "命令: 发送邮件"
    );

    // 调用服务层发送邮件
    // 该方法会：
    // 1. 验证邮件内容
    // 2. 构建邮件（MIME 编码）
    // 3. 连接 SMTP 服务器
    // 4. 发送邮件
    // 5. 保存到已发送文件夹
    let result = service.send(request).await?;

    // 记录发送成功
    tracing::info!("邮件发送成功");

    Ok(result)
}
