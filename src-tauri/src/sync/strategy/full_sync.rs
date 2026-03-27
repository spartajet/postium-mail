//! 全量同步引擎
//!
//! 当 UIDVALIDITY 变化或首次同步时使用。
//! 获取指定时间范围内的所有邮件进行完整同步。

use crate::error::MailError;
use crate::protocols::imap::AsyncImapClient;
use crate::storage::service::email::{delete_account_folder_emails, save_batch_email_headers};
use crate::sync::structs::{SyncResult, SyncStrategy};

use crate::error::Result;
use sea_orm::DbConn;

/// 内部方法：同步单个文件夹
///
/// 全量同步文件夹
///
/// 当 UIDVALIDITY 变化或首次同步时调用。获取所有邮件进行完整同步。
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `imap_client` - IMAP 客户端引用
/// * `metadata` - 文件夹元数据（已在调用方获取）
///
/// # 返回
///
/// 返回同步结果
pub async fn sync_folder_full(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    imap_client: &mut AsyncImapClient,
) -> Result<SyncResult> {
    tracing::info!(
        "全量同步文件夹: account_id={}, folder={}",
        account_id,
        folder,
    );

    // 1. 获取近三个月的邮件 UID
    tracing::info!("全量同步，获取三个月内的邮件: folder={}", folder);
    let date_since = crate::protocols::imap::three_months_ago_imap_format();
    tracing::info!(
        "全量同步，获取三个月内的邮件: folder={}, date_since={}",
        folder,
        date_since
    );

    let mut server_uids = imap_client
        .list_uids_since(folder, &date_since)
        .await
        .map_err(|e| MailError::Internal(format!("获取服务器 UID 列表失败: {}", e)))?;
    server_uids.sort();

    tracing::info!(
        "全量同步准备完成: folder={}, 需同步邮件数={}",
        folder,
        server_uids.len()
    );

    // 2. 删除该账号、该folder的所有邮件
    let deleted_count = delete_account_folder_emails(db, account_id, folder).await?;
    tracing::info!("全量同步，删除 {} 条邮件: folder={}", deleted_count, folder);

    // 2. 调用 sync_folder 进行实际同步
    let result = sync_folder(db, account_id, folder, &server_uids, imap_client).await?;

    Ok(result)
}

/// 同步单个文件夹
///
/// # 参数
///
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `server_uids` - 服务器 UID 列表（可选）
/// * `server_uids_with_flags` - 服务器 UID 和标志列表（可选）
/// * `imap_client` - IMAP 客户端引用（可选，用于获取邮件内容）
#[allow(clippy::too_many_arguments)]
pub async fn sync_folder(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uids: &[u32],
    imap_client: &mut AsyncImapClient,
) -> Result<SyncResult> {
    let uid_len = uids.len();
    tracing::info!(
        "开始同步文件夹: account_id={}, folder={}, server_uids len={}",
        account_id,
        folder,
        uid_len
    );

    // 1. 获取文件头
    let mut mail_header_list = Vec::with_capacity(uid_len);

    for group in uids.chunks(10) {
        let email_headers = imap_client
            .batch_fetch_email_headers(folder, group[0], group[group.len() - 1])
            .await?;
        mail_header_list.extend(email_headers);
        tracing::debug!("已获取邮件头: {}", mail_header_list.len());
    }

    // 2. 保存邮件头
    if !mail_header_list.is_empty() {
        tracing::debug!("开始保存邮件头...");
        save_batch_email_headers(db, account_id, folder, &mail_header_list).await?;
        tracing::debug!("已保存邮件头: {}", mail_header_list.len());
    }

    // 5. 更新 last_sync_uid（使用服务器 UID 中的最大值）

    let max_uid = if uid_len > 0 {
        uids.iter().max().unwrap_or(&uids[uid_len - 1])
    } else {
        &0
    };

    tracing::info!(
        "同步完成: account_id={}, folder={}, new_emails={}, last_sync_uid={}",
        account_id,
        folder,
        uid_len,
        max_uid
    );

    // 6. 返回同步结果
    Ok(SyncResult {
        strategy_used: SyncStrategy::UidSearch,
        new_emails: uid_len,
        modified_emails: uid_len,
        deleted_emails: 0,
        flags_changed: 0,
        duration_ms: 0,
        last_sync_uid: *max_uid,
    })
}
