//! 增量同步引擎
//!
//! 在 UIDVALIDITY 未变化时使用。
//! 基于 last_sync_uid 获取新增邮件进行增量同步。

use itertools::Itertools;
use sea_orm::DbConn;

use crate::MailError;
use crate::error::{Result, StorageError};
use crate::protocols::imap::{AsyncImapClient, EmailStatusUid};
use crate::storage::service::email::{
    EmailStatus, batch_delete_by_ids, batch_update_email_status, list_status_by_folder,
    save_batch_email_headers,
};
use crate::storage::service::folder_aync_state::get_sync_state;
use crate::sync::SyncResult;
use crate::sync::strcuts::SyncStrategy;

pub async fn sync_folder_increamental(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    last_sync_uid: i32,
    imap_client: &mut AsyncImapClient,
) -> Result<SyncResult> {
    // 1. 获取本地 last_sync_uid
    let local_last_sync_uid = get_sync_state(db, account_id, folder)
        .await?
        .ok_or(MailError::Storage(StorageError::NotFound(
            "last_sync_uid_model".to_string(),
        )))?
        .last_sync_uid
        .ok_or(MailError::Storage(StorageError::NotFound(
            "last_sync_uid".to_string(),
        )))? as u32;

    tracing::info!(
        "增量同步记录，获取本地 last UID > {} 的邮件: folder={}",
        local_last_sync_uid,
        folder
    );
    let (new_emails, server_last_uid) =
        sync_new_emails(db, account_id, imap_client, folder, local_last_sync_uid).await?;
    tracing::info!("同步新邮件完成: new_emails={}", new_emails);
    let (update_count, delete_count) =
        sync_existing_email_flag(db, account_id, imap_client, folder, local_last_sync_uid).await?;
    let sync_result = SyncResult {
        strategy_used: SyncStrategy::UidSearch,
        new_emails,
        modified_emails: update_count,
        deleted_emails: delete_count,
        flags_changed: update_count,
        duration_ms: 0,
        last_sync_uid: server_last_uid,
    };

    Ok(sync_result)
}

async fn sync_new_emails(
    db: &DbConn,
    account_id: i32,
    imap_client: &mut AsyncImapClient,
    folder: &str,
    local_last_sync_uid: u32,
) -> Result<(usize, u32)> {
    // 1. 获取服务器 last_uid
    let server_last_uid = imap_client.get_last_uid(folder).await?;
    tracing::debug!(
        "服务器 last_uid: {}, 本地 last_uid: {}",
        server_last_uid,
        local_last_sync_uid
    );

    // 3. 拉取新的邮件
    if local_last_sync_uid < server_last_uid {
        let new_email_uids: Vec<u32> = ((local_last_sync_uid + 1)..=server_last_uid).collect();
        let new_email_count = new_email_uids.len();
        let mut mail_header_list = Vec::with_capacity(new_email_count);

        for group in new_email_uids.chunks(10) {
            let email_headers = imap_client
                .batch_fetch_email_headers(folder, group[0], group[group.len() - 1])
                .await?;
            mail_header_list.extend(email_headers);
            tracing::debug!("已获取邮件头: {}", mail_header_list.len());
        }

        // 4. 保存邮件头
        if !mail_header_list.is_empty() {
            tracing::debug!("开始保存邮件头...");
            save_batch_email_headers(db, account_id, folder, &mail_header_list).await?;
            tracing::debug!("已保存邮件头: {}", mail_header_list.len());
        }
        return Ok((new_email_count, server_last_uid));
    }

    Ok((0, server_last_uid))
}

async fn sync_existing_email_flag(
    db: &DbConn,
    account_id: i32,
    imap_client: &mut AsyncImapClient,
    folder: &str,
    local_last_sync_uid: u32,
) -> Result<(usize, usize)> {
    // 1. 获取本地已同步的邮件状态
    let local_statuses = list_status_by_folder(db, account_id, folder).await?;
    let first_uid = local_statuses
        .first()
        .map(|s| s.uid.unwrap_or(0) as u32)
        .unwrap_or(0);
    let last_uid = local_statuses
        .last()
        .map(|s| s.uid.unwrap_or(0) as u32)
        .unwrap_or(0);
    // 2. 确定同步范围

    let (start_uid, end_uid) = if first_uid > last_uid {
        (last_uid, first_uid)
    } else {
        (first_uid, last_uid)
    };

    let mut update_count = 0;
    let mut delete_count = 0;

    // 3. 获取服务器端既有标记信息
    let server_statuses = imap_client
        .batch_fetch_flags(folder, start_uid, end_uid)
        .await?;

    // 4.更新并比较标记
    let (update_items, deleted_items, download_items) =
        compare_status(&local_statuses, &server_statuses).await?;
    // 5.更新本地数据库
    if !update_items.is_empty() {
        update_count = batch_update_email_status(db, &update_items).await?;
        tracing::debug!("批量更新邮件状态完成: updated_count={}", update_count);
    }
    // 6. 删除本地已删除的邮件
    if !deleted_items.is_empty() {
        delete_count = batch_delete_by_ids(db, &deleted_items).await?;
        tracing::debug!("批量删除邮件完成: deleted_count={}", delete_count);
    }

    // 7. 下载新的条目（理论上应该是不会的）

    Ok((update_count, delete_count))
}

async fn compare_status(
    local: &[EmailStatus],
    server: &[EmailStatusUid],
) -> Result<(Vec<EmailStatus>, Vec<i32>, Vec<i32>)> {
    let mut update_items = Vec::new();
    let mut deleted_items = Vec::new();
    let mut download_items = Vec::new();

    server
        .iter()
        .merge_join_by(local.iter(), |server_status, local_status| {
            server_status.uid.cmp(&server_status.uid)
        })
        .for_each(|r| match r {
            itertools::EitherOrBoth::Both(server_s, local_s) => {
                if server_s.is_read != local_s.is_read
                    || server_s.is_starred != local_s.is_starred
                    || server_s.is_draft != local_s.is_draft
                    || server_s.is_answered != local_s.is_answered
                    || server_s.is_deleted != local_s.is_deleted
                {
                    update_items.push(EmailStatus {
                        id: local_s.id,
                        uid: local_s.uid,
                        is_read: server_s.is_read,
                        is_starred: server_s.is_starred,
                        is_draft: server_s.is_draft,
                        is_answered: server_s.is_answered,
                        is_deleted: server_s.is_deleted,
                    })
                }
            }
            itertools::EitherOrBoth::Left(server_s) => {
                download_items.push(server_s.uid);
            }
            itertools::EitherOrBoth::Right(local_s) => {
                deleted_items.push(local_s.id);
            }
        });

    Ok((update_items, deleted_items, download_items))
}
