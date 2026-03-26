//! 263邮箱个人邮件服务商
//!
//! 支持 263.net、263.com、x263.net 等263邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// 263邮箱个人邮件服务商
pub struct Net263MailProvider {
    info: ProviderInfo,
}

impl Net263MailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "net263".to_string(),
                name: "263邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "263.net".to_string(),
                    "263.com".to_string(),
                    "x263.net".to_string(),
                ],
                auth_types: vec![AuthType::Password],
                capabilities: ProviderCapabilities {
                    supports_idle: true,
                    supports_push: false,
                    supports_oauth: false,
                    supports_enterprise: false,
                    supports_labels: false,
                    supports_folders: true,
                    supports_threads: false,
                    supports_search: true,
                    max_message_size: Some(50 * 1024 * 1024),
                },
                icon: Some("net263".to_string()),
            },
        }
    }
}

impl Default for Net263MailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for Net263MailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_net263_detection() {
        let provider = Net263MailProvider::new();

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
        let provider = Net263MailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.263.net");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.263.net");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));
    }

    #[test]
    fn test_net263_domains() {
        let provider = Net263MailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["263.net", "263.com", "x263.net"]);
    }

    #[test]
    fn test_net263_provider_info() {
        let provider = Net263MailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "net263");
        assert_eq!(info.name, "263邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
    }

    #[test]
    fn test_net263_capabilities() {
        let provider = Net263MailProvider::new();
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
        let provider = Net263MailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "net263");
    }
}
