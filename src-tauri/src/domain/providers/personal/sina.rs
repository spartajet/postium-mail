use crate::domain::providers::*;

pub struct SinaMailProvider {
    info: ProviderInfo,
}

impl SinaMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "sina".into(),
                name: "新浪邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "sina.com".into(),
                    "sina.cn".into(),
                    "vip.sina.com".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#E6162D".into()),
                icon: Some("sina".into()),
                sort_order: 8,
            },
        }
    }
}

impl Default for SinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for SinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').next_back().unwrap_or("sina.com");
        let host = match domain {
            "sina.cn" => "imap.sina.cn",
            "vip.sina.com" => "imap.mail.sina.com.cn",
            _ => "imap.sina.com",
        };
        ImapServerConfig {
            host: host.into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        let domain = email.split('@').next_back().unwrap_or("sina.com");
        let host = match domain {
            "sina.cn" => "smtp.sina.cn",
            "vip.sina.com" => "smtp.mail.sina.com.cn",
            _ => "smtp.sina.com",
        };
        SmtpServerConfig {
            host: host.into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sina.com", "sina.cn", "vip.sina.com"]
    }
}
