//! Yahoo Mail 邮件服务商

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, ProviderInfo};

/// Yahoo Mail 邮件服务商
pub struct YahooProvider {
    info: ProviderInfo,
}

impl YahooProvider {
    /// 创建新的 YahooProvider 实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "yahoo".to_string(),
                name: "Yahoo Mail".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "yahoo.com".to_string(),
                    "yahoo.co.jp".to_string(),
                    "yahoo.co.uk".to_string(),
                    "yahoo.com.cn".to_string(),
                ],
                auth_types: vec![AuthType::OAuth2, AuthType::Password],
                capabilities: ProviderCapabilities {
                    supports_idle: true,
                    supports_push: false,
                    supports_oauth: true,
                    supports_enterprise: false,
                    supports_labels: false,
                    supports_folders: true,
                    supports_threads: false,
                    supports_search: true,
                    max_message_size: Some(25 * 1024 * 1024),
                },
                icon: None,
            },
        }
    }
}

impl Default for YahooProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for YahooProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.yahoo.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.yahoo.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(domain == "yahoo.com" || domain.ends_with(".yahoo.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["yahoo.com", "yahoo.co.jp", "yahoo.co.uk", "yahoo.com.cn"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Self::new())
    }
}
