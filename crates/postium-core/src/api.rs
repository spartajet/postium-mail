//! CoreApi：后端能力的进程内门面。
//!
//! 原 Tauri 架构中前端通过 `invoke` 调用 command 层（跨 IPC 序列化）；
//! GPUI 架构中 UI 直接持有 `Arc<CoreApi>` 进程内调用本门面。
//! 方法与原 command 一一对应（同名，去掉 State/AppHandle 注入参数），
//! 函数体为对 service 层的直调转发。

use std::{path::Path, sync::Arc};

use crate::domain::{
    auth::AuthManager,
    sync::FolderStat,
    providers::{ProviderInfo, detect::{ProviderDetectionResult, detect_provider}, pool::ProviderPool},
    sync::{InitialSyncRange, SyncEventTx, SyncProgressEvent},
};
use crate::error::MailError;
use crate::infrastructure::{
    auth::oauth2::{OAuth2AuthUrl, OAuth2Manager, OAuth2PollResult},
    storage::database::{self, DbConn},
    storage::repository::contact_repo::{ContactDto, ContactGroupWithCount, ContactListResponse, ContactWrite},
};
use crate::service::{
    account_service::{AccountDto, AccountService, CreateAccountRequest, UpdateAccountRequest},
    attachment_service::{AttachmentDto, AttachmentService, InlineAttachmentDto},
    contact_service::ContactService,
    email_service::{
        EmailCategory,
        EmailDetail,
        EmailListResponse,
        EmailService,
        ReloadEmailResult,
        SaveDraftRequest,
        SaveDraftResponse,
        SendEmailRequest,
        SendEmailResponse,
    },
    label_service::{CreateLabelRequest, LabelDto, LabelService, UpdateLabelRequest},
    sync_service::{HistorySyncState, OlderSyncResult, SyncAllAccountsResult, SyncService},
};

/// 同步进度事件的 UI 订阅端
pub type SyncEventRx = smol::channel::Receiver<SyncProgressEvent>;

/// 后端门面：聚合全部服务，UI 唯一入口。
pub struct CoreApi {
    pub db: DbConn,
    pub auth: Arc<AuthManager>,
    pub provider_pool: Arc<ProviderPool>,
    pub accounts: AccountService,
    pub emails: EmailService,
    pub sync: SyncService,
    pub labels: LabelService,
    pub contacts: ContactService,
    pub attachments: AttachmentService,
    pub oauth2: Arc<OAuth2Manager>,
    pub data_dir: std::path::PathBuf,
    /// e2e 离线模式（e2e=true 且非真连库）：同步入口短路，保证测试确定性。
    e2e_offline: bool,
}

