//! Yandex邮箱个人邮件服务商
//!
//! 支持 yandex.com、yandex.ru 等Yandex邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

/// Yandex邮箱个人邮件服务商
pub struct YandexMailProvider;

#[async_trait]
impl MailProvider for YandexMailProvider {
    fn provider_id(&self) -> &str {
        "yandex"
    }

    fn provider_name(&self) -> &str {
        "Yandex邮箱"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password, AuthType::OAuth2]
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.yandex.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.yandex.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: "f0db2d0f6f744411895e93b31b20e4f6".to_string(), // Yandex 官方客户端ID
            client_secret: None,
            auth_url: "https://oauth.yandex.com/authorize".to_string(),
            token_url: "https://oauth.yandex.com/token".to_string(),
            redirect_uri: "urn:ietf:wg:oauth:2.0:oob".to_string(),
            scopes: vec![
                "mail:imap_full".to_string(),
                "mail:smtp_full".to_string(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_push: false,
            supports_oauth: true,
            supports_enterprise: false,
            supports_labels: true,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024), // 50MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(
            domain,
            "yandex.com" | "yandex.ru" | "yandex.ua" | "yandex.by" | "yandex.kz" | "ya.ru"
                | "yandex.fr" | "yandex.com.tr"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![
            "yandex.com",
            "yandex.ru",
            "yandex.ua",
            "yandex.by",
            "yandex.kz",
            "ya.ru",
            "yandex.fr",
            "yandex.com.tr",
        ]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(YandexMailProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_yandex_detection() {
        let provider = YandexMailProvider;

        // 测试Yandex邮箱域名
        assert!(provider.detect("test@yandex.com").await.unwrap());
        assert!(provider.detect("test@yandex.ru").await.unwrap());
        assert!(provider.detect("test@ya.ru").await.unwrap());
        assert!(provider.detect("test@yandex.ua").await.unwrap());

        // 测试非Yandex域名
        assert!(!provider.detect("test@gmail.com").await.unwrap());
        assert!(!provider.detect("test@yahoo.com").await.unwrap());
    }

    #[test]
    fn test_yandex_config() {
        let provider = YandexMailProvider;

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.yandex.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.yandex.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::Implicit));
    }

    #[test]
    fn test_yandex_domains() {
        let provider = YandexMailProvider;
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec![
                "yandex.com",
                "yandex.ru",
                "yandex.ua",
                "yandex.by",
                "yandex.kz",
                "ya.ru",
                "yandex.fr",
                "yandex.com.tr"
            ]
        );
    }

    #[test]
    fn test_yandex_provider_info() {
        let provider = YandexMailProvider;

        assert_eq!(provider.provider_id(), "yandex");
        assert_eq!(provider.provider_name(), "Yandex邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert!(provider.auth_types().contains(&AuthType::Password));
        assert!(provider.auth_types().contains(&AuthType::OAuth2));
    }

    #[test]
    fn test_yandex_capabilities() {
        let provider = YandexMailProvider;
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(!caps.supports_push);
        assert!(caps.supports_oauth);
        assert!(!caps.supports_enterprise);
        assert!(caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(50 * 1024 * 1024));
    }

    #[test]
    fn test_yandex_box_clone() {
        let provider = YandexMailProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "yandex");
    }

    #[test]
    fn test_yandex_oauth() {
        let provider = YandexMailProvider;
        let oauth = provider.oauth_config();

        assert!(oauth.is_some());
        let oauth = oauth.unwrap();
        assert!(!oauth.auth_url.is_empty());
        assert!(!oauth.token_url.is_empty());
        assert!(!oauth.scopes.is_empty());
        assert_eq!(oauth.client_id, "f0db2d0f6f744411895e93b31b20e4f6");
    }
}
