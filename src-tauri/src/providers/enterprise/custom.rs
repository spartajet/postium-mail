//! 自定义企业邮件服务商

use super::super::{
    AccountType, AuthType, EnterpriseConfig, ImapServerConfig, MailProvider, ProviderCapabilities,
    SmtpServerConfig,
};
use async_trait::async_trait;

/// 自定义企业邮件服务商
pub struct CustomProvider {
    pub name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

impl CustomProvider {
    pub fn new(
        name: String,
        imap_host: String,
        imap_port: u16,
        smtp_host: String,
        smtp_port: u16,
    ) -> Self {
        Self {
            name,
            imap_host,
            imap_port,
            smtp_host,
            smtp_port,
        }
    }
}

#[async_trait]
impl MailProvider for CustomProvider {
    fn provider_id(&self) -> &str {
        "custom"
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    fn account_type(&self) -> AccountType {
        AccountType::Enterprise
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password, AuthType::OAuth2]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: self.imap_host.clone(),
            port: self.imap_port,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: self.smtp_host.clone(),
            port: self.smtp_port,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: None,
            domain: None,
            conditional_access: false,
            mfa_required: false,
            custom_server: true,
            custom_imap: Some(crate::providers::ImapConfig {
                host: self.imap_host.clone(),
                port: self.imap_port,
                ssl: crate::providers::SslMode::Implicit,
            }),
            custom_smtp: Some(crate::providers::SmtpConfig {
                host: self.smtp_host.clone(),
                port: self.smtp_port,
                ssl: crate::providers::SslMode::StartTls,
            }),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: false,
            supports_condstore: false,
            supports_push: false,
            supports_oauth: false,
            supports_enterprise: true,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(25 * 1024 * 1024),
        }
    }

    async fn detect(&self, _email: &str) -> crate::error::Result<bool> {
        Ok(false) // 自定义服务器不自动检测
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        Vec::new()
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(CustomProvider::new(
            self.name.clone(),
            self.imap_host.clone(),
            self.imap_port,
            self.smtp_host.clone(),
            self.smtp_port,
        ))
    }
}
