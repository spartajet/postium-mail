use crate::domain::auth::AuthManager;
use crate::domain::auth::manager::Credentials;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::infrastructure::storage::repository::{account_repo, email_repo};
use crate::service::account_connection::imap_config_from_account;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait MailRemoteOperator: Send + Sync {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError>;

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError>;

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError>;
}

pub struct RealMailRemoteOperator {
    auth: Arc<AuthManager>,
}

impl RealMailRemoteOperator {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self { auth }
    }

    async fn connect_for_account(
        &self,
        account: &accounts::Model,
    ) -> Result<ImapClient, MailError> {
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let credentials = self
            .auth
            .get_credentials(
                &account.email,
                &provider.provider_info().auth_type,
                Some(&account.provider),
            )
            .await?;
        let imap_config = imap_config_from_account(account)?;

        match credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &account.email, &password).await
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await
            }
        }
    }
}

#[async_trait]
impl MailRemoteOperator for RealMailRemoteOperator {
    async fn mark_seen(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        seen: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if seen {
            client.add_flags(uid, "\\Seen").await?;
        } else {
            client.remove_flags(uid, "\\Seen").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn set_flagged(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        flagged: bool,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        if flagged {
            client.add_flags(uid, "\\Flagged").await?;
        } else {
            client.remove_flags(uid, "\\Flagged").await?;
        }
        client.logout().await.ok();
        Ok(())
    }

    async fn move_to_folder(
        &self,
        account: &accounts::Model,
        folder: &str,
        uid: u32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let mut client = self.connect_for_account(account).await?;
        client.select_folder(folder).await?;
        client.move_uid_to_folder(uid, target_folder).await?;
        client.logout().await.ok();
        Ok(())
    }
}

pub struct MailOperationService {
    db: DbConn,
    remote: Arc<dyn MailRemoteOperator>,
}

impl MailOperationService {
    pub fn new(db: DbConn, _auth: Arc<AuthManager>, remote: Arc<dyn MailRemoteOperator>) -> Self {
        Self { db, remote }
    }

    async fn get_email(&self, email_id: i32) -> Result<emails::Model, MailError> {
        email_repo::get_by_id(&self.db, email_id)
            .await?
            .ok_or(MailError::EmailNotFound(email_id))
    }

    async fn get_account(&self, account_id: i32) -> Result<accounts::Model, MailError> {
        account_repo::get_by_id(&self.db, account_id)
            .await?
            .ok_or(MailError::AccountNotFound(account_id))
    }

    async fn email_and_account(
        &self,
        email_id: i32,
    ) -> Result<(emails::Model, accounts::Model), MailError> {
        let email = self.get_email(email_id).await?;
        let account = self.get_account(email.account_id).await?;
        Ok((email, account))
    }

    pub async fn mark_as_read(&self, email_id: i32, is_read: bool) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        self.remote
            .mark_seen(&account, &email.folder, email.uid, is_read)
            .await?;
        email_repo::mark_as_read(&self.db, email_id, is_read).await
    }

    pub async fn toggle_star(&self, email_id: i32) -> Result<bool, MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let new_state = !email.is_starred.unwrap_or(false);
        self.remote
            .set_flagged(&account, &email.folder, email.uid, new_state)
            .await?;
        email_repo::toggle_star(&self.db, email_id).await
    }

    pub async fn move_to_folder(
        &self,
        email_id: i32,
        target_folder: &str,
    ) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        if email.folder == target_folder {
            return Ok(());
        }
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, target_folder).await
    }

    pub async fn delete(&self, email_ids: Vec<i32>) -> Result<usize, MailError> {
        if email_ids.len() > 1 {
            return Err(MailError::InvalidParam(
                "第一阶段删除仅支持单封邮件，避免批量远端移动产生部分成功状态".to_string(),
            ));
        }

        let mut pending = Vec::with_capacity(email_ids.len());
        for email_id in email_ids.iter().copied() {
            let (email, account) = self.email_and_account(email_id).await?;
            let target_folder = self.resolve_special_folder(&account, "trash").await?;
            let trash_folders = self.resolve_special_folders(&account, "trash").await?;
            if trash_folders.iter().any(|folder| folder == &email.folder) {
                return Err(MailError::InvalidParam(format!(
                    "邮件 {email_id} 已在 Trash 文件夹，第一阶段不执行永久删除"
                )));
            }
            pending.push((email_id, email, account, target_folder));
        }

        let mut moved = 0;
        for (email_id, email, account, target_folder) in pending {
            self.remote
                .move_to_folder(&account, &email.folder, email.uid, &target_folder)
                .await?;
            email_repo::move_to_folder(&self.db, email_id, &target_folder).await?;
            moved += 1;
        }
        Ok(moved)
    }

    pub async fn archive(&self, email_id: i32) -> Result<(), MailError> {
        let (email, account) = self.email_and_account(email_id).await?;
        let target_folder = self.resolve_special_folder(&account, "archive").await?;
        self.remote
            .move_to_folder(&account, &email.folder, email.uid, &target_folder)
            .await?;
        email_repo::move_to_folder(&self.db, email_id, &target_folder).await
    }

    async fn resolve_special_folder(
        &self,
        account: &accounts::Model,
        kind: &str,
    ) -> Result<String, MailError> {
        self.resolve_special_folders(account, kind)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                MailError::FolderNotFound(format!("账号 {} 未配置 {kind} 文件夹", account.email))
            })
    }

    async fn resolve_special_folders(
        &self,
        account: &accounts::Model,
        kind: &str,
    ) -> Result<Vec<String>, MailError> {
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let mapping = provider.folder_mapping();
        let candidates = match kind {
            "trash" => mapping.trash,
            "archive" => mapping.archive,
            _ => Vec::new(),
        };
        if candidates.is_empty() {
            return Err(MailError::FolderNotFound(format!(
                "账号 {} 未配置 {kind} 文件夹",
                account.email
            )));
        }
        Ok(candidates)
    }
}
