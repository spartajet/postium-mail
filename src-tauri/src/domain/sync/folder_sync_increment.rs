use crate::{
    domain::sync::{HISTORY_UID_BATCH_SIZE, SyncResult},
    error::MailError,
    infrastructure::{
        protocols::imap::ImapClient,
        storage::{
            DbConn,
            repository::{email_repo::save_batch_emails, sync_repo},
        },
    },
};

/// 增量同步单个文件夹
///
/// 基于上次同步的最高 UID（`last_sync_uid`），只获取新增的邮件。
/// 相比全量同步显著减少网络传输量，适用于定期自动同步场景。
///
/// # 参数
///
/// * `db` - 数据库连接
/// * `account_id` - 账号 ID
/// * `folder` - 文件夹名称
/// * `last_sync_uid` - 上次同步的最高 UID
/// * `imap_client` - IMAP 客户端引用
///
/// # 返回
///
/// 返回同步结果统计
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
    let mut inserted = 0usize;
    let mut fetched = 0usize;

    for group in new_uids.chunks(HISTORY_UID_BATCH_SIZE) {
        let new_emails = imap_client
            .batch_fetch_emails_by_uids(folder, group)
            .await?;
        fetched += new_emails.len();
        inserted += save_batch_emails(&db, account_id, folder, &new_emails).await?;
    }

    if !new_uids.is_empty() {
        tracing::info!(
            "增量同步完成，拉取 {} 条邮件，插入或更新 {} 条邮件",
            fetched,
            inserted
        );
    }

    let max_uid = new_uids.last().copied().unwrap_or(last_sync_uid);
    sync_repo::upsert_sync_state(&db, account_id, folder, None, None, Some(max_uid)).await?;

    Ok(SyncResult {
        new_emails: inserted,
        updated_emails: fetched,
        deleted_emails: 0,
        duration_ms: 0,
    })
}
