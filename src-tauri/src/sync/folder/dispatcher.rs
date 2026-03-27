use crate::error::Result;
use crate::protocols::AsyncImapClient;
use crate::storage::service::aync_state::get_sync_state;
use crate::sync::strcuts::SyncMode;
use sea_orm::DbConn;

pub async fn determine_sync_mode(
    db: &DbConn,
    account_id: i32,
    folder: &str,
    imap_client: &mut AsyncImapClient,
) -> Result<SyncMode> {
    tracing::debug!("确定同步模式: account_id={}, folder={}", account_id, folder);

    // 1. 获取文件夹 IMAP 元数据（uidvalidity, uidnext）
    let metadata = imap_client
        .fetch_folder_metadata(folder)
        .await
        .map_err(|e| {
            tracing::warn!("获取文件夹元数据失败: {}, 跳过元数据更新", e);
            // 元数据获取失败不应阻断同步流程
            crate::error::MailError::Internal(format!("获取文件夹元数据失败: {}", e))
        })?;
    tracing::info!("获取文件夹元数据成功: {:?}", metadata);
    let server_uidvalidity = metadata.uidvalidity;
    let server_last_uid = metadata.recent;
    tracing::debug!("服务器 UIDVALIDITY: {}", server_uidvalidity);

    if let Some(local_state) = get_sync_state(db, account_id, folder).await?
        && let Some(local_uidvalidity) = local_state.uidvalidity
    {
        let local_uidvalidity = local_uidvalidity as u64;
        tracing::debug!(
            "检查 UIDVALIDITY: account_id={}, folder={}, local={}, server={}",
            account_id,
            folder,
            local_uidvalidity,
            server_uidvalidity
        );
        if local_uidvalidity == server_uidvalidity {
            tracing::warn!(
                "UIDVALIDITY 变化: account_id={}, folder={}, local={}, server={}",
                account_id,
                folder,
                local_uidvalidity,
                server_uidvalidity
            );
            return Ok(SyncMode::Incremental {
                last_sync_uid: local_state.last_sync_uid.unwrap_or(0),
            });
        } else {
            return Ok(SyncMode::Full {
                uidvalidity: server_uidvalidity,
            });
        }
    }

    Ok(SyncMode::Full {
        uidvalidity: server_uidvalidity,
    })
}
