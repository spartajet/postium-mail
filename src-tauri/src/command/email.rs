//! 邮件操作 Commands
//!
//! 提供邮件的增删改查功能，包括：
//! - 分页获取邮件列表
//! - 获取邮件详情
//! - 全文搜索邮件
//! - 标记已读/未读
//! - 星标管理
//! - 批量删除
//! - 移动到文件夹
//!
//! # 分页说明
//!
//! 邮件列表采用分页加载，避免一次性加载大量邮件导致性能问题。
//! - `page`: 页码，从 0 开始
//! - `limit`: 每页数量，建议 20-50

use super::{DatabaseState, ProviderPoolState};
use crate::storage::{self, models::email, service::email as email_service, service::AccountRepository};
use sea_orm::EntityTrait;

/// 分页获取邮件列表
///
/// 从指定账号的文件夹中获取邮件列表，支持分页。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称（如 "INBOX", "Sent", "Drafts"）
/// * `page` - 页码，从 0 开始
/// * `limit` - 每页邮件数量
///
/// # 返回
/// 成功时返回邮件列表响应（EmailListResponse），包含：
/// - `items`: 邮件列表
/// - `total`: 总邮件数
/// - `page`: 当前页码
/// - `page_count`: 总页数
///
/// 失败时返回错误信息字符串
///
/// # 示例
/// ```rust,no_run
/// use crate::command::email::list_emails;
///
/// // 获取 INBOX 的第一页，每页 20 封
/// let result = list_emails(state, 1, "INBOX".to_string(), 0, 20).await?;
/// ```
#[tauri::command]
pub async fn list_emails(
    state: tauri::State<'_, DatabaseState>,
    provider_pool_state: tauri::State<'_, ProviderPoolState>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<storage::EmailListResponse, String> {
    tracing::info!("========== 邮件列表请求 ==========");
    tracing::info!(
        "参数: account_id={}, folder='{}', page={}, limit={}",
        account_id,
        folder,
        page,
        limit
    );

    let db = state.clone_conn();

    // 获取账号信息
    let account = AccountRepository::get_by_id(&db, account_id)
        .await
        .map_err(|e| {
            tracing::error!("获取账号信息失败: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            let msg = format!("账号 {} 不存在", account_id);
            tracing::error!("{}", msg);
            msg
        })?;

    // 获取服务商配置
    let provider_pool = provider_pool_state.clone_pool();
    let provider = provider_pool
        .find_provider_by_id(&account.provider)
        .ok_or_else(|| {
            let msg = format!("未找到服务商: {}", account.provider);
            tracing::error!("{}", msg);
            msg
        })?;

    let folder_mapping = provider.folder_mapping();

    // 调试：打印文件夹映射信息
    tracing::info!(
        "文件夹映射: inbox={:?}, sent={:?}, drafts={:?}, spam={:?}, trash={:?}, archive={:?}",
        folder_mapping.inbox,
        folder_mapping.sent,
        folder_mapping.drafts,
        folder_mapping.spam,
        folder_mapping.trash,
        folder_mapping.archive
    );
    tracing::info!("查询文件夹: {} (前端标准名称)", folder);

    let result = email_service::list(&db, account_id, &folder, page as u64, limit as u64, &folder_mapping)
        .await
        .map_err(|e| {
            tracing::error!("查询失败: {}", e);
            e.to_string()
        })?;

    // 调试：记录查询结果
    tracing::info!(
        "查询成功: account_id={}, folder={}, 返回 {} 封邮件, 总计 {} 封",
        account_id,
        folder,
        result.items.len(),
        result.total
    );

    Ok(result)
}

/// 获取邮件详情
///
/// 根据邮件 ID 获取完整的邮件详情，包括正文、附件等信息。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `provider_pool_state` - 服务商池状态
/// * `id` - 邮件 ID
///
/// # 返回
/// - 成功且邮件存在时：返回邮件详情（EmailDetail）
/// - 成功但邮件不存在时：返回错误
/// - 失败时：返回错误信息字符串
///
/// # EmailDetail 包含
/// - 邮件基本信息（发件人、收件人、主题、日期）
/// - 邮件正文（HTML 和纯文本）
/// - 附件列表
/// - 邮件标志（已读、星标等）
#[tauri::command]
pub async fn get_email(
    state: tauri::State<'_, DatabaseState>,
    provider_pool_state: tauri::State<'_, ProviderPoolState>,
    id: i32,
) -> Result<storage::EmailDetail, String> {
    let db = state.clone_conn();

    // 获取邮件信息以确定账号
    let email_model = email::Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| {
            tracing::error!("获取邮件信息失败: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            let msg = format!("邮件 {} 不存在", id);
            tracing::error!("{}", msg);
            msg
        })?;

    // 获取账号信息
    let account = AccountRepository::get_by_id(&db, email_model.account_id)
        .await
        .map_err(|e| {
            tracing::error!("获取账号信息失败: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            let msg = format!("账号 {} 不存在", email_model.account_id);
            tracing::error!("{}", msg);
            msg
        })?;

    // 获取服务商配置
    let provider_pool = provider_pool_state.clone_pool();
    let provider = provider_pool
        .find_provider_by_id(&account.provider)
        .ok_or_else(|| {
            let msg = format!("未找到服务商: {}", account.provider);
            tracing::error!("{}", msg);
            msg
        })?;

    let folder_mapping = provider.folder_mapping();

    email_service::get_detail(&db, id, &folder_mapping)
        .await
        .map_err(|e| e.to_string())
}

/// 全文搜索邮件
///
/// 使用 FTS（Full-Text Search）在邮件内容中搜索关键字。
/// 支持搜索发件人、收件人、主题、邮件正文等字段。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `query` - 搜索查询字符串（支持简单的关键词搜索）
/// * `account_id` - 可选，限定搜索范围到指定账号
/// * `limit` - 可选，限制返回结果数量
///
/// # 返回
/// 成功时返回搜索结果列表（SearchResult），每项包含：
/// - 邮件 ID
/// - 匹配的文本片段
/// - 相关性评分
///
/// 失败时返回错误信息字符串
///
/// # 搜索范围
/// - 不指定 `account_id`: 搜索所有账号的邮件
/// - 指定 `account_id`: 仅搜索该账号的邮件
///
/// # 示例
/// ```rust,no_run
/// use crate::command::email::search_emails_fts;
///
/// // 搜索所有账号中包含 "重要" 的邮件
/// let results = search_emails_fts(state, "重要".to_string(), None, Some(20)).await?;
///
/// // 搜索特定账号
/// let results = search_emails_fts(state, "项目".to_string(), Some(1), Some(10)).await?;
/// ```
#[tauri::command]
pub async fn search_emails_fts(
    state: tauri::State<'_, DatabaseState>,
    query: String,
    account_id: Option<i32>,
    limit: Option<u64>,
) -> Result<Vec<storage::SearchResult>, String> {
    let db = state.clone_conn();
    storage::SearchService::search_emails(&db, account_id, &query, limit)
        .await
        .map_err(|e| e.to_string())
}

/// 标记邮件已读/未读状态
///
/// 更新指定邮件的已读状态。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `email_id` - 邮件 ID
/// * `is_read` - true 设为已读，false 设为未读
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 注意
/// - 此操作会更新数据库中的邮件状态
/// - 如果启用了 IMAP 同步，状态变更可能会同步到服务器
#[tauri::command]
pub async fn mark_as_read(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    is_read: bool,
) -> Result<(), String> {
    let db = state.clone_conn();
    email_service::update_read_status(&db, email_id, is_read)
        .await
        .map_err(|e| e.to_string())
}

/// 切换邮件星标状态
///
/// 切换指定邮件的星标（标记/取消标记）。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `email_id` - 邮件 ID
///
/// # 返回
/// 成功时返回操作后的星标状态：
/// - `true`: 邮件已加星标
/// - `false`: 邮件未加星标
///
/// 失败时返回错误信息字符串
///
/// # 注意
/// - 此操作会切换当前状态，如果已星标则取消，否则添加星标
/// - 如果启用了 IMAP 同步，状态变更可能会同步到服务器的 FLAGGED 标志
#[tauri::command]
pub async fn toggle_star(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
) -> Result<bool, String> {
    let db = state.clone_conn();
    email_service::toggle_star(&db, email_id)
        .await
        .map_err(|e| e.to_string())
}

/// 批量删除邮件
///
/// 批量删除指定的邮件。删除操作会将邮件移动到废纸篓，
/// 如果邮件已在废纸篓中则永久删除。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `email_ids` - 要删除的邮件 ID 列表
///
/// # 返回
/// 成功时返回删除的邮件数量，失败时返回错误信息字符串
///
/// # 删除逻辑
/// - 如果邮件不在废纸篓文件夹：移动到废纸篓
/// - 如果邮件已在废纸篓文件夹：永久删除
///
/// # 注意
/// - 永久删除的邮件无法恢复
/// - 如果启用了 IMAP 同步，删除操作可能会同步到服务器
#[tauri::command]
pub async fn delete_emails(
    state: tauri::State<'_, DatabaseState>,
    email_ids: Vec<i32>,
) -> Result<usize, String> {
    let db = state.clone_conn();
    email_service::batch_delete(&db, email_ids)
        .await
        .map_err(|e| e.to_string())
}

/// 移动邮件到文件夹
///
/// 将指定邮件移动到目标文件夹。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `email_id` - 邮件 ID
/// * `folder` - 目标文件夹名称（如 "INBOX", "Archive", "Spam"）
///
/// # 返回
/// 成功时返回空值，失败时返回错误信息字符串
///
/// # 常见文件夹
/// - `INBOX`: 收件箱
/// - `Archive`: 归档
/// - `Spam`: 垃圾邮件
/// - `Trash`: 废纸篓
/// - `Drafts`: 草稿箱
/// - `Sent`: 已发送
///
/// # 注意
/// - 如果启用了 IMAP 同步，移动操作可能会同步到服务器
/// - 目标文件夹必须存在于账号中
#[tauri::command]
pub async fn move_email_to_folder(
    state: tauri::State<'_, DatabaseState>,
    email_id: i32,
    folder: String,
) -> Result<(), String> {
    let db = state.clone_conn();
    email_service::move_to_folder(&db, email_id, &folder)
        .await
        .map_err(|e| e.to_string())
}

/// 获取文件夹统计信息
///
/// 获取指定账号的所有文件夹统计信息，包括邮件数量和未读邮件数量。
///
/// # 参数
/// * `db_state` - 数据库连接状态
/// * `provider_pool_state` - 服务商池状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回文件夹统计列表（FolderStat），每项包含：
/// - `id`: 文件夹 ID（暂未使用，固定为 0）
/// - `account_id`: 账号 ID
/// - `name`: 文件夹名称（前端显示名称，如 "inbox"）
/// - `imap_name`: IMAP 文件夹名称（如 "INBOX"）
/// - `email_count`: 邮件总数
/// - `unread_count`: 未读邮件数量
///
/// 失败时返回错误信息字符串
///
/// # 文件夹列表
/// 从账号的 provider 配置中获取标准文件夹映射：
/// - `inbox`: 收件箱
/// - `sent`: 已发送
/// - `drafts`: 草稿箱
/// - `spam`: 垃圾邮件
/// - `trash`: 废纸篓
/// - `archive`: 归档
/// - `starred`: 星标邮件（虚拟文件夹）
#[tauri::command]
pub async fn get_folder_stats(
    db_state: tauri::State<'_, DatabaseState>,
    provider_pool_state: tauri::State<'_, ProviderPoolState>,
    account_id: i32,
) -> Result<Vec<email_service::FolderStat>, String> {
    tracing::info!("获取文件夹统计: account_id={}", account_id);

    let db = db_state.clone_conn();

    // 获取账号信息
    let account = AccountRepository::get_by_id(&db, account_id)
        .await
        .map_err(|e| {
            tracing::error!("获取账号信息失败: {}", e);
            e.to_string()
        })?
        .ok_or_else(|| {
            let msg = format!("账号 {} 不存在", account_id);
            tracing::error!("{}", msg);
            msg
        })?;

    // 获取服务商配置
    let provider_pool = provider_pool_state.clone_pool();
    let provider = provider_pool
        .find_provider_by_id(&account.provider)
        .ok_or_else(|| {
            let msg = format!("未找到服务商: {}", account.provider);
            tracing::error!("{}", msg);
            msg
        })?;

    let folder_mapping = provider.folder_mapping();

    email_service::get_folder_stats(&db, account_id, &folder_mapping)
        .await
        .map_err(|e| {
            tracing::error!("获取文件夹统计失败: {}", e);
            e.to_string()
        })
}
