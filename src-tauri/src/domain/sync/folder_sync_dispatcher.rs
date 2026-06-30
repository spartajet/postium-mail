use crate::domain::auth::AuthManager;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::sync::folder_sync_full::sync_folder_full;
use crate::domain::sync::folder_sync_increment::sync_folder_incremental;
use crate::domain::sync::{
    InitialSyncRange, SyncMode, SyncProgress, SyncProgressEmitter, SyncResult, SyncStage,
    SyncWindow,
};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::{account_repo, sync_repo};
use crate::service::account_connection::imap_config_from_account;
use std::collections::HashSet;
use std::sync::Arc;

/// 同步编排器 — 只负责同步流程编排，不负责进度通知
pub struct SyncOrchestrator {
    db: DbConn,
    auth: Arc<AuthManager>,
    emitter: Option<SyncProgressEmitter>,
}

impl SyncOrchestrator {
    pub fn new(db: DbConn, auth: Arc<AuthManager>) -> Self {
        Self {
            db,
            auth,
            emitter: None,
        }
    }

    pub fn with_emitter(mut self, emitter: SyncProgressEmitter) -> Self {
        self.emitter = Some(emitter);
        self
    }

    /// 同步账号的所有文件夹
    pub async fn sync_account(&self, account_id: i32) -> Result<SyncResult, MailError> {
        self.sync_account_with_initial_window(account_id, None)
            .await
    }

    pub async fn sync_account_with_range(
        &self,
        account_id: i32,
        range: InitialSyncRange,
    ) -> Result<SyncResult, MailError> {
        let now = chrono::Utc::now().timestamp();
        let initial_window = crate::domain::sync::history::window_for_initial_range(range, now);
        self.sync_account_with_initial_window(account_id, Some(initial_window))
            .await
    }

    async fn sync_account_with_initial_window(
        &self,
        account_id: i32,
        initial_window: Option<SyncWindow>,
    ) -> Result<SyncResult, MailError> {
        tracing::info!(account_id, "开始同步账号");
        let start = std::time::Instant::now();

        // 1. 获取账号
        let account = account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))?;

        // 2. 获取 provider 和凭证
        let provider_id = &account.provider;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".to_string(),
            ))?
            .clone();
        let provider = provider_pool
            .get(provider_id)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
        let mail_auth_type = &provider.provider_info().auth_type;

        // let auth_type = account.auth_type.as_deref().unwrap_or("password");
        // let oauth_provider = account.oauth_provider.as_deref();
        let credentials = self
            .auth
            .get_credentials(&account.email, mail_auth_type, Some(provider_id))
            .await?;

        let imap_config = imap_config_from_account(&account)?;

        // 3. 连接 IMAP
        let mut client = match &credentials {
            crate::domain::auth::manager::Credentials::Password(pwd) => {
                ImapClient::connect(&imap_config, &account.email, pwd).await?
            }
            crate::domain::auth::manager::Credentials::OAuth2 { access_token } => {
                // let xoauth2_string = provider.generate_xoauth2(&account.email, access_token);
                ImapClient::connect_xoauth2(&imap_config, &account.email, access_token).await?
            }
        };
        tracing::debug!(account_id, "IMAP 连接成功");

        // 4. 列出远程文件夹
        let remote_folders = client.list_folders().await?;
        let remote_folder_names = remote_folders
            .into_iter()
            .map(|folder| folder.name)
            .collect::<Vec<_>>();
        let folder_mapping = provider.folder_mapping();
        let sync_folders = resolve_sync_folders(&folder_mapping, &remote_folder_names);
        tracing::info!(
            account_id,
            count = sync_folders.len(),
            "需要同步 {} 个文件夹: {:?}",
            sync_folders.len(),
            sync_folders
        );

        // 5. 同步每个文件夹
        let total_folders = sync_folders.len();
        if let Some(ref emitter) = self.emitter {
            emitter.emit(SyncProgress {
                account_id,
                stage: SyncStage::SyncingFolders,
                folder: None,
                current: 0,
                total: total_folders,
                message: format!("发现 {} 个文件夹需要同步", total_folders),
            });
        }

        let mut sync_results = Vec::new();

        for (idx, folder) in sync_folders.iter().enumerate() {
            if let Some(ref emitter) = self.emitter {
                emitter.emit(SyncProgress {
                    account_id,
                    stage: SyncStage::SyncingEmails,
                    folder: Some(folder.clone()),
                    current: idx + 1,
                    total: total_folders,
                    message: format!("正在同步文件夹 {} ({}/{})", folder, idx + 1, total_folders),
                });
            }
            tracing::debug!(account_id, folder = %folder, "开始同步文件夹");
            let sync_mode = self
                .determine_sync_mode(account_id, folder, &mut client)
                .await?;
            tracing::debug!(account_id, folder = %folder, "同步模式: {:?}", sync_mode);
            let sync_result = match sync_mode {
                SyncMode::Full {
                    uidvalidity,
                    uidnext,
                } => {
                    let window = initial_window.unwrap_or_else(|| {
                        crate::domain::sync::history::window_for_initial_range(
                            InitialSyncRange::ThreeMonths,
                            chrono::Utc::now().timestamp(),
                        )
                    });
                    sync_folder_full(
                        self.db.clone(),
                        account_id,
                        folder,
                        uidvalidity as u32,
                        uidnext as u32,
                        &mut client,
                        window,
                    )
                    .await?
                }
                SyncMode::Incremental { last_sync_uid } => {
                    sync_folder_incremental(
                        self.db.clone(),
                        account_id,
                        folder,
                        last_sync_uid,
                        &mut client,
                    )
                    .await?
                }
            };
            tracing::info!(account_id, folder = %folder, "同步完成");
            sync_results.push(sync_result);
        }

        // 6. 更新最后同步时间
        account_repo::update_last_sync(&self.db, account_id).await?;

        // 7. 登出
        client.logout().await.ok();

        let duration_ms = start.elapsed().as_millis() as u64;

        let mut total_sync_result = sync_results.into_iter().fold(
            SyncResult {
                new_emails: 0,
                updated_emails: 0,
                deleted_emails: 0,
                duration_ms: 0,
            },
            |acc, r| SyncResult {
                new_emails: acc.new_emails + r.new_emails,
                updated_emails: acc.updated_emails + r.updated_emails,
                deleted_emails: acc.deleted_emails + r.deleted_emails,
                duration_ms: 0,
            },
        );
        total_sync_result.duration_ms = duration_ms;

        Ok(total_sync_result)
    }

    pub async fn determine_sync_mode(
        &self,
        account_id: i32,
        folder: &str,
        imap_client: &mut ImapClient,
    ) -> Result<SyncMode, MailError> {
        tracing::debug!("确定同步模式: account_id={}, folder={}", account_id, folder);

        // 1. 获取文件夹 IMAP 元数据（uidvalidity, uidnext）
        let metadata = imap_client.fetch_folder_metadata(folder).await?;
        tracing::info!("获取文件夹元数据成功: {:?}", metadata);
        let server_uidvalidity = metadata.uidvalidity;
        let server_last_uid = metadata.recent;
        tracing::debug!(
            "服务器 UIDVALIDITY: {}, LAST_UID: {}",
            server_uidvalidity,
            server_last_uid
        );

        if let Some(local_state) = sync_repo::get_sync_state(&self.db, account_id, folder).await?
            && let Some(local_uidvalidity) = local_state.uidvalidity
        {
            let local_uidvalidity = local_uidvalidity as u64;
            if local_uidvalidity == server_uidvalidity {
                tracing::warn!(
                    "UIDVALIDITY 保持不变: account_id={}, folder={}, local={}, server={}",
                    account_id,
                    folder,
                    local_uidvalidity,
                    server_uidvalidity
                );
                return Ok(SyncMode::Incremental {
                    last_sync_uid: local_state.last_sync_uid.unwrap_or(0),
                });
            } else {
                tracing::warn!(
                    "UIDVALIDITY 发生变化: account_id={}, folder={}, local={}, server={}",
                    account_id,
                    folder,
                    local_uidvalidity,
                    server_uidvalidity
                );
                return Ok(SyncMode::Full {
                    uidvalidity: server_uidvalidity,
                    uidnext: metadata.uidnext,
                });
            }
        }

        Ok(SyncMode::Full {
            uidvalidity: server_uidvalidity,
            uidnext: metadata.uidnext,
        })
    }
}

