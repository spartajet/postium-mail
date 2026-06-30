use crate::{
    domain::sync::SyncResult,
    domain::sync::SyncWindow,
    error::MailError,
    infrastructure::{
        protocols::{imap::ImapClient, utils::unix_seconds_to_imap_date},
        storage::{
            DbConn,
            repository::{email_repo, sync_repo},
        },
    },
};

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
    db: DbConn,
    account_id: i32,
    folder: &str,
    uidvalidity: u32,
    uidnext: u32,
    imap_client: &mut ImapClient,
    window: SyncWindow,
) -> Result<SyncResult, MailError> {
    tracing::info!(
        "全量同步文件夹: account_id={}, folder={}",
        account_id,
        folder,
    );

    // 1. 按同步窗口获取邮件 UID
    let uids = match (window.start, window.end) {
        (Some(start), None) => {
            let date_since = unix_seconds_to_imap_date(start)?;
            tracing::info!(
                "全量同步，获取时间窗口内邮件: folder={}, date_since={}",
                folder,
                date_since
            );
            imap_client.list_uids_since(folder, &date_since).await?
        }
        (None, None) => {
            tracing::info!("全量同步，获取全部邮件 UID: folder={}", folder);
            imap_client.list_all_uids(folder).await?
        }
        _ => {
            return Err(MailError::InvalidParam(
                "全量同步仅支持起始时间窗口或全量窗口".into(),
            ));
        }
    };
    let server_uids_len = uids.len();
    if server_uids_len == 0 {
        let last_sync_uid = last_sync_uid_for_full_sync(&uids, uidnext);
        sync_repo::upsert_sync_state(
            &db,
            account_id,
            folder,
            Some(uidnext),
            Some(uidvalidity),
            Some(last_sync_uid),
        )
        .await?;
        match window.start {
            Some(start) => {
                sync_repo::update_history_state(&db, account_id, folder, Some(start), false).await?
            }
            None => sync_repo::update_history_state(&db, account_id, folder, None, true).await?,
        }
        return Ok(SyncResult {
            new_emails: 0,
            updated_emails: 0,
            deleted_emails: 0,
            duration_ms: 0,
        });
    }

    tracing::info!(
        "全量同步准备完成: folder={}, 需同步邮件数={}",
        folder,
        uids.len()
    );

    // 2. 删除该账号、该folder的所有邮件
    let deleted_count = delete_account_folder_emails(db.clone(), account_id, folder).await?;
    tracing::info!(
        "全量同步，删除 {} 条原有邮件: folder={}",
        deleted_count,
        folder
    );

    tracing::info!(
        "开始同步文件夹: account_id={}, folder={}, server_uids len={}",
        account_id,
        folder,
        server_uids_len
    );

    // 3. 批量获取文件头

    for group in uids.chunks(10) {
        let email_headers = imap_client
            .batch_fetch_email_headers(folder, group[0], group[group.len() - 1])
            .await?;
        // mail_header_list.extend(email_headers);
        tracing::debug!("已获取邮件头: {}", email_headers.len());
        email_repo::save_batch_email_headers(&db, account_id, folder, &email_headers).await?;
        tracing::debug!("保存邮件头: {}", email_headers.len())
    }

    let max_uid = last_sync_uid_for_full_sync(&uids, uidnext);

    tracing::info!(
        "同步完成: account_id={}, folder={}, new_emails={}, last_sync_uid={}",
        account_id,
        folder,
        server_uids_len,
        max_uid
    );

    // 4. 更新或者新增 同步记录
    //
    sync_repo::upsert_sync_state(
        &db,
        account_id,
        folder,
        Some(uidnext),
        Some(uidvalidity),
        Some(max_uid),
    )
    .await?;

    match window.start {
        Some(start) => {
            sync_repo::update_history_state(&db, account_id, folder, Some(start), false).await?
        }
        None => sync_repo::update_history_state(&db, account_id, folder, None, true).await?,
    }

    // let folder_string = folder.to_string();
    // let imap_client_clone = imap_client.clone();
    // tauri::async_runtime::spawn(async move {
    fetch_emails_body(db, account_id, folder, &uids, imap_client).await?;
    // });

    // 6. 返回同步结果
    Ok(SyncResult {
        new_emails: server_uids_len,
        deleted_emails: 0,
        duration_ms: 0,
        updated_emails: server_uids_len,
    })
}

fn last_sync_uid_for_full_sync(uids: &[u32], uidnext: u32) -> u32 {
    uids.last()
        .copied()
        .unwrap_or_else(|| uidnext.saturating_sub(1))
}

pub(crate) async fn fetch_emails_body(
    db: DbConn,
    account_id: i32,
    folder: &str,
    uids: &[u32],
    imap_client: &mut ImapClient,
) -> Result<(), MailError> {
    tracing::debug!(
        "开始获取邮件正文: account_id={}, folder={}, uids len={}",
        account_id,
        folder,
        uids.len()
    );
    imap_client.select_folder(folder).await?;
    for uid in uids {
        if let Ok((body_text, body_html)) = imap_client.fetch_body(*uid).await {
            email_repo::update_body(&db, account_id, folder, *uid, body_text, body_html).await?;
            tracing::debug!("已更新邮件正文: account_id={}, uid={}", account_id, uid);
        }
    }

    Ok(())
}

///
/// # 返回
///
/// 返回删除的邮件数量
pub async fn delete_account_folder_emails(
    db: DbConn,
    account_id: i32,
    folder: &str,
) -> Result<usize, MailError> {
    let deleted_count = email_repo::delete_folder_contents(&db, account_id, folder).await? as usize;

    tracing::info!(
        "已删除账号文件夹的所有邮件: account_id={}, folder={}, count={}",
        account_id,
        folder,
        deleted_count
    );

    Ok(deleted_count)
}

#[cfg(test)]
mod tests {
    use super::last_sync_uid_for_full_sync;

    #[test]
    fn last_sync_uid_for_full_sync_should_use_uidnext_high_watermark_when_window_is_empty() {
        assert_eq!(last_sync_uid_for_full_sync(&[], 501), 500);
    }
}
