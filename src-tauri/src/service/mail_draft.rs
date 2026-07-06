use crate::domain::auth::Credentials;
use crate::domain::folders::{FolderCategory, FolderRegistry};
use crate::domain::providers::{AuthType, pool::PROVIDER_POOL};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::{accounts, emails};
use crate::infrastructure::storage::repository::attachment_repo::AttachmentWrite;
use crate::infrastructure::storage::repository::{account_repo, email_repo};
use crate::service::account_connection::imap_config_from_account;
use crate::service::email_service::{EmailService, local_folder_registry_inputs};
use crate::service::mail_send::{ComposeAttachmentInput, describe_local_attachment_sync};
use async_trait::async_trait;
use lettre::Message;
use lettre::message::header::{ContentType, Date, MessageId};
use lettre::message::{Attachment, Body, Mailbox, MultiPart, SinglePart};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SaveDraftRequest {
    pub draft_id: Option<i32>,
    pub account_id: i32,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    #[serde(default)]
    pub attachments: Vec<ComposeAttachmentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SaveDraftResponse {
    pub draft_id: i32,
    pub message_id: String,
    pub folder: String,
    pub saved_at: i64,
    pub remote_saved: bool,
    pub cleanup_error: Option<String>,
}

pub struct DraftAppendRequest {
    pub account: accounts::Model,
    pub folder: String,
    pub credentials: Credentials,
    pub raw: Vec<u8>,
}

#[async_trait]
pub trait DraftRemoteWriter: Send + Sync {
    async fn append_draft(&self, req: DraftAppendRequest) -> Result<(), MailError>;
    async fn delete_draft(
        &self,
        account: &accounts::Model,
        credentials: &Credentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), MailError>;
}

pub struct RealDraftRemoteWriter;

#[async_trait]
impl DraftRemoteWriter for RealDraftRemoteWriter {
    async fn append_draft(&self, req: DraftAppendRequest) -> Result<(), MailError> {
        let imap_config = imap_config_from_account(&req.account)?;
        let mut client = match &req.credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &req.account.email, password).await?
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &req.account.email, access_token).await?
            }
        };
        let result = client
            .append_email_with_flags(&req.folder, Some("(\\Draft \\Seen)"), &req.raw)
            .await;
        client.logout().await.ok();
        result
    }

    async fn delete_draft(
        &self,
        account: &accounts::Model,
        credentials: &Credentials,
        folder: &str,
        uid: u32,
    ) -> Result<(), MailError> {
        let imap_config = imap_config_from_account(account)?;
        let mut client = match credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &account.email, password).await?
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &account.email, access_token).await?
            }
        };
        client.select_folder(folder).await?;
        let result = client.delete_uid(uid).await;
        client.logout().await.ok();
        result
    }
}

pub fn is_blank_draft(req: &SaveDraftRequest) -> bool {
    req.to
        .iter()
        .chain(req.cc.iter())
        .chain(req.bcc.iter())
        .all(|value| value.trim().is_empty())
        && req.subject.trim().is_empty()
        && req.body_text.trim().is_empty()
        && req.body_html.trim().is_empty()
        && req.attachments.is_empty()
}

pub async fn save_draft(
    service: &EmailService,
    req: SaveDraftRequest,
) -> Result<SaveDraftResponse, MailError> {
    if is_blank_draft(&req) {
        return Err(MailError::InvalidParam("空白草稿不需要保存".to_string()));
    }

    let account = account_repo::get_by_id(service.db_conn(), req.account_id)
        .await?
        .ok_or(MailError::AccountNotFound(req.account_id))?;
    let credentials = resolve_credentials(service, &account).await?;
    let folder = resolve_drafts_folder(service, &account).await?;
    let built = build_draft_email(&account, &req)?;
    let now = chrono::Utc::now().timestamp();

    service
        .draft_writer()
        .append_draft(DraftAppendRequest {
            account: account.clone(),
            folder: folder.clone(),
            credentials,
            raw: built.raw,
        })
        .await?;

    let attachments = to_attachment_writes(&req.attachments, now)?;
    let inserted = email_repo::insert_draft_email_with_attachments(
        service.db_conn(),
        email_repo::EmailWrite {
            account_id: account.id,
            folder: folder.clone(),
            uid: 0,
            message_id: Some(built.message_id.clone()),
            subject: Some(req.subject.trim().to_string()),
            sender_name: account.display_name.clone(),
            sender_email: account.email.clone(),
            recipient_emails: join_addresses(&req.to),
            cc_emails: join_optional_addresses(&req.cc),
            bcc_emails: join_optional_addresses(&req.bcc),
            preview: Some(draft_preview(&req)),
            body_text: Some(req.body_text.clone()),
            body_html: Some(req.body_html.clone()),
            is_read: Some(true),
            is_starred: Some(false),
            is_draft: Some(true),
            is_answered: Some(false),
            is_deleted: Some(false),
            sent_at: now,
            received_at: now,
            created_at: now,
            updated_at: now,
        },
        attachments,
    )
    .await?;

    let cleanup_error = if let Some(draft_id) = req.draft_id {
        cleanup_old_draft(service, &account, draft_id).await
    } else {
        None
    };

    Ok(SaveDraftResponse {
        draft_id: inserted.id,
        message_id: built.message_id,
        folder,
        saved_at: now,
        remote_saved: true,
        cleanup_error,
    })
}

