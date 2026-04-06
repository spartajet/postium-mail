use crate::domain::{auth::AuthManager, providers::pool::PROVIDER_POOL};
use crate::error::MailError;
use crate::infrastructure::storage::repository::{account_repo, attachment_repo, email_repo};
use crate::infrastructure::storage::{DbConn, search};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

// ─── DTO ───

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDto {
    pub id: i32,
    pub account_id: i32,
    pub folder: String,
    pub uid: u32,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub preview: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub sent_at: i64,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailDetail {
    #[serde(flatten)]
    pub email: EmailDto,
    pub recipient_emails: String,
    pub cc_emails: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EmailListResponse {
    pub emails: Vec<EmailDto>,
    pub total: u64,
    pub page: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SendEmailRequest {
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
}

// ─── Service ───

pub struct EmailService {
    auth: Arc<AuthManager>,
    db: DbConn,
}

impl EmailService {
    pub fn new(auth: Arc<AuthManager>, db: DbConn) -> Self {
        Self { auth, db }
    }

    pub async fn list(
        &self,
        account_id: i32,
        folder: &str,
        page: usize,
        limit: usize,
    ) -> Result<EmailListResponse, MailError> {
        let (emails, total) =
            email_repo::list_by_folder(&self.db, account_id, folder, page, limit).await?;
        Ok(EmailListResponse {
            emails: emails
                .into_iter()
                .map(|e| EmailDto {
                    id: e.id,
                    account_id: e.account_id,
                    folder: e.folder,
                    uid: e.uid,
                    subject: e.subject,
                    sender_name: e.sender_name,
                    sender_email: e.sender_email,
                    preview: e.preview,
                    is_read: e.is_read.unwrap_or(false),
                    is_starred: e.is_starred.unwrap_or(false),
                    sent_at: e.sent_at,
                    has_attachments: false,
                })
                .collect(),
            total,
            page,
            limit,
        })
    }

    pub async fn get(&self, id: i32) -> Result<EmailDetail, MailError> {
        let email = email_repo::get_by_id(&self.db, id)
            .await?
            .ok_or(MailError::EmailNotFound(id))?;
        Ok(EmailDetail {
            email: EmailDto {
                id: email.id,
                account_id: email.account_id,
                folder: email.folder,
                uid: email.uid,
                subject: email.subject,
                sender_name: email.sender_name,
                sender_email: email.sender_email,
                preview: email.preview,
                is_read: email.is_read.unwrap_or(false),
                is_starred: email.is_starred.unwrap_or(false),
                sent_at: email.sent_at,
                has_attachments: false,
            },
            recipient_emails: email.recipient_emails,
            cc_emails: email.cc_emails,
            body_text: email.body_text,
            body_html: email.body_html,
        })
    }

    pub async fn search(
        &self,
        query: &str,
        account_id: Option<i32>,
        limit: Option<u64>,
    ) -> Result<Vec<search::SearchResult>, MailError> {
        search::search_fts(&self.db, query, account_id, limit.unwrap_or(50)).await
    }

    pub async fn mark_as_read(&self, id: i32, is_read: bool) -> Result<(), MailError> {
        email_repo::mark_as_read(&self.db, id, is_read).await
    }

    pub async fn toggle_star(&self, id: i32) -> Result<bool, MailError> {
        email_repo::toggle_star(&self.db, id).await
    }

    pub async fn delete(&self, ids: Vec<i32>) -> Result<usize, MailError> {
        email_repo::soft_delete(&self.db, ids).await
    }

    pub async fn move_to_folder(&self, id: i32, folder: &str) -> Result<(), MailError> {
        email_repo::move_to_folder(&self.db, id, folder).await
    }

    pub async fn send(&self, req: SendEmailRequest) -> Result<String, MailError> {
        tracing::info!(account_id = req.account_id, to = req.to.len(), "发送邮件");
        let account = account_repo::get_by_id(&self.db, req.account_id)
            .await?
            .ok_or(MailError::AccountNotFound(req.account_id))?;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or(MailError::ProviderNotSupported(
                "未找到provider pool".to_string(),
            ))?
            .clone();
        let provider = provider_pool
            .get(&account.provider)
            .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;

        let smtp_config = provider.smtp_config(&account.email);
        let from = account
            .display_name
            .map(|n| format!("{} <{}>", n, account.email))
            .unwrap_or_else(|| account.email.clone());
        let mail_auth_type = &provider.as_ref().provider_info().auth_type;

        // let auth_type = account.auth_type.as_deref().unwrap_or("password");
        // let oauth_provider = account.oauth_provider.as_deref();
        let credentials = self
            .auth
            .get_credentials(&account.email, mail_auth_type, Some(&account.provider))
            .await?;

        let result = match &credentials {
            crate::domain::auth::Credentials::Password(pwd) => {
                crate::infrastructure::protocols::smtp::SmtpClient::send_email(
                    &smtp_config,
                    &account.email,
                    pwd,
                    &from,
                    &req.to,
                    &req.cc,
                    &req.bcc,
                    &req.subject,
                    &req.body_html,
                    &req.body_text,
                )
                .await
            }
            crate::domain::auth::Credentials::OAuth2 { access_token } => {
                crate::infrastructure::protocols::smtp::SmtpClient::send_email_xoauth2(
                    &smtp_config,
                    &account.email,
                    access_token,
                    &from,
                    &req.to,
                    &req.cc,
                    &req.bcc,
                    &req.subject,
                    &req.body_html,
                    &req.body_text,
                )
                .await
            }
        };

        tracing::info!(account_id = req.account_id, "邮件发送成功");
        result
    }

    ///
    /// # 返回
    ///
    /// 返回删除的邮件数量
    pub async fn delete_account_folder_emails(
        &self,
        account_id: i32,
        folder: &str,
    ) -> Result<usize, MailError> {
        // 1. 先获取该账号、该文件夹的所有邮件 ID
        let email_ids = email_repo::get_ids_by_folder(&self.db, account_id, folder).await?;

        // 2. 删除这些邮件的附件
        for email_id in &email_ids {
            attachment_repo::delete_by_email(&self.db, *email_id).await?;
        }

        // 3. 删除所有邮件
        let deleted_count =
            email_repo::delete_by_folder(&self.db, account_id, folder).await? as usize;

        tracing::info!(
            "已删除账号文件夹的所有邮件: account_id={}, folder={}, count={}",
            account_id,
            folder,
            deleted_count
        );

        Ok(deleted_count)
    }
}
