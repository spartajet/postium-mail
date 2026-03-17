//! Google Workspace 企业邮件服务商

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig, EnterpriseConfig};

/// Google Workspace 企业邮件服务商
pub struct GoogleWorkspaceProvider {
    pub domain: Option<String>,
}

impl GoogleWorkspaceProvider {
    pub fn new(domain: Option<String>) -> Self {
        Self { domain }
    }
}

#[async_trait]
impl MailProvider for GoogleWorkspaceProvider {
    fn provider_id(&self) -> &str {
        "google-workspace"
    }

    fn provider_name(&self) -> &str {
        "Google Workspace"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Enterprise
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::OAuth2]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmail.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.gmail.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: "".to_string(), // 从环境变量加载
            client_secret: None,
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri: "http://localhost:3000/callback".to_string(),
            scopes: vec![
                "https://mail.google.com/".to_string(),
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        })
    }

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: None,
            domain: self.domain.clone(),
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
            supports_labels: true,
            supports_folders: false,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024),
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        // 如果配置了域名，检查邮箱是否属于该域名
        if let Some(ref domain) = self.domain {
            let email_domain = email.split('@').nth(1).unwrap_or("");
            return Ok(email_domain == domain);
        }
        Ok(false)
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![] // Google Workspace 使用动态域名，不在静态列表中
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(GoogleWorkspaceProvider::new(self.domain.clone()))
    }
}
