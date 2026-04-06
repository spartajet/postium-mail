use crate::domain::providers::*;

pub struct GmxMailProvider {
    info: ProviderInfo,
}

impl GmxMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "gmx".into(),
                name: "GMX Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "gmx.com".into(),
                    "gmx.net".into(),
                    "gmx.de".into(),
                    "gmx.fr".into(),
                    "gmx.co.uk".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#1C449B".into()),
                icon: Some("gmx".into()),
                sort_order: 13,
            },
        }
    }
}

impl Default for GmxMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for GmxMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmx.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "mail.gmx.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmx.com", "gmx.net", "gmx.de", "gmx.fr", "gmx.co.uk"]
    }
}
