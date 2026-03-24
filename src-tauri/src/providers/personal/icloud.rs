//! iCloud 邮件服务商
//!
//! 支持 icloud.com、me.com、mac.com 等 Apple 邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// iCloud 邮件服务商
pub struct ICloudProvider;

#[async_trait]
impl MailProvider for ICloudProvider {
    fn provider_id(&self) -> &str {
        "icloud"
    }

    fn provider_name(&self) -> &str {
        "iCloud"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.me.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.me.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // iCloud 需要应用专用密码
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
        Ok(matches!(domain, "icloud.com" | "me.com" | "mac.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["icloud.com", "me.com", "mac.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(ICloudProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_icloud_detection() {
        let provider = ICloudProvider;

        // 测试 iCloud 邮箱域名
        assert!(provider.detect("test@icloud.com").await.unwrap());
        assert!(provider.detect("test@me.com").await.unwrap());
        assert!(provider.detect("test@mac.com").await.unwrap());

        // 测试非 iCloud 域名
        assert!(!provider.detect("test@163.com").await.unwrap());
        assert!(!provider.detect("test@gmail.com").await.unwrap());
    }

    #[test]
    fn test_icloud_config() {
        let provider = ICloudProvider;

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.mail.me.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.mail.me.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::StartTls));
    }

    #[test]
    fn test_icloud_domains() {
        let provider = ICloudProvider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["icloud.com", "me.com", "mac.com"]);
    }

    #[test]
    fn test_icloud_provider_info() {
        let provider = ICloudProvider;

        assert_eq!(provider.provider_id(), "icloud");
        assert_eq!(provider.provider_name(), "iCloud");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_icloud_capabilities() {
        let provider = ICloudProvider;
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
    fn test_icloud_box_clone() {
        let provider = ICloudProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "icloud");
    }
}
