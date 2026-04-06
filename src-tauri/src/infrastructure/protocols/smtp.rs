use crate::domain::providers::SmtpServerConfig;
use crate::domain::providers::SslMode;
use crate::error::MailError;
use lettre::message::header::ContentType;
use lettre::message::MultiPart;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// SMTP 客户端 — 封装 lettre，支持 TLS / StartTLS / Plain / XOAUTH2
pub struct SmtpClient;

impl SmtpClient {
    /// 发送邮件（密码认证）
    #[allow(clippy::too_many_arguments)]
    pub async fn send_email(
        config: &SmtpServerConfig,
        email: &str,
        password: &str,
        from: &str,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<String, MailError> {
        let credentials = Credentials::new(email.to_string(), password.to_string());
        Self::send_email_inner(
            config,
            credentials,
            None,
            from,
            to,
            cc,
            bcc,
            subject,
            body_html,
            body_text,
        )
        .await
    }

    /// 发送邮件（XOAUTH2 认证 — access_token 作为 secret）
    #[allow(clippy::too_many_arguments)]
    pub async fn send_email_xoauth2(
        config: &SmtpServerConfig,
        email: &str,
        access_token: &str,
        from: &str,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<String, MailError> {
        let credentials = Credentials::new(email.to_string(), access_token.to_string());
        Self::send_email_inner(
            config,
            credentials,
            Some(vec![Mechanism::Xoauth2]),
            from,
            to,
            cc,
            bcc,
            subject,
            body_html,
            body_text,
        )
        .await
    }

    /// 共用邮件构建与发送逻辑
    #[allow(clippy::too_many_arguments)]
    async fn send_email_inner(
        config: &SmtpServerConfig,
        credentials: Credentials,
        mechanisms: Option<Vec<Mechanism>>,
        from: &str,
        to: &[String],
        cc: &[String],
        bcc: &[String],
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<String, MailError> {
        tracing::info!(
            host = %config.host,
            port = config.port,
            ssl_mode = ?config.ssl,
            from,
            to_count = to.len(),
            cc_count = cc.len(),
            bcc_count = bcc.len(),
            subject,
            "SMTP: 准备发送邮件"
        );
        // 构建邮件
        let mut builder = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| MailError::InvalidParam(format!("发件人地址无效: {e}")))?,
            )
            .subject(subject);

        for addr in to {
            builder = builder.to(addr
                .parse()
                .map_err(|e| MailError::InvalidParam(format!("收件人无效 '{addr}': {e}")))?);
        }
        for addr in cc {
            builder = builder.cc(addr
                .parse()
                .map_err(|e| MailError::InvalidParam(format!("抄送无效 '{addr}': {e}")))?);
        }
        for addr in bcc {
            builder = builder.bcc(
                addr.parse()
                    .map_err(|e| MailError::InvalidParam(format!("密送无效 '{addr}': {e}")))?,
            );
        }

        let multipart = MultiPart::alternative()
            .singlepart(
                lettre::message::SinglePart::builder()
                    .header(ContentType::TEXT_PLAIN)
                    .body(body_text.to_string()),
            )
            .singlepart(
                lettre::message::SinglePart::builder()
                    .header(ContentType::TEXT_HTML)
                    .body(body_html.to_string()),
            );

        let message = builder
            .multipart(multipart)
            .map_err(|e| MailError::SmtpSendFailed(format!("构建邮件失败: {e}")))?;

        // 构建异步 SMTP transport
        let transport: AsyncSmtpTransport<Tokio1Executor> = match config.ssl {
            SslMode::Implicit => {
                let mut b = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                    .map_err(|e| MailError::SmtpSendFailed(format!("TLS 连接构建失败: {e}")))?
                    .port(config.port)
                    .credentials(credentials);
                if let Some(m) = mechanisms {
                    b = b.authentication(m);
                }
                b.build()
            }
            SslMode::StartTls => {
                let mut b = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                    .map_err(|e| MailError::SmtpSendFailed(format!("STARTTLS 连接构建失败: {e}")))?
                    .port(config.port)
                    .credentials(credentials);
                if let Some(m) = mechanisms {
                    b = b.authentication(m);
                }
                b.build()
            }
            SslMode::None => {
                let mut b = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
                    .port(config.port)
                    .credentials(credentials);
                if let Some(m) = mechanisms {
                    b = b.authentication(m);
                }
                b.build()
            }
        };

        tracing::debug!(host = %config.host, "SMTP: 正在发送");
        transport
            .send(message)
            .await
            .map_err(|e| MailError::SmtpSendFailed(format!("发送失败: {e}")))?;

        tracing::info!(host = %config.host, subject, "SMTP: 邮件发送成功");
        Ok("ok".to_string())
    }
}
