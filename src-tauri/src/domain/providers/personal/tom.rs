use crate::domain::providers::*;

pub struct TomMailProvider {
    info: ProviderInfo,
}

impl TomMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "tom".into(),
                name: "Tom邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["tom.com".into()],
                auth_type: AuthType::Password,
                color: Some("#0066CC".into()),
                icon: Some("tom".into()),
                sort_order: 17,
            },
        }
    }
}

impl Default for TomMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for TomMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.tom.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.tom.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["tom.com"]
    }
}
