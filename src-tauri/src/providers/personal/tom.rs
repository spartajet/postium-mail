//! Tom邮箱个人邮件服务商
//!
//! 支持 tom.com、mail.tom.com、163.tom.com 等Tom邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// Tom邮箱个人邮件服务商
pub struct TomMailProvider {
    info: ProviderInfo,
}

impl TomMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "tom".to_string(),
                name: "Tom邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "tom.com".to_string(),
                    "mail.tom.com".to_string(),
                    "163.tom.com".to_string(),
                    "vip.tom.com".to_string(),
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
                    max_message_size: Some(50 * 1024 * 1024), // 50MB
                },
                icon: Some("tom".to_string()),
            },
        }
    }
}

impl Default for TomMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for TomMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.tom.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.tom.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // Tom邮箱不支持 OAuth
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
        Ok(matches!(
            domain,
            "tom.com" | "mail.tom.com" | "163.tom.com" | "vip.tom.com"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["tom.com", "mail.tom.com", "163.tom.com", "vip.tom.com"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tom_detection() {
        let provider = TomMailProvider::new();

        // 测试Tom邮箱域名
        assert!(provider.detect("test@tom.com").await.unwrap());
        assert!(provider.detect("test@mail.tom.com").await.unwrap());
        assert!(provider.detect("test@163.tom.com").await.unwrap());
        assert!(provider.detect("test@vip.tom.com").await.unwrap());

        // 测试非Tom域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@163.com").await.unwrap());
    }

    #[test]
    fn test_tom_config() {
        let provider = TomMailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.tom.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.tom.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));
    }

    #[test]
    fn test_tom_domains() {
        let provider = TomMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec!["tom.com", "mail.tom.com", "163.tom.com", "vip.tom.com"]
        );
    }

    #[test]
    fn test_tom_provider_info() {
        let provider = TomMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "tom");
        assert_eq!(info.name, "Tom邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
    }

    #[test]
    fn test_tom_capabilities() {
        let provider = TomMailProvider::new();
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
    fn test_tom_box_clone() {
        let provider = TomMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "tom");
    }
}