pub async fn delete_draft(service: &EmailService, draft_id: i32) -> Result<(), MailError> {
    let draft = email_repo::get_by_id(service.db_conn(), draft_id)
        .await?
        .ok_or(MailError::EmailNotFound(draft_id))?;
    if draft.is_draft != Some(true) {
        return Err(MailError::InvalidParam("指定邮件不是草稿".to_string()));
    }
    let account = account_repo::get_by_id(service.db_conn(), draft.account_id)
        .await?
        .ok_or(MailError::AccountNotFound(draft.account_id))?;
    let credentials = resolve_credentials(service, &account).await?;
    if let Err(err) = service
        .draft_writer()
        .delete_draft(&account, &credentials, &draft.folder, draft.uid)
        .await
    {
        tracing::warn!(
            draft_id,
            account_id = account.id,
            uid = draft.uid,
            error = %err,
            "远端删除草稿失败"
        );
    }
    email_repo::delete_one_with_attachments(service.db_conn(), draft_id).await?;
    Ok(())
}

async fn cleanup_old_draft(
    service: &EmailService,
    account: &accounts::Model,
    draft_id: i32,
) -> Option<String> {
    let old = match email_repo::get_by_id(service.db_conn(), draft_id).await {
        Ok(Some(model)) => model,
        Ok(None) => return None,
        Err(err) => return Some(err.to_string()),
    };

    let credentials = match resolve_credentials(service, account).await {
        Ok(credentials) => credentials,
        Err(err) => return Some(err.to_string()),
    };

    let remote_err = service
        .draft_writer()
        .delete_draft(account, &credentials, &old.folder, old.uid)
        .await
        .err()
        .map(|err| err.to_string());
    let local_err = email_repo::delete_one_with_attachments(service.db_conn(), draft_id)
        .await
        .err()
        .map(|err| err.to_string());

    match (remote_err, local_err) {
        (None, None) => None,
        (Some(remote), None) => Some(format!("旧远端草稿删除失败: {remote}")),
        (None, Some(local)) => Some(format!("旧本地草稿删除失败: {local}")),
        (Some(remote), Some(local)) => Some(format!(
            "旧远端草稿删除失败: {remote}; 旧本地草稿删除失败: {local}"
        )),
    }
}

async fn resolve_drafts_folder(
    service: &EmailService,
    account: &accounts::Model,
) -> Result<String, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported(
            "未找到provider pool".to_string(),
        ))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let (local_names, known_categories) =
        local_folder_registry_inputs(service.db_conn(), account.id).await?;
    let registry = FolderRegistry::builder()
        .remote_folders(local_names)
        .known_categories(known_categories)
        .provider_mapping(provider.folder_mapping())
        .allow_unverified_provider_fallback(true)
        .build();
    Ok(registry
        .resolve_one(FolderCategory::Drafts)
        .unwrap_or_else(|| "Drafts".to_string()))
}

async fn resolve_credentials(
    service: &EmailService,
    account: &accounts::Model,
) -> Result<Credentials, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or(MailError::ProviderNotSupported(
            "未找到provider pool".to_string(),
        ))?
        .clone();
    let provider = provider_pool
        .get(&account.provider)
        .ok_or(MailError::ProviderNotSupported(account.provider.clone()))?;
    let provider_auth_type = &provider.as_ref().provider_info().auth_type;
    let account_auth_type = account
        .auth_type
        .as_deref()
        .and_then(parse_account_auth_type)
        .unwrap_or_else(|| provider_auth_type.clone());
    service
        .auth_manager()
        .get_credentials(&account.email, &account_auth_type, Some(&account.provider))
        .await
}

fn parse_account_auth_type(value: &str) -> Option<AuthType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "password" => Some(AuthType::Password),
        "oauth2" => Some(AuthType::OAuth2),
        _ => None,
    }
}

fn draft_preview(req: &SaveDraftRequest) -> String {
    let preview = if req.body_text.trim().is_empty() {
        req.body_html.trim()
    } else {
        req.body_text.trim()
    };
    preview.chars().take(200).collect()
}

