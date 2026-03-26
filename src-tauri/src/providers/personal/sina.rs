//! 新浪邮箱个人邮件服务商
//!
//! 支持 sina.com、sina.cn 等新浪邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig, StandardFolder,
};
use async_trait::async_trait;

/// 新浪邮箱个人邮件服务商
pub struct SinaMailProvider {
    info: ProviderInfo,
}

impl SinaMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "sina".to_string(),
                name: "新浪邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec![
                    "sina.com".to_string(),
                    "sina.cn".to_string(),
                    "vip.sina.com".to_string(),
                    "2008.sina.com".to_string(),
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
                    max_message_size: Some(50 * 1024 * 1024),
                },
                icon: Some("sina".to_string()),
            },
        }
    }
}

impl Default for SinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for SinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').nth(1).unwrap_or("");
        let host = match domain {
            "sina.cn" => "imap.sina.cn",
            "vip.sina.com" | "2008.sina.com" => "imap.mail.sina.com.cn",
            _ => "imap.sina.com",
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
            "sina.cn" => "smtp.sina.cn",
            "vip.sina.com" | "2008.sina.com" => "smtp.mail.sina.com.cn",
            _ => "smtp.sina.com",
        };
        SmtpServerConfig {
            host: host.to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 新浪邮箱不支持 OAuth
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
            "sina.com" | "sina.cn" | "vip.sina.com" | "2008.sina.com"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sina.com", "sina.cn", "vip.sina.com", "2008.sina.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".to_string(), "收件箱".to_string()],
            sent: vec!["Sent".to_string(), "已发送".to_string()],
            drafts: vec!["Drafts".to_string(), "草稿箱".to_string()],
            spam: vec![
                "Spam".to_string(),
                "Junk".to_string(),
                "垃圾邮件".to_string(),
            ],
            trash: vec![
                "Trash".to_string(),
                "Deleted".to_string(),
                "已删除".to_string(),
            ],
            archive: vec!["Archive".to_string(), "归档".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sina_detection() {
        let provider = SinaMailProvider::new();

        // 测试新浪邮箱域名
        assert!(provider.detect("test@sina.com").await.unwrap());
        assert!(provider.detect("test@sina.cn").await.unwrap());
        assert!(provider.detect("test@vip.sina.com").await.unwrap());
        assert!(provider.detect("test@2008.sina.com").await.unwrap());

        // 测试非新浪域名
        assert!(!provider.detect("test@qq.com").await.unwrap());
        assert!(!provider.detect("test@163.com").await.unwrap());
    }

    #[test]
    fn test_sina_config() {
        let provider = SinaMailProvider::new();

        // 测试 sina.com 域名的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@sina.com");
        assert_eq!(imap_config.host, "imap.sina.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        let smtp_config = provider.smtp_config("test@sina.com");
        assert_eq!(smtp_config.host, "smtp.sina.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        // 测试 sina.cn 域名的配置
        let imap_config = provider.imap_config("test@sina.cn");
        assert_eq!(imap_config.host, "imap.sina.cn");

        // 测试 vip.sina.com 和 2008.sina.com 域名使用相同服务器
        let imap_config = provider.imap_config("test@vip.sina.com");
        assert_eq!(imap_config.host, "imap.mail.sina.com.cn");

        let imap_config = provider.imap_config("test@2008.sina.com");
        assert_eq!(imap_config.host, "imap.mail.sina.com.cn");
    }

    #[test]
    fn test_sina_domains() {
        let provider = SinaMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(
            domains,
            vec!["sina.com", "sina.cn", "vip.sina.com", "2008.sina.com"]
        );
    }

    #[test]
    fn test_sina_provider_info() {
        let provider = SinaMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "sina");
        assert_eq!(info.name, "新浪邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
    }

    #[test]
    fn test_sina_capabilities() {
        let provider = SinaMailProvider::new();
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
    fn test_sina_box_clone() {
        let provider = SinaMailProvider::new();
        let cloned = provider;

        assert_eq!(cloned.provider_info().id, "sina");
    }
}
