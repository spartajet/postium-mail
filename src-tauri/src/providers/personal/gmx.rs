//! GMX邮箱个人邮件服务商
//!
//! 支持 gmx.com、gmx.net、gmx.co.uk 等GMX邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// GMX邮箱个人邮件服务商
pub struct GmxMailProvider;

#[async_trait]
impl MailProvider for GmxMailProvider {
    fn provider_id(&self) -> &str {
        "gmx"
    }

    fn provider_name(&self) -> &str {
        "GMX邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmx.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "mail.gmx.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // GMX 不支持标准 OAuth
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: true,
            supports_push: false,
            supports_oauth: false,
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024), // 50MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(
            domain,
            "gmx.com" | "gmx.net" | "gmx.co.uk" | "gmx.de" | "gmx.fr" | "gmx.es" | "gmx.it"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![
            "gmx.com",
            "gmx.net",
            "gmx.co.uk",
            "gmx.de",
            "gmx.fr",
            "gmx.es",
            "gmx.it",
        ]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(GmxMailProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gmx_detection() {
        let provider = GmxMailProvider;

        // 测试GMX邮箱域名
        assert!(provider.detect("test@gmx.com").await.unwrap());
        assert!(provider.detect("test@gmx.net").await.unwrap());
        assert!(provider.detect("test@gmx.de").await.unwrap());
        assert!(provider.detect("test@gmx.co.uk").await.unwrap());

        // 测试非GMX域名
        assert!(!provider.detect("test@gmail.com").await.unwrap());
        assert!(!provider.detect("test@yahoo.com").await.unwrap());
    }

    #[test]
    fn test_gmx_config() {
        let provider = GmxMailProvider;

        // 测试 IMAP 配置
        let imap_config = provider.default_imap_config();
        assert_eq!(imap_config.host, "imap.gmx.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.default_smtp_config();
        assert_eq!(smtp_config.host, "mail.gmx.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::StartTls));
    }

    #[test]
    fn test_gmx_domains() {
        let provider = GmxMailProvider;
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec!["gmx.com", "gmx.net", "gmx.co.uk", "gmx.de", "gmx.fr", "gmx.es", "gmx.it"]
        );
    }

    #[test]
    fn test_gmx_provider_info() {
        let provider = GmxMailProvider;

        assert_eq!(provider.provider_id(), "gmx");
        assert_eq!(provider.provider_name(), "GMX邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_gmx_capabilities() {
        let provider = GmxMailProvider;
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(caps.supports_condstore);
        assert!(!caps.supports_push);
        assert!(!caps.supports_oauth);
        assert!(!caps.supports_enterprise);
        assert!(!caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(!caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(50 * 1024 * 1024));
    }

    #[test]
    fn test_gmx_box_clone() {
        let provider = GmxMailProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "gmx");
    }
}
