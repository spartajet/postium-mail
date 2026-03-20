//! QQ 邮箱个人邮件服务商
//!
//! 支持 qq.com、foxmail.com 等腾讯邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// QQ 邮箱个人邮件服务商
pub struct QqMailProvider;

#[async_trait]
impl MailProvider for QqMailProvider {
    fn provider_id(&self) -> &str {
        "qq"
    }

    fn provider_name(&self) -> &str {
        "QQ邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').nth(1).unwrap_or("");
        let host = match domain {
            "foxmail.com" => "imap.foxmail.com",
            _ => "imap.qq.com",
        };
        ImapServerConfig {
            host: host.to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        let domain = email.split('@').nth(1).unwrap_or("");
        let host = match domain {
            "foxmail.com" => "smtp.foxmail.com",
            _ => "smtp.qq.com",
        };
        SmtpServerConfig {
            host: host.to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // QQ 邮箱不支持 OAuth
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
        Ok(matches!(domain, "qq.com" | "foxmail.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["qq.com", "foxmail.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(QqMailProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qqmail_detection() {
        let provider = QqMailProvider;

        // 测试 QQ 邮箱域名
        assert!(provider.detect("test@qq.com").await.unwrap());
        assert!(provider.detect("test@foxmail.com").await.unwrap());

        // 测试非 QQ 域名
        assert!(!provider.detect("test@163.com").await.unwrap());
        assert!(!provider.detect("test@gmail.com").await.unwrap());
    }

    #[test]
    fn test_qqmail_config() {
        let provider = QqMailProvider;

        // 测试 QQ 邮箱 (qq.com) 的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@qq.com");
        assert_eq!(imap_config.host, "imap.qq.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        let smtp_config = provider.smtp_config("test@qq.com");
        assert_eq!(smtp_config.host, "smtp.qq.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::StartTls));

        // 测试 Foxmail 邮箱 (foxmail.com) 的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@foxmail.com");
        assert_eq!(imap_config.host, "imap.foxmail.com");

        let smtp_config = provider.smtp_config("test@foxmail.com");
        assert_eq!(smtp_config.host, "smtp.foxmail.com");
    }

    #[test]
    fn test_qqmail_domains() {
        let provider = QqMailProvider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["qq.com", "foxmail.com"]);
    }

    #[test]
    fn test_qqmail_provider_info() {
        let provider = QqMailProvider;

        assert_eq!(provider.provider_id(), "qq");
        assert_eq!(provider.provider_name(), "QQ邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_qqmail_capabilities() {
        let provider = QqMailProvider;
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
    fn test_qqmail_box_clone() {
        let provider = QqMailProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "qq");
    }
}
