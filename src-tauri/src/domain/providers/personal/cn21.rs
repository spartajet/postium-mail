use crate::domain::providers::*;

pub struct Cn21MailProvider {
    info: ProviderInfo,
}

impl Cn21MailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "cn21".into(),
                name: "21CN邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["21cn.com".into(), "21cn.net".into()],
                auth_type: AuthType::Password,
                color: Some("#0066CC".into()),
                icon: Some("cn21".into()),
                sort_order: 19,
            },
        }
    }
}

impl Default for Cn21MailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for Cn21MailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.21cn.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.21cn.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["21cn.com", "21cn.net"]
    }
}