fn resolve_sync_folders(
    mapping: &crate::domain::providers::StandardFolder,
    remote_folders: &[String],
) -> Vec<String> {
    let remote_folders = remote_folders.iter().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();

    for candidates in [
        &mapping.inbox,
        &mapping.sent,
        &mapping.drafts,
        &mapping.spam,
        &mapping.trash,
        &mapping.archive,
    ] {
        if candidates.is_empty() {
            continue;
        }

        let folder = candidates
            .iter()
            .find(|candidate| remote_folders.contains(candidate))
            .unwrap_or(&candidates[0])
            .clone();

        if seen.insert(folder.clone()) {
            resolved.push(folder);
        }
    }

    resolved
}

#[cfg(test)]
mod tests {
    use super::resolve_sync_folders;
    use crate::domain::providers::StandardFolder;

    #[test]
    fn sync_folders_should_select_one_existing_alias_per_standard_folder() {
        let mapping = StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["[Gmail]/Sent Mail".into(), "Sent".into()],
            drafts: vec!["[Gmail]/Drafts".into()],
            spam: vec!["[Gmail]/Spam".into()],
            trash: vec!["[Gmail]/Trash".into()],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        };
        let remote_folders = [
            "INBOX".to_string(),
            "Sent".to_string(),
            "[Gmail]/Drafts".to_string(),
            "[Gmail]/Spam".to_string(),
            "[Gmail]/Trash".to_string(),
            "Archive".to_string(),
        ];

        let folders = resolve_sync_folders(&mapping, &remote_folders);

        assert_eq!(
            folders,
            vec![
                "INBOX".to_string(),
                "Sent".to_string(),
                "[Gmail]/Drafts".to_string(),
                "[Gmail]/Spam".to_string(),
                "[Gmail]/Trash".to_string(),
                "Archive".to_string(),
            ]
        );
    }

    #[test]
    fn sync_folders_should_not_sync_both_gmail_archive_aliases() {
        let mapping = StandardFolder {
            inbox: vec![],
            sent: vec![],
            drafts: vec![],
            spam: vec![],
            trash: vec![],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        };
        let remote_folders = ["[Gmail]/All Mail".to_string(), "Archive".to_string()];

        let folders = resolve_sync_folders(&mapping, &remote_folders);

        assert_eq!(folders, vec!["[Gmail]/All Mail".to_string()]);
    }

    #[test]
    fn sync_folders_should_fallback_to_first_alias_when_no_remote_candidate_exists() {
        let mapping = StandardFolder {
            inbox: vec![],
            sent: vec![],
            drafts: vec![],
            spam: vec![],
            trash: vec![],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        };
        let remote_folders = ["Unrelated".to_string()];

        let folders = resolve_sync_folders(&mapping, &remote_folders);

        assert_eq!(folders, vec!["[Gmail]/All Mail".to_string()]);
    }
}