impl CoreApi {
    /// 完整装配：初始化数据库、provider 池、认证与服务层。
    ///
    /// 返回 `(CoreApi, 事件接收端)`；接收端交给 UI 的同步状态模块消费，
    /// 其克隆出的发送端用于手动同步与后台调度器。
    pub async fn init(data_dir: &Path, e2e: bool, e2e_truth: bool) -> Result<(Arc<Self>, SyncEventRx), MailError> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| MailError::FileSystemError(e.to_string()))?;
        tracing::info!("数据目录: {}", data_dir.display());

        let db = database::init_database(data_dir).await?;

        if e2e && !e2e_truth {
            crate::infrastructure::testing::e2e_seed::seed_e2e_data(&db).await?;
        }

        crate::domain::providers::pool::init_provider_pool();
        let provider_pool = crate::domain::providers::pool::PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("provider pool 未初始化".into()))?
            .clone();

        let auth = Arc::new(AuthManager::default());
        // e2e（非真连库）模式跳过 IMAP 真连验证，允许离线创建测试账号
        let accounts = if e2e && !e2e_truth {
            AccountService::new_with_imap_verifier(
                db.clone(),
                auth.clone(),
                Arc::new(crate::service::account_connection::NoopImapConnectionVerifier),
            )
        } else {
            AccountService::new(db.clone(), auth.clone())
        };
        let emails = EmailService::new_for_runtime(auth.clone(), db.clone(), e2e, e2e_truth);
        let sync = SyncService::new(db.clone(), auth.clone());
        let labels = LabelService::new(db.clone());
        let contacts = ContactService::new(db.clone());
        let attachments = AttachmentService::new(db.clone(), auth.clone(), data_dir.to_path_buf());
        let oauth2 = Arc::new(OAuth2Manager::new(auth.clone(), db.clone())?);
        auth.set_oauth2_manager(oauth2.clone());

        let (_tx, rx) = smol::channel::bounded::<SyncProgressEvent>(256);

        Ok((
            Arc::new(Self {
                db,
                auth,
                provider_pool,
                accounts,
                emails,
                sync,
                labels,
                contacts,
                attachments,
                oauth2,
                data_dir: data_dir.to_path_buf(),
                e2e_offline: e2e && !e2e_truth,
            }),
            rx,
        ))
    }

    /// 创建同步进度事件通道（UI 消费 rx，把 tx 传给 sync_*/scheduler）。
    pub fn sync_event_channel() -> (SyncEventTx, SyncEventRx) {
        smol::channel::bounded(256)
    }

    // ===== account =====

    pub async fn list_accounts(&self) -> Result<Vec<AccountDto>, MailError> {
        self.accounts.list().await
    }

    pub async fn get_account(&self, id: i32) -> Result<AccountDto, MailError> {
        self.accounts.get(id).await
    }

    pub async fn create_account(&self, req: CreateAccountRequest) -> Result<AccountDto, MailError> {
        self.accounts.create(req).await
    }

    pub async fn update_account(&self, req: UpdateAccountRequest) -> Result<AccountDto, MailError> {
        self.accounts.update(req).await
    }

    pub async fn delete_account(&self, id: i32) -> Result<(), MailError> {
        self.accounts.delete(id).await
    }

    pub async fn update_account_password(&self, id: i32, password: String) -> Result<(), MailError> {
        self.accounts.update_password(id, password).await
    }

    // ===== email =====

    pub async fn list_emails(
        &self,
        account_id: i32,
        folder: &str,
        page: usize,
        limit: usize,
    ) -> Result<EmailListResponse, MailError> {
        self.emails.list(account_id, folder, page, limit).await
    }

    pub async fn list_emails_by_category(
        &self,
        account_id: i32,
        category: EmailCategory,
        page: usize,
        limit: usize,
        unread_only: bool,
    ) -> Result<EmailListResponse, MailError> {
        self.emails
            .list_by_category(account_id, category, page, limit, unread_only)
            .await
    }

    pub async fn list_emails_by_category_for_all_accounts(
        &self,
        category: EmailCategory,
        page: usize,
        limit: usize,
        unread_only: bool,
    ) -> Result<EmailListResponse, MailError> {
        self.emails
            .list_by_category_for_all_accounts(category, page, limit, unread_only)
            .await
    }

    pub async fn get_email(&self, id: i32) -> Result<EmailDetail, MailError> {
        self.emails.get(id).await
    }

    pub async fn reload_email(&self, email_id: i32) -> Result<ReloadEmailResult, MailError> {
        self.emails.reload_email(email_id).await
    }

    pub async fn search_emails(
        &self,
        query: &str,
        account_id: Option<i32>,
        limit: Option<u64>,
    ) -> Result<Vec<crate::infrastructure::storage::search::SearchResult>, MailError> {
        self.emails.search(query, account_id, limit).await
    }

    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), MailError> {
        self.emails.mark_as_read(id, is_read).await
    }

    pub async fn toggle_star(&self, id: i32) -> Result<bool, MailError> {
        self.emails.toggle_star(id).await
    }

    pub async fn delete_emails(&self, ids: Vec<i32>) -> Result<usize, MailError> {
        self.emails.delete(ids).await
    }

    pub async fn move_email_to_folder(&self, id: i32, folder: &str) -> Result<(), MailError> {
        self.emails.move_to_folder(id, folder).await
    }

    pub async fn archive_email(&self, id: i32) -> Result<(), MailError> {
        self.emails.archive(id).await
    }

    pub async fn describe_local_attachments(
        &self,
        paths: Vec<String>,
    ) -> Result<Vec<crate::service::mail_send::LocalAttachmentDraft>, MailError> {
        self.emails.describe_local_attachments(paths).await
    }

    pub async fn save_draft(&self, req: SaveDraftRequest) -> Result<SaveDraftResponse, MailError> {
        self.emails.save_draft(req).await
    }

    pub async fn delete_draft(&self, draft_id: i32) -> Result<(), MailError> {
        self.emails.delete_draft(draft_id).await
    }

    pub async fn parse_email_addresses(
        &self,
        input: String,
    ) -> Result<crate::service::email_service::ParseEmailAddressesResponse, MailError> {
        self.emails.parse_email_addresses(input).await
    }

    pub async fn send_email(&self, req: SendEmailRequest) -> Result<SendEmailResponse, MailError> {
        self.emails.send(req).await
    }

    // ===== 附件 =====

    pub async fn ensure_attachment_cached(&self, attachment_id: i32) -> Result<AttachmentDto, MailError> {
        self.attachments.ensure_cached(attachment_id).await
    }

    pub async fn save_attachment_as(&self, attachment_id: i32, target_path: String) -> Result<(), MailError> {
        self.attachments.save_as(attachment_id, target_path).await
    }

    pub async fn open_attachment(&self, attachment_id: i32) -> Result<(), MailError> {
        self.attachments.open(attachment_id).await
    }

    pub async fn resolve_inline_attachments(&self, email_id: i32) -> Result<Vec<InlineAttachmentDto>, MailError> {
        self.attachments.resolve_inline_images(email_id).await
    }

    // ===== sync =====

    pub async fn sync_account(
        &self,
        event_tx: SyncEventTx,
        account_id: i32,
    ) -> Result<(), MailError> {
        if self.e2e_offline {
            return Ok(());
        }
        self.sync.sync_account_with_progress(event_tx, account_id).await
    }

    pub async fn sync_account_with_range(
        &self,
        event_tx: SyncEventTx,
        account_id: i32,
        range: InitialSyncRange,
    ) -> Result<(), MailError> {
        if self.e2e_offline {
            return Ok(());
        }
        self.sync.sync_account_with_range(event_tx, account_id, range).await
    }

    /// e2e 离线模式（测试确定性）：UI 据此跳过后台同步调度。
    pub fn is_e2e_offline(&self) -> bool {
        self.e2e_offline
    }

    pub async fn sync_all_accounts(
        &self,
        event_tx: SyncEventTx,
    ) -> Result<SyncAllAccountsResult, MailError> {
        if self.e2e_offline {
            return Ok(SyncAllAccountsResult { total: 0, succeeded: Vec::new(), failed: Vec::new() });
        }
        self.sync.sync_all_accounts_with_progress(event_tx).await
    }

    pub async fn get_sync_history_state(
        &self,
        account_id: i32,
        category: EmailCategory,
    ) -> Result<HistorySyncState, MailError> {
        self.sync.get_history_state(account_id, category).await
    }

    pub async fn sync_older_emails(
        &self,
        account_id: i32,
        category: EmailCategory,
    ) -> Result<OlderSyncResult, MailError> {
        if self.e2e_offline {
            return Ok(OlderSyncResult { new_emails: 0, updated_emails: 0, window_start: 0, window_end: 0, history_exhausted: true, folders: Vec::new() });
        }
        self.sync.sync_older_emails(account_id, category).await
    }

    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>, MailError> {
        self.sync.get_folder_stats(account_id).await
    }

    pub async fn get_folder_stats_for_all_accounts(&self) -> Result<Vec<FolderStat>, MailError> {
        self.sync.get_folder_stats_for_all_accounts().await
    }

    // ===== auth =====

    pub fn detect_provider(&self, email: &str) -> ProviderDetectionResult {
        detect_provider(email, &self.provider_pool)
    }

    pub fn list_providers(&self) -> Result<Vec<ProviderInfo>, MailError> {
        Ok(self.provider_pool.list_providers())
    }

    pub async fn start_oauth2(
        &self,
        provider_id: String,
        email: String,
        display_name: Option<String>,
    ) -> Result<OAuth2AuthUrl, MailError> {
        self.oauth2
            .start_auth(&provider_id, &email, display_name.as_deref())
            .await
    }

    pub async fn poll_oauth2(&self, state: &str) -> Result<OAuth2PollResult, MailError> {
        self.oauth2.poll_oauth2(state).await
    }

    // ===== label =====

    pub async fn list_labels(&self, account_id: i32) -> Result<Vec<LabelDto>, MailError> {
        self.labels.list_labels(account_id).await
    }

    pub async fn create_label(&self, req: CreateLabelRequest) -> Result<LabelDto, MailError> {
        self.labels.create_label(req).await
    }

    pub async fn update_label(&self, id: i32, req: UpdateLabelRequest) -> Result<LabelDto, MailError> {
        self.labels.update_label(id, req).await
    }

    pub async fn delete_label(&self, id: i32) -> Result<(), MailError> {
        self.labels.delete_label(id).await
    }

    pub async fn add_label_to_email(&self, email_id: i32, label_id: i32) -> Result<(), MailError> {
        self.labels.add_label_to_email(email_id, label_id).await
    }

    pub async fn remove_label_from_email(&self, email_id: i32, label_id: i32) -> Result<(), MailError> {
        self.labels.remove_label_from_email(email_id, label_id).await
    }

    pub async fn get_labels_for_email(&self, email_id: i32) -> Result<Vec<LabelDto>, MailError> {
        self.labels.get_labels_for_email(email_id).await
    }

    pub async fn list_emails_by_label(&self, label_id: i32) -> Result<Vec<i32>, MailError> {
        self.labels.list_emails_by_label(label_id).await
    }

    // ===== contact =====

    pub async fn backfill_contacts(&self) -> Result<usize, MailError> {
        self.contacts.backfill().await
    }

    pub async fn list_contacts(
        &self,
        query: Option<String>,
        group_id: Option<i32>,
        starred_only: bool,
        page: usize,
        limit: usize,
    ) -> Result<ContactListResponse, MailError> {
        self.contacts
            .list(query, group_id, starred_only, page, limit)
            .await
    }

    pub async fn get_contact(&self, id: i32) -> Result<Option<ContactDto>, MailError> {
        self.contacts.get(id).await
    }

    /// 按邮箱查询联系人（大小写不敏感）；邮件详情页「查看/添加联系人」联动入口。
    pub async fn get_contact_by_email(&self, email: String) -> Result<Option<ContactDto>, MailError> {
        self.contacts.get_by_email(&email).await
    }

    pub async fn create_contact(&self, w: ContactWrite) -> Result<ContactDto, MailError> {
        self.contacts.create(w).await
    }

    pub async fn update_contact(&self, id: i32, w: ContactWrite) -> Result<(), MailError> {
        self.contacts.update(id, w).await
    }

    pub async fn delete_contact(&self, id: i32) -> Result<(), MailError> {
        self.contacts.delete(id).await
    }

    pub async fn set_contact_starred(&self, id: i32, starred: bool) -> Result<(), MailError> {
        self.contacts.set_starred(id, starred).await
    }

    pub async fn merge_contacts(&self, primary_id: i32, absorbed: Vec<i32>) -> Result<ContactDto, MailError> {
        self.contacts.merge(primary_id, &absorbed).await
    }

    pub async fn list_contact_groups(&self) -> Result<Vec<ContactGroupWithCount>, MailError> {
        self.contacts.group_list().await
    }

    pub async fn create_contact_group(&self, name: String) -> Result<i32, MailError> {
        self.contacts.group_insert(&name).await
    }

    pub async fn rename_contact_group(&self, id: i32, name: String) -> Result<(), MailError> {
        self.contacts.group_rename(id, &name).await
    }

    pub async fn delete_contact_group(&self, id: i32) -> Result<(), MailError> {
        self.contacts.group_delete(id).await
    }

    pub async fn set_contact_group_members(&self, contact_id: i32, group_ids: Vec<i32>) -> Result<(), MailError> {
        self.contacts.group_set_members(contact_id, &group_ids).await
    }
}
