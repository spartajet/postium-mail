//! 中华网邮箱个人邮件服务商
//!
//! 支持 china.com、mail.china.com 等中华网邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// 中华网邮箱个人邮件服务商
pub struct ChinaMailProvider {
    info: ProviderInfo,
}

impl ChinaMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "china".to_string(),
                name: "中华网邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec!["china.com".to_string(), "mail.china.com".to_string()],
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
                icon: Some("china".to_string()),
            },
        }
    }
}

impl Default for ChinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for ChinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.china.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.china.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 中华网邮箱不支持 OAuth
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
        Ok(matches!(domain, "china.com" | "mail.china.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["china.com", "mail.china.com"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_china_detection() {
        let provider = ChinaMailProvider::new();

        // 测试中华网邮箱域名
        assert!(provider.detect("test@china.com").await.unwrap());
        assert!(provider.detect("test@mail.china.com").await.unwrap());

        // 测试非中华网域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@163.com").await.unwrap());
    }

    #[test]
    fn test_china_config() {
        let provider = ChinaMailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.china.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.china.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));
    }

    #[test]
    fn test_china_domains() {
        let provider = ChinaMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["china.com", "mail.china.com"]);
    }

    #[test]
    fn test_china_provider_info() {
        let provider = ChinaMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "china");
        assert_eq!(info.name, "中华网邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
    }

    #[test]
    fn test_china_capabilities() {
        let provider = ChinaMailProvider::new();
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
    fn test_china_box_clone() {
        let provider = ChinaMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "china");
    }

    #[test]
    fn test_china_provider_info_new() {
        let provider = ChinaMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "china");
        assert_eq!(info.name, "中华网邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
        assert!(info.capabilities.supports_idle);
    }
}
