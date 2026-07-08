//!
//! # SMTP 客户端 (SMTP Client)
//!
//! 本模块封装 [lettre] 库，提供异步 SMTP 邮件发送能力，是基础设施层向邮件服务器投递邮件的统一入口。
//!
//! ## 认证方式
//!
//! 支持两条认证路径，分别对应两个公开入口：
//!
//! - **密码认证**：[`SmtpClient::send_email`] —— 使用用户名 + 密码（用户名即邮箱地址），
//!   认证机制交由 lettre 自动协商。
//! - **XOAUTH2 认证**：[`SmtpClient::send_email_xoauth2`] —— 使用邮箱地址 + OAuth2 access_token，
//!   并显式指定 `Mechanism::Xoauth2`，适用于 Gmail / Outlook 等支持 OAuth2 的服务商。
//!
//! 两者最终都汇聚到 [`SmtpClient::send_email_inner`] 完成实际的邮件构建与发送。
//!
//! ## 加密模式（[`SslMode`]）
//!
//! 根据 [`SmtpServerConfig::ssl`] 选择 transport 构建方式：
//!
//! - `SslMode::Implicit`：隐式 TLS（端口通常 465），建立时即握手 TLS。
//! - `SslMode::StartTls`：显式 STARTTLS（端口通常 587），先明文连接再升级为 TLS。
//! - `SslMode::None`：不加密（明文，仅用于本地测试，对应 `builder_dangerous`）。
//!

use crate::domain::auth::Credentials as MailCredentials;
use crate::domain::providers::{SmtpServerConfig, SslMode};
use crate::error::MailError;
use crate::service::mail_send::BuiltEmail;
use lettre::message::MultiPart;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// SMTP 客户端 — 封装 lettre，支持 TLS / StartTLS / Plain / XOAUTH2
///
/// 本结构体无状态（unit struct），所有方法均为关联函数；加密方式由 `config` 决定，
/// 认证方式由调用入口（[`Self::send_email`] / [`Self::send_email_xoauth2`]）决定。
pub struct SmtpClient;

impl SmtpClient {
    /// 发送邮件（密码认证）
    ///
    /// 使用 `email` + `password` 构造凭证，认证机制交由 lettre 自动协商
    /// （对应未指定 `Mechanism` 的情况）。
    ///
    /// # 参数
    ///
    /// - `config`: 目标 SMTP 服务器配置（host / port / `SslMode`）。
    /// - `email`: 登录用邮箱地址，同时作为认证用户名。
    /// - `password`: 邮箱登录密码（或服务商专用授权码，如 QQ / 网易）。
    /// - `from`: 发件人地址（RFC 5322，如 `"Name <a@b.com>"`）。
    /// - `to`: 收件人地址列表。
    /// - `cc`: 抄送地址列表（可为空）。
    /// - `bcc`: 密送地址列表（可为空）。
    /// - `subject`: 邮件主题。
    /// - `body_html`: HTML 正文。
    /// - `body_text`: 纯文本正文（与 HTML 组成 `multipart/alternative`）。
    ///
    /// # 返回
    ///
    /// - `Ok("ok")`: 邮件发送成功。
    /// - `Err(MailError)`: 地址解析失败（`InvalidParam`）或发送失败（`SmtpSendFailed`）。
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
    ///
    /// 使用 `email` + `access_token` 构造凭证，并显式指定 `Mechanism::Xoauth2`
    /// 进行 SASL XOAUTH2 认证，适用于支持 OAuth2 的服务商（Gmail / Outlook 等）。
    ///
    /// # 参数
    ///
    /// - `config`: 目标 SMTP 服务器配置（host / port / `SslMode`）。
    /// - `email`: 登录用邮箱地址，同时作为 XOAUTH2 用户名。
    /// - `access_token`: 有效的 OAuth2 访问令牌（作为认证 secret）。
    /// - `from`: 发件人地址（RFC 5322，如 `"Name <a@b.com>"`）。
    /// - `to`: 收件人地址列表。
    /// - `cc`: 抄送地址列表（可为空）。
    /// - `bcc`: 密送地址列表（可为空）。
    /// - `subject`: 邮件主题。
    /// - `body_html`: HTML 正文。
    /// - `body_text`: 纯文本正文（与 HTML 组成 `multipart/alternative`）。
    ///
    /// # 返回
    ///
    /// - `Ok("ok")`: 邮件发送成功。
    /// - `Err(MailError)`: 地址解析失败（`InvalidParam`）或发送失败（`SmtpSendFailed`）。
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

    /// 发送已构建好的邮件。
    ///
    /// 调用方负责完成 `Message` 构建和校验；本方法只负责按账号凭证选择 SMTP
    /// 认证方式、构建 transport，并把邮件投递到服务器。
    pub async fn send_built_email(
        config: &SmtpServerConfig,
        account_email: &str,
        credentials: &MailCredentials,
        email: &BuiltEmail,
    ) -> Result<(), MailError> {
        let (credentials, mechanisms) = match credentials {
            MailCredentials::Password(password) => (
                Credentials::new(account_email.to_string(), password.to_string()),
                None,
            ),
            MailCredentials::OAuth2 { access_token } => (
                Credentials::new(account_email.to_string(), access_token.to_string()),
                Some(vec![Mechanism::Xoauth2]),
            ),
        };
        let transport = Self::build_transport(config, credentials, mechanisms)?;
        tracing::debug!(host = %config.host, "SMTP: 正在发送已构建邮件");
        transport
            .send_raw(&email.envelope, &email.raw)
            .await
            .map_err(|e| MailError::SmtpSendFailed(format!("发送失败: {e}")))?;
        Ok(())
    }

    /// 共用邮件构建与发送逻辑
    ///
    /// 两个公开入口（密码认证 / XOAUTH2）的差异仅在于凭证内容与是否指定认证机制，
    /// 其余「构造 `multipart/alternative` 邮件 + 按 `SslMode` 构建 transport + 发送」的逻辑
    /// 完全一致，因此统一收敛到本方法。
    ///
    /// # 参数
    ///
    /// - `config`: 目标 SMTP 服务器配置（决定 host / port / `SslMode`）。
    /// - `credentials`: 已组装好的 lettre 认证凭证（用户名 + password 或 access_token）。
    /// - `mechanisms`: 显式指定的 SASL 机制；`None` 表示交由 lettre 自动协商，
    ///   `Some([Xoauth2])` 表示强制使用 XOAUTH2。
    /// - `from` / `to` / `cc` / `bcc` / `subject` / `body_html` / `body_text`:
    ///   邮件信封与正文内容，含义同公开入口。
    ///
    /// # 返回
    ///
    /// - `Ok("ok")`: 邮件发送成功。
    /// - `Err(MailError::InvalidParam)`: 发件 / 收件 / 抄送 / 密送地址解析失败。
    /// - `Err(MailError::SmtpSendFailed)`: transport 构建或实际发送失败。
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

        let transport = Self::build_transport(config, credentials, mechanisms)?;

        tracing::debug!(host = %config.host, "SMTP: 正在发送");
        transport
            .send(message)
            .await
            .map_err(|e| MailError::SmtpSendFailed(format!("发送失败: {e}")))?;

        tracing::info!(host = %config.host, subject, "SMTP: 邮件发送成功");
        Ok("ok".to_string())
    }

    fn build_transport(
        config: &SmtpServerConfig,
        credentials: Credentials,
        mechanisms: Option<Vec<Mechanism>>,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, MailError> {
        let transport = match config.ssl {
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

        Ok(transport)
    }
}
