use crate::domain::providers::*;

pub struct ICloudProvider {
    info: ProviderInfo,
}

impl ICloudProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "icloud".into(),
                name: "iCloud Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["icloud.com".into(), "me.com".into(), "mac.com".into()],
                auth_type: AuthType::Password,
                color: Some("#3693F5".into()),
                icon: Some("icloud".into()),
                sort_order: 5,
            },
        }
    }
}

impl Default for ICloudProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for ICloudProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.me.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.me.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["icloud.com", "me.com", "mac.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent Messages".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Junk".into()],
            trash: vec!["Deleted Messages".into()],
            archive: vec!["Archive".into()],
        }
    }
}
