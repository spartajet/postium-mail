//! 163 邮箱个人邮件服务商
//!
//! 支持 163.com、126.com、yeah.net 等网易邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// 163 邮箱个人邮件服务商
pub struct Mail163Provider;

#[async_trait]
impl MailProvider for Mail163Provider {
    fn provider_id(&self) -> &str {
        "163"
    }

    fn provider_name(&self) -> &str {
        "网易邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.163.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.163.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 163 邮箱不支持 OAuth
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: false,
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
        Ok(matches!(domain, "163.com" | "126.com" | "yeah.net"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["163.com", "126.com", "yeah.net"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Mail163Provider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mail163_detection() {
        let provider = Mail163Provider;

        // 测试 163 域名
        assert!(provider.detect("test@163.com").await.unwrap());
        assert!(provider.detect("test@126.com").await.unwrap());
        assert!(provider.detect("test@yeah.net").await.unwrap());

        // 测试非 163 域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@gmail.com").await.unwrap());
    }

    #[test]
    fn test_mail163_config() {
        let provider = Mail163Provider;

        // 测试 IMAP 配置
        let imap_config = provider.default_imap_config();
        assert_eq!(imap_config.host, "imap.163.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.default_smtp_config();
        assert_eq!(smtp_config.host, "smtp.163.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::Implicit));
    }

    #[test]
    fn test_mail163_domains() {
        let provider = Mail163Provider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["163.com", "126.com", "yeah.net"]);
    }

    #[test]
    fn test_mail163_provider_info() {
        let provider = Mail163Provider;

        assert_eq!(provider.provider_id(), "163");
        assert_eq!(provider.provider_name(), "网易邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_mail163_capabilities() {
        let provider = Mail163Provider;
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(!caps.supports_condstore);
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
    fn test_mail163_box_clone() {
        let provider = Mail163Provider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "163");
    }
}
