use crate::domain::providers::*;

pub struct CmccMailProvider {
    info: ProviderInfo,
}

impl CmccMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "cmcc".into(),
                name: "中国移动139邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["139.com".into(), "139.com.cn".into(), "10086.cn".into()],
                auth_type: AuthType::Password,
                color: Some("#0066FF".into()),
                icon: Some("cmcc".into()),
                sort_order: 7,
            },
        }
    }
}

impl Default for CmccMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for CmccMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.139.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.139.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["139.com", "139.com.cn", "10086.cn"]
    }
}
