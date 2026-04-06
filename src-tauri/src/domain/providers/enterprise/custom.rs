use crate::domain::providers::*;

/// 自定义服务商 — 所有配置从用户输入读取
pub struct CustomProvider {
    info: ProviderInfo,
    imap_host: String,
    imap_port: u16,
    smtp_host: String,
    smtp_port: u16,
}

impl CustomProvider {
    pub fn new(imap_host: &str, imap_port: u16, smtp_host: &str, smtp_port: u16) -> Self {
        Self {
            info: ProviderInfo {
                id: "custom".into(),
                name: "Custom IMAP/SMTP".into(),
                account_type: AccountType::Enterprise,
                domains: vec![],
                auth_type: AuthType::Password,
                color: None,
                icon: None,
                sort_order: 99,
            },
            imap_host: imap_host.into(),
            imap_port,
            smtp_host: smtp_host.into(),
            smtp_port,
        }
    }
}

impl MailProvider for CustomProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: self.imap_host.clone(),
            port: self.imap_port,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: self.smtp_host.clone(),
            port: self.smtp_port,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec![],
        }
    }
}
