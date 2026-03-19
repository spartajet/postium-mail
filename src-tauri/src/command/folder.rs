//! 文件夹操作 Commands
//!
//! 提供文件夹的查询功能，包括：
//! - 获取账号的所有文件夹及统计信息
//!
//! # 文件夹统计
//!
//! 每个文件夹包含以下统计信息：
//! - 邮件总数
//! - 未读邮件数
//! - 文件夹属性（是否有子文件夹、是否可选中邮件等）

use super::DatabaseState;
use crate::storage::models;
use crate::storage;

/// 获取账号的所有文件夹及统计信息
///
/// 返回指定账号的所有文件夹，包括每个文件夹的邮件统计。
///
/// # 参数
/// * `state` - 数据库连接状态
/// * `account_id` - 账号 ID
///
/// # 返回
/// 成功时返回文件夹列表（FolderDto 数组），每项包含：
/// - `id`: 文件夹 ID
/// - `name`: 文件夹名称
/// - `display_name`: 显示名称（可能包含本地化）
/// - `account_id`: 所属账号 ID
/// - `email_count`: 邮件总数
/// - `unread_count`: 未读邮件数
/// - `attributes`: 文件夹属性（是否有子文件夹、是否可选中等）
///
/// 失败时返回错误信息字符串
///
/// # 文件夹属性说明
///
/// FolderDto 的 attributes 字段是一个字符串数组，可能包含：
/// - `\HasChildren`: 有子文件夹
/// - `\HasNoChildren`: 无子文件夹
/// - `\Noselect`: 不可选中（通常只用于父文件夹）
/// - `\Marked`: 已标记
/// - `\Unmarked`: 未标记
///
/// # 示例
/// ```rust
/// let folders = get_folder_stats(state, 1).await?;
/// for folder in folders {
///     println!("{}: {} 总邮件, {} 未读",
///              folder.display_name,
///              folder.email_count,
///              folder.unread_count);
/// }
/// ```
#[tauri::command]
pub async fn get_folder_stats(
    state: tauri::State<'_, DatabaseState>,
    account_id: i32,
) -> Result<Vec<models::folder::FolderDto>, String> {
    let db = state.clone_conn();
    let folders = storage::FolderRepository::get_by_account(&db, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(folders.into_iter().map(|f| f.into()).collect())
}
