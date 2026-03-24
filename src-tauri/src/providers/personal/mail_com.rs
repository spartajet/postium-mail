//! Mail.com邮箱个人邮件服务商
//!
//! 支持 mail.com 等邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// Mail.com邮箱个人邮件服务商
pub struct MailComProvider;

#[async_trait]
impl MailProvider for MailComProvider {
    fn provider_id(&self) -> &str {
        "mailcom"
    }

    fn provider_name(&self) -> &str {
        "Mail.com邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // Mail.com 不支持标准 OAuth
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
        Ok(matches!(domain, "mail.com" | "email.com" | "myemail.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["mail.com", "email.com", "myemail.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(MailComProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mailcom_detection() {
        let provider = MailComProvider;

        // 测试Mail.com邮箱域名
        assert!(provider.detect("test@mail.com").await.unwrap());
        assert!(provider.detect("test@email.com").await.unwrap());
        assert!(provider.detect("test@myemail.com").await.unwrap());

        // 测试非Mail.com域名
        assert!(!provider.detect("test@gmail.com").await.unwrap());
        assert!(!provider.detect("test@yahoo.com").await.unwrap());
    }

    #[test]
    fn test_mailcom_config() {
        let provider = MailComProvider;

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.mail.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.mail.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::StartTls));
    }

    #[test]
    fn test_mailcom_domains() {
        let provider = MailComProvider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["mail.com", "email.com", "myemail.com"]);
    }

    #[test]
    fn test_mailcom_provider_info() {
        let provider = MailComProvider;

        assert_eq!(provider.provider_id(), "mailcom");
        assert_eq!(provider.provider_name(), "Mail.com邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_mailcom_capabilities() {
        let provider = MailComProvider;
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
    fn test_mailcom_box_clone() {
        let provider = MailComProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "mailcom");
    }
}
