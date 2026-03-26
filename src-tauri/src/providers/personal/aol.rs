//! AOL邮箱个人邮件服务商
//!
//! 支持 aol.com 等AOL邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// AOL邮箱个人邮件服务商
pub struct AolMailProvider {
    info: ProviderInfo,
}

impl AolMailProvider {
    /// 创建 AOL 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "aol".to_string(),
                name: "AOL邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "aol.com".to_string(),
                    "aim.com".to_string(),
                    "netscape.net".to_string(),
                    "cs.com".to_string(),
                    "verizon.net".to_string(),
                ],
                auth_types: vec![AuthType::Password, AuthType::OAuth2],
                capabilities: ProviderCapabilities {
                    supports_idle: true,
                    supports_push: false,
                    supports_oauth: true,
                    supports_enterprise: false,
                    supports_labels: false,
                    supports_folders: true,
                    supports_threads: true,
                    supports_search: true,
                    max_message_size: Some(25 * 1024 * 1024), // 25MB
                },
                icon: Some("aol".to_string()),
            },
        }
    }
}

impl Default for AolMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for AolMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.aol.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.aol.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        // AOL 使用与 Yahoo 相同的 OAuth 配置
        Some(OAuthConfig {
            client_id: "dj0yJmk9QVk2dDJFVVNSQUzJnPTFjYTVDMjNiN0J0Y1ZJT09nTVVNNE15TXpJbw"
                .to_string(),
            client_secret: None,
            auth_url: "https://api.login.yahoo.com/oauth2/request_auth".to_string(),
            token_url: "https://api.login.yahoo.com/oauth2/get_token".to_string(),
            redirect_uri: "oob".to_string(),
            scopes: vec!["mail-r".to_string(), "mail-w".to_string()],
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
            supports_labels: false,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(25 * 1024 * 1024), // 25MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(
            domain,
            "aol.com" | "aim.com" | "netscape.net" | "cs.com" | "verizon.net"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![
            "aol.com",
            "aim.com",
            "netscape.net",
            "cs.com",
            "verizon.net",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_aol_detection() {
        let provider = AolMailProvider::new();

        // 测试AOL邮箱域名
        assert!(provider.detect("test@aol.com").await.unwrap());
        assert!(provider.detect("test@aim.com").await.unwrap());
        assert!(provider.detect("test@netscape.net").await.unwrap());
        assert!(provider.detect("test@verizon.net").await.unwrap());

        // 测试非AOL域名
        assert!(!provider.detect("test@gmail.com").await.unwrap());
        assert!(!provider.detect("test@yahoo.com").await.unwrap());
    }

    #[test]
    fn test_aol_config() {
        let provider = AolMailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.aol.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.aol.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::StartTls
        ));
    }

    #[test]
    fn test_aol_domains() {
        let provider = AolMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec![
                "aol.com",
                "aim.com",
                "netscape.net",
                "cs.com",
                "verizon.net"
            ]
        );
    }

    #[test]
    fn test_aol_provider_info() {
        let provider = AolMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "aol");
        assert_eq!(info.name, "AOL邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
        assert!(info.auth_types.contains(&AuthType::OAuth2));
    }

    #[test]
    fn test_aol_capabilities() {
        let provider = AolMailProvider::new();
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(!caps.supports_push);
        assert!(caps.supports_oauth);
        assert!(!caps.supports_enterprise);
        assert!(!caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(25 * 1024 * 1024));
    }

    #[test]
    fn test_aol_box_clone() {
        let provider = AolMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "aol");
    }

    #[test]
    fn test_aol_oauth() {
        let provider = AolMailProvider::new();
        let oauth = provider.oauth_config();

        assert!(oauth.is_some());
        let oauth = oauth.unwrap();
        assert!(!oauth.auth_url.is_empty());
        assert!(!oauth.token_url.is_empty());
        assert!(!oauth.scopes.is_empty());
    }
}
