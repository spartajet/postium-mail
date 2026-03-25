//! QQ 邮箱个人邮件服务商
//!
//! 支持 qq.com、foxmail.com 等腾讯邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig, StandardFolder,
};
use async_trait::async_trait;

/// QQ 邮箱个人邮件服务商
pub struct QqMailProvider {
    info: ProviderInfo,
}

impl QqMailProvider {
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "qq".to_string(),
                name: "QQ邮箱".to_string(),
                account_type: AccountType::Personal,
                domains: vec!["qq.com".to_string(), "foxmail.com".to_string()],
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
                icon: Some("qq".to_string()),
            },
        }
    }
}

#[async_trait]
impl MailProvider for QqMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
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

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_qqmail_detection() {
        let provider = QqMailProvider::new();

        // 测试 QQ 邮箱域名
        assert!(provider.detect("test@qq.com").await.unwrap());
        assert!(provider.detect("test@foxmail.com").await.unwrap());

        // 测试非 QQ 域名
        assert!(!provider.detect("test@163.com").await.unwrap());
        assert!(!provider.detect("test@gmail.com").await.unwrap());
    }

    #[test]
    fn test_qqmail_config() {
        let provider = QqMailProvider::new();

        // 测试 QQ 邮箱 (qq.com) 的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@qq.com");
        assert_eq!(imap_config.host, "imap.qq.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(
            imap_config.ssl,
            crate::providers::SslMode::Implicit
        ));

        let smtp_config = provider.smtp_config("test@qq.com");
        assert_eq!(smtp_config.host, "smtp.qq.com");
        assert_eq!(smtp_config.port, 587);
        assert!(matches!(
            smtp_config.ssl,
            crate::providers::SslMode::StartTls
        ));

        // 测试 Foxmail 邮箱 (foxmail.com) 的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@foxmail.com");
        assert_eq!(imap_config.host, "imap.foxmail.com");

        let smtp_config = provider.smtp_config("test@foxmail.com");
        assert_eq!(smtp_config.host, "smtp.foxmail.com");
    }

    #[test]
    fn test_qqmail_domains() {
        let provider = QqMailProvider::new();
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["qq.com", "foxmail.com"]);
    }

    #[test]
    fn test_qqmail_provider_info() {
        let provider = QqMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "qq");
        assert_eq!(info.name, "QQ邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert_eq!(info.auth_types, vec![AuthType::Password]);
    }

    #[test]
    fn test_qqmail_capabilities() {
        let provider = QqMailProvider::new();
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
    fn test_qqmail_box_clone() {
        let provider = QqMailProvider::new();
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_info().id, "qq");
    }

    #[test]
    fn test_qqmail_provider_info_new() {
        let provider = QqMailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "qq");
        assert_eq!(info.name, "QQ邮箱");
        assert_eq!(info.account_type, AccountType::Personal);
        assert!(info.auth_types.contains(&AuthType::Password));
        assert!(info.capabilities.supports_idle);
    }
}
