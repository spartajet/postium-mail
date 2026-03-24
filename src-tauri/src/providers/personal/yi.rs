//! 163 邮箱个人邮件服务商
//!
//! 支持 163.com、126.com、yeah.net 等网易邮箱域名

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    SmtpServerConfig, StandardFolder,
};
use async_trait::async_trait;

/// 163 邮箱个人邮件服务商
pub struct Mail163Provider;

#[async_trait]
impl MailProvider for Mail163Provider {
    fn provider_id(&self) -> &str {
        "yi"
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

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').nth(1).unwrap_or("");
        let host = match domain {
            "126.com" => "imap.126.com",
            "yeah.net" => "imap.yeah.net",
            _ => "imap.163.com",
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
            "126.com" => "smtp.126.com",
            "yeah.net" => "smtp.yeah.net",
            _ => "smtp.163.com",
        };
        SmtpServerConfig {
            host: host.to_string(),
            port: 465,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None // 163 邮箱不支持 OAuth
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: false,      // 不支持 IDLE 扩展
            supports_push: false,      // 不支持标准推送（虽有 XAPPLEPUSHSERVICE）
            supports_oauth: true,      // 支持 XOAUTH2 认证
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true, // 支持文件夹管理
            supports_threads: false,
            supports_search: true,            // 支持搜索
            max_message_size: Some(71680000), // APPENDLIMIT=71680000 (约68MB)
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(domain, "163.com" | "126.com" | "yeah.net"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["163.com", "126.com", "yeah.net"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".to_string()],
            sent: vec!["Sent".to_string(), "&XfJT0ZAB-".to_string()],
            drafts: vec!["&g0l6P3ux-".to_string()],
            spam: vec![
                "&W4xRaFeDVz6Qrk72-".to_string(),
                "&V4NXPpCuTvY-".to_string(),
            ],
            trash: vec!["&XfJSIJZk-".to_string()],
            archive: vec!["Archive".to_string(), "&W1hoYw-".to_string()],
        }
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

        // 测试 163.com 域名的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@163.com");
        assert_eq!(imap_config.host, "imap.163.com");
        assert_eq!(imap_config.port, 993);

        let smtp_config = provider.smtp_config("test@163.com");
        assert_eq!(smtp_config.host, "smtp.163.com");
        assert_eq!(smtp_config.port, 465);

        // 测试 126.com 域名的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@126.com");
        assert_eq!(imap_config.host, "imap.126.com");

        let smtp_config = provider.smtp_config("test@126.com");
        assert_eq!(smtp_config.host, "smtp.126.com");

        // 测试 yeah.net 域名的 IMAP/SMTP 配置
        let imap_config = provider.imap_config("test@yeah.net");
        assert_eq!(imap_config.host, "imap.yeah.net");

        let smtp_config = provider.smtp_config("test@yeah.net");
        assert_eq!(smtp_config.host, "smtp.yeah.net");
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

        assert_eq!(provider.provider_id(), "yi");
        assert_eq!(provider.provider_name(), "网易邮箱");
        assert_eq!(provider.account_type(), AccountType::Personal);
        assert_eq!(provider.auth_types(), vec![AuthType::Password]);
    }

    #[test]
    fn test_mail163_capabilities() {
        let provider = Mail163Provider;
        let caps = provider.capabilities();

        assert!(!caps.supports_idle);
        assert!(!caps.supports_push);
        assert!(caps.supports_oauth); // 支持 XOAUTH2 认证
        assert!(!caps.supports_enterprise);
        assert!(!caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(!caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(71680000)); // APPENDLIMIT=71680000
    }

    #[test]
    fn test_mail163_box_clone() {
        let provider = Mail163Provider;
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "yi");
    }
}
