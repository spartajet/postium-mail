use crate::domain::providers::*;

pub struct Net263MailProvider {
    info: ProviderInfo,
}

impl Net263MailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "net263".into(),
                name: "263邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["263.net".into(), "263.com".into(), "x263.net".into()],
                auth_type: AuthType::Password,
                color: Some("#FF6600".into()),
                icon: Some("net263".into()),
                sort_order: 18,
            },
        }
    }
}

impl Default for Net263MailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for Net263MailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.263.net".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.263.net".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["263.net", "263.com", "x263.net"]
    }
}
