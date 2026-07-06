use crate::domain::auth::Credentials;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::providers::{SmtpServerConfig, SslMode};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use crate::service::account_connection::imap_config_from_account;
use crate::service::email_service::SendEmailRequest;
use async_trait::async_trait;
use lettre::Message;
use lettre::message::header::{ContentType, Date, MessageId};
use lettre::message::{Mailbox, MultiPart, SinglePart};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ComposeAttachmentInput {
    pub path: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct LocalAttachmentDraft {
    pub path: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SendEmailResponse {
    pub message_id: String,
    pub local_email_id: i32,
    pub remote_archived: bool,
    pub remote_archive_error: Option<String>,
}

pub struct BuiltEmail {
    pub message_id: String,
    pub raw: Vec<u8>,
    pub message: Message,
}

pub struct SentArchiveRequest {
    pub account: accounts::Model,
    pub folder: String,
    pub credentials: Credentials,
    pub message_id: String,
    pub raw: Vec<u8>,
}

#[async_trait]
pub trait SmtpEmailSender: Send + Sync {
    async fn send(
        &self,
        config: &SmtpServerConfig,
        account_email: &str,
        credentials: &Credentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError>;
}

pub struct RealSmtpEmailSender;

#[async_trait]
impl SmtpEmailSender for RealSmtpEmailSender {
    async fn send(
        &self,
        config: &SmtpServerConfig,
        account_email: &str,
        credentials: &Credentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError> {
        crate::infrastructure::protocols::smtp::SmtpClient::send_built_email(
            config,
            account_email,
            credentials,
            email.message.clone(),
        )
        .await
    }
}

#[async_trait]
pub trait SentArchiveWriter: Send + Sync {
    async fn append_to_sent(&self, req: SentArchiveRequest) -> Result<(), MailError>;
}

pub struct RealSentArchiveWriter;

#[async_trait]
impl SentArchiveWriter for RealSentArchiveWriter {
    async fn append_to_sent(&self, req: SentArchiveRequest) -> Result<(), MailError> {
        let imap_config = imap_config_from_account(&req.account)?;
        let mut client = match &req.credentials {
            Credentials::Password(password) => {
                ImapClient::connect(&imap_config, &req.account.email, password).await?
            }
            Credentials::OAuth2 { access_token } => {
                ImapClient::connect_xoauth2(&imap_config, &req.account.email, access_token).await?
            }
        };

        let result = client.append_email(&req.folder, &req.raw).await;
        let logout_result = client.logout().await;
        if let Err(err) = logout_result {
            tracing::warn!(
                account_id = req.account.id,
                message_id = %req.message_id,
                error = %err,
                "远端 Sent 归档后 IMAP 登出失败"
            );
        }
        result
    }
}

pub fn non_empty_recipients(req: &SendEmailRequest) -> Vec<String> {
    req.to
        .iter()
        .chain(req.cc.iter())
        .chain(req.bcc.iter())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn validate_send_request(req: &SendEmailRequest) -> Result<(), MailError> {
    if non_empty_recipients(req).is_empty() {
        return Err(MailError::InvalidParam(
            "至少需要一个收件人、抄送或密送地址".to_string(),
        ));
    }
    if req.subject.trim().is_empty() {
        return Err(MailError::InvalidParam("邮件主题不能为空".to_string()));
    }
    if req.body_text.trim().is_empty() {
        return Err(MailError::InvalidParam("邮件正文不能为空".to_string()));
    }
    Ok(())
}

pub async fn describe_local_attachment(path: String) -> Result<LocalAttachmentDraft, MailError> {
    describe_local_attachment_sync(&path)
}

pub fn describe_local_attachment_sync(path: &str) -> Result<LocalAttachmentDraft, MailError> {
    let metadata = std::fs::metadata(path)
        .map_err(|e| MailError::InvalidParam(format!("附件文件不存在或不可访问: {path}: {e}")))?;
    if !metadata.is_file() {
        return Err(MailError::InvalidParam(format!(
            "附件路径不是普通文件: {path}"
        )));
    }

    let filename = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| MailError::InvalidParam(format!("附件文件名无效: {path}")))?
        .to_string();
    let content_type = guess_content_type(&filename);

    Ok(LocalAttachmentDraft {
        path: path.to_string(),
        filename,
        content_type,
        size: i64::try_from(metadata.len())
            .map_err(|_| MailError::InvalidParam("附件大小超出支持范围".to_string()))?,
    })
}

pub fn guess_content_type(filename: &str) -> String {
    match Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("txt") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("pdf") => "application/pdf",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub fn smtp_config_from_account(account: &accounts::Model) -> Result<SmtpServerConfig, MailError> {
    if let Some(host) = account
        .smtp_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let Some(port) = account.smtp_port else {
            return Err(MailError::InvalidParam("SMTP 端口不能为空".to_string()));
        };
        let port = u16::try_from(port)
            .map_err(|_| MailError::InvalidParam(format!("SMTP 端口无效: {port}")))?;

        return Ok(SmtpServerConfig {
            host: host.to_string(),
            port,
            ssl: parse_smtp_ssl_mode(account.smtp_ssl_mode.as_deref().unwrap_or("Tls"))?,
        });
    }

    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
    let provider = provider_pool
        .get(&account.provider)
        .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;

    Ok(provider.smtp_config(&account.email))
}

fn parse_smtp_ssl_mode(value: &str) -> Result<SslMode, MailError> {
    match value.trim() {
        "Tls" | "TLS" | "Implicit" => Ok(SslMode::Implicit),
        "StartTls" | "STARTTLS" => Ok(SslMode::StartTls),
        "None" => Ok(SslMode::None),
        other => Err(MailError::InvalidParam(format!(
            "SMTP 加密模式无效: {other}"
        ))),
    }
}

pub fn build_email(
    account: &accounts::Model,
    req: &SendEmailRequest,
) -> Result<BuiltEmail, MailError> {
    validate_send_request(req)?;

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

    for addr in &req.to {
        if !addr.trim().is_empty() {
            builder = builder.to(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("收件人无效 '{}': {e}", addr.trim()))
            })?);
        }
    }
    for addr in &req.cc {
        if !addr.trim().is_empty() {
            builder = builder.cc(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("抄送无效 '{}': {e}", addr.trim()))
            })?);
        }
    }
    for addr in &req.bcc {
        if !addr.trim().is_empty() {
            builder = builder.bcc(addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("密送无效 '{}': {e}", addr.trim()))
            })?);
        }
    }

    let multipart = MultiPart::alternative()
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
    let message = builder
        .multipart(multipart)
        .map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {e}")))?;
    let raw = message.formatted();

    Ok(BuiltEmail {
        message_id,
        raw,
        message,
    })
}
