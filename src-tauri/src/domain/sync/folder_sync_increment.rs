use crate::{
    domain::sync::SyncResult,
    error::MailError,
    infrastructure::{
        protocols::imap::ImapClient,
        storage::{
            DbConn,
            repository::{email_repo::save_batch_emails, sync_repo},
        },
    },
};

pub async fn sync_folder_incremental(
    db: DbConn,
    account_id: i32,
    folder: &str,
    last_sync_uid: u32,
    imap_client: &mut ImapClient,
) -> Result<SyncResult, MailError> {
    tracing::info!(
        account_id = account_id,
        folder = folder,
        last_sync_uid = last_sync_uid,
        "开始增量同步"
    );

    let new_uids = imap_client
        .list_uids_since_uid(folder, last_sync_uid + 1)
        .await?;
    if !new_uids.is_empty() {
        // 批量获取新邮件
        let new_emails = imap_client
            .batch_fetch_emails(
                folder,
                last_sync_uid + 1,
                new_uids.last().copied().unwrap_or(0),
            )
            .await?;

        let insert_result = save_batch_emails(&db, account_id, folder, &new_emails).await?;
        tracing::info!("同步完成，插入 {} 条新邮件", insert_result);
    }
    let max_uid = new_uids.last().copied().unwrap_or(last_sync_uid);
    sync_repo::upsert_sync_state(&db, account_id, folder, None, None, Some(max_uid)).await?;

    Ok(SyncResult {
        new_emails: 0,
        updated_emails: 0,
        deleted_emails: 0,
        duration_ms: 0,
    })
}
