//! Zoho邮箱个人邮件服务商
//!
//! 支持 zoho.com 等Zoho邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// Zoho邮箱个人邮件服务商
pub struct ZohoMailProvider {
    info: ProviderInfo,
}

impl ZohoMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "zoho".to_string(),
                name: "Zoho邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "zoho.com".to_string(),
                    "zohomail.com".to_string(),
                    "zoho.eu".to_string(),
                    "zoho.in".to_string(),
                    "zoho.com.au".to_string(),
                ],
                auth_types: vec![AuthType::Password, AuthType::OAuth2],
                capabilities: ProviderCapabilities {
                    supports_idle: true,
                    supports_push: false,
                    supports_oauth: true,
                    supports_enterprise: false,
                    supports_labels: true,
                    supports_folders: true,
                    supports_threads: true,
                    supports_search: true,
                    max_message_size: Some(50 * 1024 * 1024),
                },
                icon: Some("zoho".to_string()),
            },
        }
    }
}

impl Default for ZohoMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for ZohoMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.zoho.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.zoho.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: "1000.ZOHO.CLIENT".to_string(), // 需要替换为实际的 client_id
            client_secret: None,
            auth_url: "https://accounts.zoho.com/oauth/v2/auth".to_string(),
            token_url: "https://accounts.zoho.com/oauth/v2/token".to_string(),
            redirect_uri: "urn:ietf:wg:oauth:2.0:oob".to_string(),
            scopes: vec![
                "ZohoMail.messages.READ".to_string(),
                "ZohoMail.messages.WRITE".to_string(),
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
            "zoho.com" | "zohomail.com" | "zoho.eu" | "zoho.in" | "zoho.com.au"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![
            "zoho.com",
            "zohomail.com",
            "zoho.eu",
            "zoho.in",
            "zoho.com.au",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_zoho_detection() {
        let provider = ZohoMailProvider::new();

        // 测试Zoho邮箱域名
        assert!(provider.detect("test@zoho.com").await.unwrap());
        assert!(provider.detect("test@zohomail.com").await.unwrap());
        assert!(provider.detect("test@zoho.eu").await.unwrap());
        assert!(provider.detect("test@zoho.in").await.unwrap());

        // 测试非Zoho域名
        assert!(!provider.detect("test@gmail.com").await.unwrap());
        assert!(!provider.detect("test@yahoo.com").await.unwrap());
    }

    #[test]
    fn test_zoho_config() {
        let provider = ZohoMailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.zoho.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.zoho.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));
    }

    #[test]
    fn test_zoho_domains() {
        let provider = ZohoMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec![
                "zoho.com",
                "zohomail.com",
                "zoho.eu",
                "zoho.in",
                "zoho.com.au"
            ]
        );
    }

    #[test]
    fn test_zoho_provider_info() {
        let provider = ZohoMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "zoho");
        assert_eq!(info.name, "Zoho邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
        assert!(info.auth_types.contains(&AuthType::OAuth2));
    }

    #[test]
    fn test_zoho_capabilities() {
        let provider = ZohoMailProvider::new();
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
    fn test_zoho_box_clone() {
        let provider = ZohoMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "zoho");
    }

    #[test]
    fn test_zoho_oauth() {
        let provider = ZohoMailProvider::new();
        let oauth = provider.oauth_config();

        assert!(oauth.is_some());
        let oauth = oauth.unwrap();
        assert!(!oauth.auth_url.is_empty());
        assert!(!oauth.token_url.is_empty());
        assert!(!oauth.scopes.is_empty());
    }
}
