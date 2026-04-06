use crate::domain::providers::*;

pub struct AolMailProvider {
    info: ProviderInfo,
}

impl AolMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "aol".into(),
                name: "AOL Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "aol.com".into(),
                    "aim.com".into(),
                    "netscape.net".into(),
                    "verizon.net".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#231F20".into()),
                icon: Some("aol".into()),
                sort_order: 12,
            },
        }
    }
}

impl Default for AolMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for AolMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.aol.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.aol.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["aol.com", "aim.com", "netscape.net", "verizon.net"]
    }
}
