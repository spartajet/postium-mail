use crate::domain::providers::*;

pub struct YahooProvider {
    info: ProviderInfo,
}

impl YahooProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "yahoo".into(),
                name: "Yahoo Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "yahoo.com".into(),
                    "yahoo.co.jp".into(),
                    "yahoo.co.uk".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#6001D2".into()),
                icon: Some("yahoo".into()),
                sort_order: 6,
            },
        }
    }
}

impl Default for YahooProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for YahooProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.yahoo.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.yahoo.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["yahoo.com", "yahoo.co.jp", "yahoo.co.uk"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Draft".into()],
            spam: vec!["Bulk Mail".into(), "Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec!["Archive".into()],
        }
    }
}
