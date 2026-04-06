use crate::domain::providers::*;

pub struct ZohoProvider {
    info: ProviderInfo,
}

impl ZohoProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "zoho".into(),
                name: "Zoho Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["zoho.com".into(), "zohomail.com".into(), "zoho.eu".into()],
                auth_type: AuthType::Password,
                color: Some("#D4382C".into()),
                icon: Some("zoho".into()),
                sort_order: 11,
            },
        }
    }
}

impl Default for ZohoProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for ZohoProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.zoho.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.zoho.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["zoho.com", "zohomail.com", "zoho.eu"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec!["Archive".into()],
        }
    }
}
