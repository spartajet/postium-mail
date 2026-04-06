use crate::domain::providers::*;

pub struct SohuMailProvider {
    info: ProviderInfo,
}

impl SohuMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "sohu".into(),
                name: "搜狐邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "sohu.com".into(),
                    "vip.sohu.com".into(),
                    "sohu.net".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#DA1F26".into()),
                icon: Some("sohu".into()),
                sort_order: 16,
            },
        }
    }
}

impl Default for SohuMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for SohuMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.sohu.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.sohu.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sohu.com", "vip.sohu.com", "sohu.net"]
    }
}
