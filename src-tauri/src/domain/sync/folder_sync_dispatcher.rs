use crate::domain::auth::AuthManager;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::sync::folder_sync_full::sync_folder_full;
use crate::domain::sync::folder_sync_increment::sync_folder_incremental;
use crate::domain::sync::{SyncMode, SyncProgress, SyncProgressEmitter, SyncResult, SyncStage};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::database::DbConn;
use crate::infrastructure::storage::repository::{account_repo, sync_repo};
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

        let imap_config = provider.imap_config(&account.email);

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
        // let sync_folders = client.list_folders().await?;
        let folder_mapping = provider.folder_mapping();
        let sync_folders = folder_mapping.list();
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
                SyncMode::Full { uidvalidity } => {
                    sync_folder_full(
                        self.db.clone(),
                        account_id,
                        folder,
                        uidvalidity as u32,
                        &mut client,
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
                });
            }
        }

        Ok(SyncMode::Full {
            uidvalidity: server_uidvalidity,
        })
    }
}

// /// 从 header 和可选 body 构建邮件模型
// fn build_email_model(
//     account_id: i32,
//     folder: &str,
//     uid: u32,
//     header: &crate::infrastructure::protocols::imap::RawEmailHeader,
//     body_bytes: Option<&[u8]>,
// ) -> emails::ActiveModel {
//     let now = chrono::Utc::now().timestamp();
//     let flags = &header.flags;
//     let is_read = !flags
//         .iter()
//         .any(|f| f.contains("Seen") || f.contains("\\Seen"));
//     let is_starred = flags
//         .iter()
//         .any(|f| f.contains("Flagged") || f.contains("\\Flagged"));
//     let is_draft = flags
//         .iter()
//         .any(|f| f.contains("Draft") || f.contains("\\Draft"));
//     let is_answered = flags
//         .iter()
//         .any(|f| f.contains("Answered") || f.contains("\\Answered"));

//     // 尝试解析 body
//     if let Some(bytes) = body_bytes
//         && let Some(msg) = mail_parser::MessageParser::default().parse(bytes)
//     {
//         let sender_name = msg
//             .from()
//             .and_then(|a| a.first().and_then(|p| p.name().map(|n| n.to_string())));
//         let sender_email = msg
//             .from()
//             .and_then(|a| a.first().and_then(|p| p.address().map(|a| a.to_string())))
//             .or_else(|| header.from.clone())
//             .unwrap_or_default();

//         let recipient_emails = msg
//             .to()
//             .map(|addr| {
//                 addr.iter()
//                     .filter_map(|a| a.address().map(|a| a.to_string()))
//                     .collect::<Vec<_>>()
//                     .join(", ")
//             })
//             .or_else(|| header.to.clone())
//             .unwrap_or_default();

//         let body_text = msg.body_text(0).map(|t| t.to_string());
//         let body_html = msg.body_html(0).map(|t| t.to_string());
//         let preview = body_text.as_ref().map(|t| t.chars().take(200).collect());

//         let sent_at = msg
//             .date()
//             .map(|d| d.to_timestamp())
//             .unwrap_or_else(|| parse_date_to_timestamp(header.date.as_deref()).unwrap_or(now));

//         let subject = msg
//             .subject()
//             .map(|s| s.to_string())
//             .or_else(|| header.subject.clone());
//         let message_id = msg
//             .message_id()
//             .map(|s| s.to_string())
//             .or_else(|| header.message_id.clone());

//         return emails::ActiveModel {
//             account_id: Set(account_id),
//             folder: Set(folder.to_string()),
//             uid: Set(uid),
//             message_id: Set(message_id),
//             subject: Set(subject),
//             sender_name: Set(sender_name),
//             sender_email: Set(sender_email),
//             recipient_emails: Set(recipient_emails),
//             cc_emails: Set(None),
//             bcc_emails: Set(None),
//             preview: Set(preview),
//             body_text: Set(body_text),
//             body_html: Set(body_html),
//             is_read: Set(Some(is_read)),
//             is_starred: Set(Some(is_starred)),
//             is_draft: Set(Some(is_draft)),
//             is_answered: Set(Some(is_answered)),
//             is_deleted: Set(Some(false)),
//             sent_at: Set(sent_at),
//             received_at: Set(now),
//             created_at: Set(now),
//             updated_at: Set(now),
//             ..Default::default()
//         };
//     }

//     // 解析失败或无 body — 只存 header
//     emails::ActiveModel {
//         account_id: Set(account_id),
//         folder: Set(folder.to_string()),
//         uid: Set(uid),
//         message_id: Set(header.message_id.clone()),
//         subject: Set(header.subject.clone()),
//         sender_name: Set(None),
//         sender_email: Set(header.from.clone().unwrap_or_default()),
//         recipient_emails: Set(header.to.clone().unwrap_or_default()),
//         cc_emails: Set(None),
//         bcc_emails: Set(None),
//         preview: Set(None),
//         body_text: Set(None),
//         body_html: Set(None),
//         is_read: Set(Some(is_read)),
//         is_starred: Set(Some(is_starred)),
//         is_draft: Set(Some(is_draft)),
//         is_answered: Set(Some(is_answered)),
//         is_deleted: Set(Some(false)),
//         sent_at: Set(parse_date_to_timestamp(header.date.as_deref()).unwrap_or(now)),
//         received_at: Set(now),
//         created_at: Set(now),
//         updated_at: Set(now),
//         ..Default::default()
//     }
// }

// /// 解析 RFC2822 日期字符串为 Unix 时间戳
// fn parse_date_to_timestamp(date_str: Option<&str>) -> Option<i64> {
//     let date_str = date_str?;
//     // 尝试 chrono 的 RFC2822 解析
//     chrono::DateTime::parse_from_rfc2822(date_str)
//         .map(|dt| dt.timestamp())
//         .ok()
// }
