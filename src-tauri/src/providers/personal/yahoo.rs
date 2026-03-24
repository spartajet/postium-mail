//! Yahoo Mail 邮件服务商

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities};

/// Yahoo Mail 邮件服务商
pub struct YahooProvider;

#[async_trait]
impl MailProvider for YahooProvider {
    fn provider_id(&self) -> &str {
        "yahoo"
    }

    fn provider_name(&self) -> &str {
        "Yahoo Mail"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![
            AuthType::OAuth2,
            AuthType::Password,
        ]
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

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_push: false,
            supports_oauth: true,
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(25 * 1024 * 1024),
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
        Box::new(YahooProvider)
    }
}
