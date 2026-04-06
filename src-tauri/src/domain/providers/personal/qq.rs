use crate::domain::providers::*;

pub struct QqMailProvider {
    info: ProviderInfo,
}

impl QqMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "qq".into(),
                name: "QQ Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["qq.com".into(), "foxmail.com".into()],
                auth_type: AuthType::Password,
                color: Some("#12B7F5".into()),
                icon: Some("qq".into()),
                sort_order: 1,
            },
        }
    }
}

impl Default for QqMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for QqMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.qq.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.qq.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["qq.com", "foxmail.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent Messages".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Junk".into()],
            trash: vec!["Deleted Messages".into()],
            archive: vec![],
        }
    }
}
