use crate::domain::providers::*;

pub struct ChinaMailProvider {
    info: ProviderInfo,
}

impl ChinaMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "china".into(),
                name: "中华网邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["china.com".into(), "mail.china.com".into()],
                auth_type: AuthType::Password,
                color: Some("#CC0000".into()),
                icon: Some("china".into()),
                sort_order: 20,
            },
        }
    }
}

impl Default for ChinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for ChinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.china.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.china.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["china.com", "mail.china.com"]
    }
}
