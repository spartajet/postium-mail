use crate::domain::providers::*;

/// Mail.com 邮箱服务商
///
/// Mail.com 提供的邮箱服务，使用密码认证，
/// 支持 mail.com、email.com 域名。
pub struct MailComProvider {
    info: ProviderInfo,
}

impl MailComProvider {
    /// 创建 Mail.com 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "mailcom".into(),
                name: "Mail.com".into(),
                account_type: AccountType::Personal,
                domains: vec!["mail.com".into(), "email.com".into()],
                auth_type: AuthType::Password,
                color: Some("#00478F".into()),
                icon: Some("mailcom".into()),
                sort_order: 14,
            },
        }
    }
}

/// 默认实现，等同于 [`MailComProvider::new`]
impl Default for MailComProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Mail.com 邮箱的 [`MailProvider`] 实现
impl MailProvider for MailComProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.mail.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.mail.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 支持的域名：mail.com、email.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["mail.com", "email.com"]
    }
}
