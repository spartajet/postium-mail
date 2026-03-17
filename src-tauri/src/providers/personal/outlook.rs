//! Outlook 个人邮件服务商

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// Outlook 个人邮件服务商
pub struct OutlookProvider;

#[async_trait]
impl MailProvider for OutlookProvider {
    fn provider_id(&self) -> &str {
        "outlook"
    }

    fn provider_name(&self) -> &str {
        "Outlook"
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

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: "outlook.office365.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp-mail.outlook.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: "".to_string(), // 从环境变量加载
            client_secret: None,
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            redirect_uri: "http://localhost:3000/callback".to_string(),
            scopes: vec![
                "https://outlook.office.com/IMAP.AccessAsUser.All".to_string(),
                "https://outlook.office.com/SMTP.Send".to_string(),
                "offline_access".to_string(),
            ],
            pkce_enabled: true,
            tenant_id: Some("common".to_string()),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(35 * 1024 * 1024), // 35MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(
            domain,
            "outlook.com" | "hotmail.com" | "live.com" | "msn.com"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["outlook.com", "hotmail.com", "live.com", "msn.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(OutlookProvider)
    }
}
