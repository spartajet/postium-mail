//! 中国移动139邮箱个人邮件服务商
//!
//! 支持 139.com、139.com.cn、10086.cn 等中国移动邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig,
};
use async_trait::async_trait;

/// 中国移动139邮箱个人邮件服务商
pub struct CmccMailProvider {
    info: ProviderInfo,
}

impl CmccMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "cmcc".to_string(),
                name: "中国移动139邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "139.com".to_string(),
                    "139.com.cn".to_string(),
                    "10086.cn".to_string(),
                    "10086.com".to_string(),
                    "139mail.com".to_string(),
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
                icon: Some("cmcc".to_string()),
            },
        }
    }
}

impl Default for CmccMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for CmccMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.139.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.139.com".to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 中国移动139邮箱不支持 OAuth
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
            "139.com" | "139.com.cn" | "10086.cn" | "10086.com" | "139mail.com"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![
            "139.com",
            "139.com.cn",
            "10086.cn",
            "10086.com",
            "139mail.com",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cmcc_detection() {
        let provider = CmccMailProvider::new();

        // 测试中国移动139邮箱域名
        assert!(provider.detect("test@139.com").await.unwrap());
        assert!(provider.detect("test@139.com.cn").await.unwrap());
        assert!(provider.detect("test@10086.cn").await.unwrap());
        assert!(provider.detect("test@10086.com").await.unwrap());
        assert!(provider.detect("test@139mail.com").await.unwrap());

        // 测试非中国移动域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@163.com").await.unwrap());
    }

    #[test]
    fn test_cmcc_config() {
        let provider = CmccMailProvider::new();

        // 测试 IMAP 配置
        let imap_config = provider.imap_config("test@domain.com");
        assert_eq!(imap_config.host, "imap.139.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 SMTP 配置
        let smtp_config = provider.smtp_config("test@domain.com");
        assert_eq!(smtp_config.host, "smtp.139.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));
    }

    #[test]
    fn test_cmcc_domains() {
        let provider = CmccMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec![
                "139.com",
                "139.com.cn",
                "10086.cn",
                "10086.com",
                "139mail.com"
            ]
        );
    }

    #[test]
    fn test_cmcc_provider_info() {
        let provider = CmccMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "cmcc");
        assert_eq!(info.name, "中国移动139邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
    }

    #[test]
    fn test_cmcc_capabilities() {
        let provider = CmccMailProvider::new();
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
    fn test_cmcc_box_clone() {
        let provider = CmccMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "cmcc");
    }

    #[test]
    fn test_cmcc_provider_info_new() {
        let provider = CmccMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "cmcc");
        assert_eq!(info.name, "中国移动139邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
        assert!(info.capabilities.supports_idle);
    }
}
