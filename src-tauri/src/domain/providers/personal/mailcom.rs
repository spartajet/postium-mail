use crate::domain::providers::*;

pub struct MailComProvider {
    info: ProviderInfo,
}

impl MailComProvider {
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

impl Default for MailComProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for MailComProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["mail.com", "email.com"]
    }
}