fn join_addresses(values: &[String]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

fn join_optional_addresses(values: &[String]) -> Option<String> {
    let joined = join_addresses(values);
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn to_attachment_writes(
    attachments: &[ComposeAttachmentInput],
    now: i64,
) -> Result<Vec<AttachmentWrite>, MailError> {
    attachments
        .iter()
        .map(|input| {
            let described = describe_local_attachment_sync(&input.path)?;
            Ok(AttachmentWrite {
                email_id: 0,
                filename: Some(
                    input
                        .filename
                        .clone()
                        .unwrap_or(described.filename)
                        .trim()
                        .to_string(),
                ),
                content_type: Some(input.content_type.clone().unwrap_or(described.content_type)),
                size: input.size.unwrap_or(described.size),
                section_path: String::new(),
                disposition: Some("attachment".to_string()),
                content_id: None,
                path: Some(input.path.clone()),
                created_at: now,
            })
        })
        .collect()
}

#[allow(dead_code)]
fn _draft_model(_draft: &emails::Model) {}

struct BuiltDraft {
    message_id: String,
    raw: Vec<u8>,
}

fn build_draft_email(
    account: &accounts::Model,
    req: &SaveDraftRequest,
) -> Result<BuiltDraft, MailError> {
    let domain = account
        .email
        .split('@')
        .next_back()
        .unwrap_or("postium.local");
    let message_id = format!("<{}@{}>", Uuid::new_v4(), domain);
    let from = account
        .display_name
        .as_ref()
        .map(|name| format!("{name} <{}>", account.email))
        .unwrap_or_else(|| account.email.clone());
    let mut builder = Message::builder()
        .from(
            from.parse::<Mailbox>()
                .map_err(|e| MailError::InvalidParam(format!("发件人地址无效: {e}")))?,
        )
        .header(MessageId::from(
            message_id.trim_matches(['<', '>']).to_string(),
        ))
        .header(Date::now())
        .subject(req.subject.trim());

    let mut has_recipients = false;
    for addr in &req.to {
        if !addr.trim().is_empty() {
            builder = builder.to(parse_mailbox(addr, "收件人")?);
            has_recipients = true;
        }
    }
    for addr in &req.cc {
        if !addr.trim().is_empty() {
            builder = builder.cc(parse_mailbox(addr, "抄送")?);
            has_recipients = true;
        }
    }
    for addr in &req.bcc {
        if !addr.trim().is_empty() {
            builder = builder.bcc(parse_mailbox(addr, "密送")?);
            has_recipients = true;
        }
    }
    if !has_recipients {
        builder = builder.bcc(
            "draft-placeholder@postium.invalid"
                .parse::<Mailbox>()
                .map_err(|e| MailError::InvalidParam(format!("构建草稿占位收件人失败: {e}")))?,
        );
    }

    let message = builder
        .multipart(build_draft_body(req)?)
        .map_err(|e| MailError::SmtpSendFailed(format!("构建草稿失败: {e}")))?;
    let raw = message.formatted();

    Ok(BuiltDraft { message_id, raw })
}

fn build_draft_body(req: &SaveDraftRequest) -> Result<MultiPart, MailError> {
    let body_part = MultiPart::alternative()
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(req.body_text.clone()),
        )
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(req.body_html.clone()),
        );

    if req.attachments.is_empty() {
        Ok(body_part)
    } else {
        let mut mixed = MultiPart::mixed().multipart(body_part);
        for input in &req.attachments {
            let described = describe_local_attachment_sync(&input.path)?;
            let filename = sanitize_attachment_filename(
                input
                    .filename
                    .as_deref()
                    .unwrap_or(described.filename.as_str()),
            );
            let content_type = input
                .content_type
                .as_deref()
                .unwrap_or(described.content_type.as_str());
            let bytes = std::fs::read(&input.path).map_err(|e| {
                MailError::InvalidParam(format!("读取附件失败: {}: {e}", input.path))
            })?;
            mixed = mixed.singlepart(
                Attachment::new(filename).body(Body::new(bytes), parse_content_type(content_type)?),
            );
        }
        Ok(mixed)
    }
}

fn parse_mailbox(addr: &str, label: &str) -> Result<Mailbox, MailError> {
    addr.trim()
        .parse()
        .map_err(|e| MailError::InvalidParam(format!("{label}无效 '{}': {e}", addr.trim())))
}

fn parse_content_type(value: &str) -> Result<ContentType, MailError> {
    value
        .parse::<ContentType>()
        .map_err(|e| MailError::InvalidParam(format!("附件 Content-Type 无效: {value}: {e}")))
}

fn sanitize_attachment_filename(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
