//! 新浪邮箱个人邮件服务商
//!
//! 支持 sina.com、sina.cn 等新浪邮箱域名

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig, StandardFolder};

/// 新浪邮箱个人邮件服务商
pub struct SinaMailProvider;

#[async_trait]
impl MailProvider for SinaMailProvider {
    fn provider_id(&self) -> &str {
        "sina"
    }

    fn provider_name(&self) -> &str {
        "新浪邮箱"
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
        Ok(matches!(domain, "sina.com" | "sina.cn" | "vip.sina.com" | "2008.sina.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sina.com", "sina.cn", "vip.sina.com", "2008.sina.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".to_string(), "收件箱".to_string()],
            sent: vec!["Sent".to_string(), "已发送".to_string()],
            drafts: vec!["Drafts".to_string(), "草稿箱".to_string()],
            spam: vec!["Spam".to_string(), "Junk".to_string(), "垃圾邮件".to_string()],
            trash: vec!["Trash".to_string(), "Deleted".to_string(), "已删除".to_string()],
            archive: vec!["Archive".to_string(), "归档".to_string()],
        }
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(SinaMailProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sina_detection() {
        let provider = SinaMailProvider;

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
        let provider = SinaMailProvider;

        // 测试 sina.com 域名的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@sina.com");
        assert_eq!(imap_config.host, "imap.sina.com");
        assert_eq!(imap_config.port, 993);
        assert!(matches!(imap_config.ssl, crate::providers::SslMode::Implicit));

        let smtp_config = provider.smtp_config("test@sina.com");
        assert_eq!(smtp_config.host, "smtp.sina.com");
        assert_eq!(smtp_config.port, 465);
        assert!(matches!(smtp_config.ssl, crate::providers::SslMode::Implicit));

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
        let provider = SinaMailProvider;
        let domains = provider.supported_domains();

        assert_eq!(domains, vec!["sina.com", "sina.cn", "vip.sina.com", "2008.sina.com"]);
    }

    #[test]
    fn test_sina_provider_info() {
        let provider = SinaMailProvider;

        assert_eq!(provider.provider_id(), "sina");
        assert_eq!(provider.provider_name(), "新浪邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_sina_capabilities() {
        let provider = SinaMailProvider;
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
    fn test_sina_box_clone() {
        let provider = SinaMailProvider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "sina");
    }
}
