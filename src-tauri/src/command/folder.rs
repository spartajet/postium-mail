//! 文件夹操作 Commands
//!
//! 提供文件夹的查询功能，包括：
//! - 获取账号的标准文件夹列表（从 provider 动态生成）
//!
//! # 标准文件夹
//!
//! 应用只展示 6 个标准文件夹：
//! - **inbox**: 收件箱
//! - **sent**: 已发送
//! - **drafts**: 草稿箱
//! - **spam**: 垃圾邮件
//! - **trash**: 已删除
//! - **archive**: 归档
//!
//! 文件夹配置由各个邮件服务商的 `provider.folder_mapping()` 提供，
//! 不存储在数据库中。

use super::DatabaseState;
use crate::storage;
use crate::providers::ProviderPool;

/// 标准文件夹 DTO（用于前端传输）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StandardFolderDto {
    /// 文件夹类型（标准名称）
    pub folder_type: String,
    /// IMAP 文件夹名称（实际在服务器上的名称）
    pub imap_name: String,
    /// 显示名称（本地化）
    pub display_name: String,
    /// 邮件数量（从 emails 表统计）
    pub email_count: i64,
    /// 未读邮件数量（从 emails 表统计）
    pub unread_count: i64,
}

/// 获取账号的标准文件夹列表
///
/// 返回指定账号的 6 个标准文件夹（inbox, sent, drafts, spam, trash, archive）。
/// 文件夹配置由邮件服务商的 `folder_mapping()` 提供，动态生成。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回标准文件夹列表（StandardFolderDto 数组），按固定顺序返回：
/// 1. inbox（收件箱）
/// 2. sent（已发送）
/// 3. drafts（草稿箱）
/// 4. spam（垃圾邮件）
/// 5. trash（已删除）
/// 6. archive（归档）
///
/// 每个文件夹包含：
/// - `folder_type`: 文件夹类型
/// - `imap_name`: IMAP 文件夹名称（如 "INBOX", "[Gmail]/Sent Mail"）
/// - `display_name`: 显示名称（如 "收件箱", "已发送"）
/// - `email_count`: 该文件夹的邮件总数
/// - `unread_count`: 该文件夹的未读邮件数
///
/// 失败时返回错误信息字符串
#[tauri::command]
pub async fn get_standard_folders(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<Vec<StandardFolderDto>, String> {
    let db = state.clone_conn();

    // 1. 获取账号信息
    let account = storage::AccountRepository::get_by_id(&db, account_id)
        .await
        .map_err(|e| format!("获取账号失败: {}", e))?
        .ok_or_else(|| format!("账号 {} 不存在", account_id))?;

    // 2. 检测服务商
    let provider_pool = ProviderPool::new();
    let provider = provider_pool
        .detect_provider(&account.email)
        .await
        .map_err(|e| format!("检测服务商失败: {}", e))?;

    // 3. 获取文件夹映射
    let folder_mapping = provider.folder_mapping();

    // 4. 生成标准文件夹列表
    let mut folders = Vec::new();

    // 标准文件夹配置
    let standard_folders = [
        ("inbox", "收件箱", &folder_mapping.inbox),
        ("sent", "已发送", &folder_mapping.sent),
        ("drafts", "草稿箱", &folder_mapping.drafts),
        ("spam", "垃圾邮件", &folder_mapping.spam),
        ("trash", "已删除", &folder_mapping.trash),
        ("archive", "归档", &folder_mapping.archive),
    ];

    for (folder_type, display_name, imap_names) in standard_folders {
        // 使用第一个 IMAP 名称（如果有的话）
        let imap_name = imap_names.first().cloned().unwrap_or_default();

        // 统计邮件数量
        let (email_count, unread_count) = if !imap_name.is_empty() {
            let total = storage::EmailRepository::count_by_folder(&db, account_id, &imap_name)
                .await
                .map_err(|e| format!("统计邮件数量失败: {}", e))?;
            let unread = storage::EmailRepository::count_unread_by_folder(&db, account_id, &imap_name)
                .await
                .map_err(|e| format!("统计未读邮件数量失败: {}", e))?;
            (total, unread)
        } else {
            (0, 0)
        };

        folders.push(StandardFolderDto {
            folder_type: folder_type.to_string(),
            imap_name,
            display_name: display_name.to_string(),
            email_count,
            unread_count,
        });
    }

    Ok(folders)
}
