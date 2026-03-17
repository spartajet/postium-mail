//! Microsoft 365 企业邮件服务商

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig, EnterpriseConfig};

/// Microsoft 365 企业邮件服务商
pub struct Microsoft365Provider {
    pub tenant_id: Option<String>,
}

impl Microsoft365Provider {
    pub fn new(tenant_id: Option<String>) -> Self {
        Self { tenant_id }
    }
}

#[async_trait]
impl MailProvider for Microsoft365Provider {
    fn provider_id(&self) -> &str {
        "microsoft365"
    }

    fn provider_name(&self) -> &str {
        "Microsoft 365"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Enterprise
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
            host: "smtp.office365.com".to_string(),
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
            tenant_id: self.tenant_id.clone(),
        })
    }

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: self.tenant_id.clone(),
            domain: None,
            conditional_access: true,
            mfa_required: true,
            custom_server: false,
            custom_imap: None,
            custom_smtp: None,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: true,
            supports_labels: false,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(150 * 1024 * 1024), // 150MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        // Microsoft 365 企业邮箱通常使用 onmicrosoft.com 域名
        // 或者需要管理员配置的自定义域名
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(domain.ends_with(".onmicrosoft.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![".onmicrosoft.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Microsoft365Provider::new(self.tenant_id.clone()))
    }
}
