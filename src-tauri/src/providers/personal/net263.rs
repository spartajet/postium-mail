//! 263邮箱个人邮件服务商
//!
//! 支持 263.net、263.com、x263.net 等263邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// 263邮箱个人邮件服务商
pub struct Net263MailProvider;

#[async_trait]
impl MailProvider for Net263MailProvider {
    fn provider_id(&self) -> &str {
        "net263"
    }

    fn provider_name(&self) -> &str {
        "263邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.263.net".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.263.net".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 263邮箱不支持 OAuth
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
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
        Ok(matches!(domain, "263.net" | "263.com" | "x263.net"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["263.net", "263.com", "x263.net"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Net263MailProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_net263_detection() {
        let provider = Net263MailProvider;

        // 测试263邮箱域名
        assert!(provider.detect("test@263.net").await.unwrap());
        assert!(provider.detect("test@263.com").await.unwrap());
        assert!(provider.detect("test@x263.net").await.unwrap());

        // 测试非263域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@163.com").await.unwrap());
    }

    #[test]
    fn test_net263_config() {
        let provider = Net263MailProvider;

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.263.net");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.263.net");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::Implicit));
    }

    #[test]
    fn test_net263_domains() {
        let provider = Net263MailProvider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["263.net", "263.com", "x263.net"]);
    }

    #[test]
    fn test_net263_provider_info() {
        let provider = Net263MailProvider;

        assert_eq!(provider.provider_id(), "net263");
        assert_eq!(provider.provider_name(), "263邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_net263_capabilities() {
        let provider = Net263MailProvider;
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
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
    fn test_net263_box_clone() {
        let provider = Net263MailProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "net263");
    }
}
