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
use lettre::address::{Address, Envelope};
use lettre::message::header::{ContentType, Date, InReplyTo, MessageId, References};
use lettre::message::{Attachment, Body, Mailbox, MultiPart, SinglePart};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComposeAttachmentInput {
    pub path: String,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalAttachmentDraft {
    pub path: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailResponse {
    pub message_id: String,
    pub local_email_id: i32,
    pub remote_archived: bool,
    pub remote_archive_error: Option<String>,
}

pub struct BuiltEmail {
    pub message_id: String,
    pub raw: Vec<u8>,
    pub envelope: Envelope,
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
            email,
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

        let result = client
            .append_email_with_flags(&req.folder, Some("(\\Seen)"), &req.raw)
            .await;
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

fn read_attachment_bytes(path: &str) -> Result<Vec<u8>, MailError> {
    std::fs::read(path).map_err(|e| MailError::InvalidParam(format!("读取附件失败: {path}: {e}")))
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

fn build_body_part(req: &SendEmailRequest) -> MultiPart {
    MultiPart::alternative()
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
                .body(req.body_text.clone()),
        )
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(req.body_html.clone()),
        )
}

pub(crate) fn sanitize_attachment_filename(input: &str) -> String {
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

fn content_type_header(value: &str) -> Result<ContentType, MailError> {
    value
        .parse::<ContentType>()
        .map_err(|e| MailError::InvalidParam(format!("附件 Content-Type 无效: {value}: {e}")))
}

fn parse_envelope_address(addr: &str, label: &str) -> Result<Address, MailError> {
    addr.trim()
        .parse()
        .map_err(|e| MailError::InvalidParam(format!("{label}无效 '{}': {e}", addr.trim())))
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
    let envelope_from = account
        .email
        .trim()
        .parse::<Address>()
        .map_err(|e| MailError::InvalidParam(format!("发件人地址无效: {e}")))?;
    let mut envelope_recipients = Vec::new();

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

    // 线程头（回信发送）：非空才写头——Message-ID 仍为本函数新生成的 UUID，
    // 不取草稿/原邮件的 Message-ID。与 build_draft_email 同一用法。
    if let Some(value) = req.in_reply_to.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        builder = builder.header(InReplyTo::from(value.to_string()));
    }
    if let Some(value) = req.references.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        builder = builder.header(References::from(value.to_string()));
    }

    for addr in &req.to {
        if !addr.trim().is_empty() {
            let mailbox = addr.trim().parse().map_err(|e| {
                MailError::InvalidParam(format!("收件人无效 '{}': {e}", addr.trim()))
            })?;
            builder = builder.to(mailbox);
            envelope_recipients.push(parse_envelope_address(addr, "收件人")?);
        }
    }
    for addr in &req.cc {
        if !addr.trim().is_empty() {
            let mailbox = addr
                .trim()
                .parse()
                .map_err(|e| MailError::InvalidParam(format!("抄送无效 '{}': {e}", addr.trim())))?;
            builder = builder.cc(mailbox);
            envelope_recipients.push(parse_envelope_address(addr, "抄送")?);
        }
    }
    for addr in &req.bcc {
        if !addr.trim().is_empty() {
            let _mailbox: Mailbox = addr
                .trim()
                .parse()
                .map_err(|e| MailError::InvalidParam(format!("密送无效 '{}': {e}", addr.trim())))?;
            envelope_recipients.push(parse_envelope_address(addr, "密送")?);
        }
    }
    let envelope = Envelope::new(Some(envelope_from), envelope_recipients)
        .map_err(|e| MailError::InvalidParam(format!("构建 SMTP envelope 失败: {e}")))?;

    let body_part = build_body_part(req);
    let message = builder
        .multipart(if req.attachments.is_empty() {
            body_part
        } else {
            let mut mixed = MultiPart::mixed().multipart(body_part);
            for input in &req.attachments {
                let described = describe_local_attachment_sync(&input.path)?;
                let filename = input
                    .filename
                    .as_deref()
                    .map(sanitize_attachment_filename)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(described.filename);
                let content_type = input
                    .content_type
                    .as_deref()
                    .unwrap_or(&described.content_type);
                let bytes = read_attachment_bytes(&input.path)?;
                let attachment = Attachment::new(filename)
                    .body(Body::new(bytes), content_type_header(content_type)?);
                mixed = mixed.singlepart(attachment);
            }
            mixed
        })
        .map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {e}")))?;
    let raw = message.formatted();

    Ok(BuiltEmail {
        message_id,
        raw,
        envelope,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_email, read_attachment_bytes};
    use crate::error::MailError;
    use crate::infrastructure::storage::models::accounts;
    use crate::service::email_service::SendEmailRequest;
    use uuid::Uuid;

    #[test]
    fn read_attachment_bytes_reports_read_failure_for_missing_file() {
        let path = std::env::temp_dir().join(format!(
            "postium-mail-missing-attachment-{}",
            Uuid::new_v4()
        ));
        let path = path.to_string_lossy().into_owned();

        let err = read_attachment_bytes(&path).err().unwrap();

        assert!(
            matches!(err, MailError::InvalidParam(message) if message.contains("读取附件失败"))
        );
    }

    fn test_account() -> accounts::Model {
        accounts::Model {
            id: 1,
            name: "测试账号".to_string(),
            email: "user@example.com".to_string(),
            display_name: Some("用户".to_string()),
            provider: "custom".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl: None,
            smtp_ssl_mode: None,
            color: None,
            sync_enabled: None,
            last_sync_at: None,
            auth_type: None,
            account_type: "personal".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn send_req() -> SendEmailRequest {
        SendEmailRequest {
            account_id: 1,
            to: vec!["dest@example.com".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Re: 主题".to_string(),
            body_html: "<p>hi</p>".to_string(),
            body_text: "hi".to_string(),
            in_reply_to: None,
            references: None,
            attachments: vec![],
            draft_id: None,
        }
    }

    #[test]
    fn build_email_includes_threading_headers_when_present() {
        let req = SendEmailRequest {
            in_reply_to: Some("<orig@x.com>".to_string()),
            references: Some("<root@x.com>,<orig@x.com>".to_string()),
            ..send_req()
        };
        let built = build_email(&test_account(), &req).expect("构建回信应成功");
        let raw = String::from_utf8_lossy(&built.raw);
        assert!(
            raw.contains("In-Reply-To: <orig@x.com>"),
            "raw 应含 In-Reply-To 头: {raw}"
        );
        assert!(
            raw.contains("References: <root@x.com>,<orig@x.com>"),
            "raw 应含 References 头: {raw}"
        );
        // 发送链路 Message-ID 语义不变：新 UUID（不取草稿 Message-ID）
        assert!(
            built.message_id.starts_with('<')
                && built.message_id.ends_with('>')
                && built.message_id.contains('@')
                && built.message_id != "<orig@x.com>",
            "Message-ID 仍为新 UUID 形态: {}",
            built.message_id
        );
    }

    #[test]
    fn build_email_omits_threading_headers_when_absent() {
        let built = build_email(&test_account(), &send_req()).expect("构建普通发送应成功");
        let raw = String::from_utf8_lossy(&built.raw);
        assert!(!raw.contains("In-Reply-To:"), "无值不应带 In-Reply-To 头");
        assert!(!raw.contains("References:"), "无值不应带 References 头");
    }

    #[test]
    fn build_email_blank_threading_values_omit_headers() {
        let req = SendEmailRequest {
            in_reply_to: Some("   ".to_string()),
            references: Some("".to_string()),
            ..send_req()
        };
        let built = build_email(&test_account(), &req).expect("构建发送应成功");
        let raw = String::from_utf8_lossy(&built.raw);
        assert!(
            !raw.contains("In-Reply-To:"),
            "空白值不应带 In-Reply-To 头: {raw}"
        );
        assert!(
            !raw.contains("References:"),
            "空白值不应带 References 头: {raw}"
        );
    }
}
