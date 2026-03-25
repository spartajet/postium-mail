//! Gmail 个人邮件服务商
//!
//! 支持 Gmail 个人邮箱
//! OAuth 2.0、密码认证、应用专用密码

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities,
    ProviderInfo, SmtpServerConfig, StandardFolder,
};
use async_trait::async_trait;

/// Gmail 个人邮件服务商
pub struct GmailProvider {
    info: ProviderInfo,
}

impl GmailProvider {
    /// Gmail 默认客户端 ID
    ///
    /// 注册地址: https://console.cloud.google.com/
    /// 应用类型: Desktop app
    /// 授权重定向 URI: postium-mail://oauth/callback
    const DEFAULT_CLIENT_ID: &str =
        "56071600997-2ggvvrf279h5391a2uka4aigisabbsja.apps.googleusercontent.com";

    /// Gmail 默认授权端点
    const DEFAULT_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

    /// Gmail 默认令牌端点
    const DEFAULT_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

    /// Gmail 默认 OAuth Scopes
    ///
    /// - https://mail.google.com/ - 完整访问 Gmail（读取、发送、管理邮件）
    /// - https://www.googleapis.com/auth/userinfo.email - 获取用户邮箱地址
    const DEFAULT_SCOPES: &[&str] = &[
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/userinfo.email",
    ];

    /// Gmail IMAP 服务器配置
    const IMAP_HOST: &str = "imap.gmail.com";
    const IMAP_PORT: u16 = 993;

    /// Gmail SMTP 服务器配置
    const SMTP_HOST: &str = "smtp.gmail.com";
    const SMTP_PORT: u16 = 587;

    /// 创建 Gmail 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "gmail".to_string(),
                name: "Gmail".to_string(),
                account_type: AccountType::Personal,
                domains: vec!["gmail.com".to_string(), "googlemail.com".to_string()],
                auth_types: vec![AuthType::OAuth2, AuthType::Password],
                capabilities: ProviderCapabilities {
                    supports_idle: true,
                    supports_push: true,
                    supports_oauth: true,
                    supports_enterprise: false,
                    supports_labels: true,
                    supports_folders: false,
                    supports_threads: true,
                    supports_search: true,
                    max_message_size: Some(50 * 1024 * 1024), // 50MB
                },
                icon: Some("gmail".to_string()),
            },
        }
    }

    /// 获取 OAuth 配置
    pub fn oauth_config(&self) -> OAuthConfig {
        // 从环境变量加载配置（支持 client_secret）
        match OAuthConfig::from_env_for_provider("gmail") {
            Ok(config) => config,
            Err(_) => {
                // 回退到硬编码配置
                tracing::warn!("使用 Gmail 硬编码 OAuth 配置，建议配置 src-tauri/.env 文件");
                let port = crate::config::get_oauth_callback_port();
                let redirect_uri = crate::config::generate_redirect_uri(port);

                tracing::info!("Gmail OAuth 配置: redirect_uri = {}", redirect_uri);

                OAuthConfig {
                    client_id: Self::DEFAULT_CLIENT_ID.to_string(),
                    client_secret: None, // 硬编码默认值
                    auth_url: Self::DEFAULT_AUTH_URL.to_string(),
                    token_url: Self::DEFAULT_TOKEN_URL.to_string(),
                    redirect_uri,
                    scopes: Self::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
                    pkce_enabled: true,
                    tenant_id: None,
                }
            }
        }
    }
}

impl Default for GmailProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailProvider for GmailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: Self::IMAP_HOST.to_string(),
            port: Self::IMAP_PORT,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: Self::SMTP_HOST.to_string(),
            port: Self::SMTP_PORT,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(self.oauth_config())
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(domain == "gmail.com" || domain == "googlemail.com")
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmail.com", "googlemail.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".to_string()],
            sent: vec!["Sent".to_string(), "[Gmail]/Sent Mail".to_string()],
            drafts: vec!["Drafts".to_string(), "[Gmail]/Drafts".to_string()],
            spam: vec!["Spam".to_string(), "[Gmail]/Spam".to_string()],
            trash: vec!["Trash".to_string(), "[Gmail]/Trash".to_string()],
            archive: vec!["[Gmail]/All Mail".to_string()],
        }
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gmail_provider_info() {
        let provider = GmailProvider::new();
        let info = provider.provider_info();

        assert_eq!(info.id, "gmail");
        assert_eq!(info.name, "Gmail");
        assert_eq!(info.account_type, AccountType::Personal);
        assert_eq!(info.domains, vec!["gmail.com", "googlemail.com"]);
        assert_eq!(info.auth_types, vec![AuthType::OAuth2, AuthType::Password]);
        assert!(info.capabilities.supports_oauth);
        assert!(info.capabilities.supports_labels);
        assert_eq!(info.icon, Some("gmail".to_string()));
    }

    #[test]
    fn test_gmail_imap_config() {
        let provider = GmailProvider::new();
        let config = provider.imap_config("test@gmail.com");

        assert_eq!(config.host, "imap.gmail.com");
        assert_eq!(config.port, 993);
    }

    #[test]
    fn test_gmail_smtp_config() {
        let provider = GmailProvider::new();
        let config = provider.smtp_config("test@gmail.com");

        assert_eq!(config.host, "smtp.gmail.com");
        assert_eq!(config.port, 587);
    }

    #[test]
    fn test_gmail_box_clone() {
        let provider = GmailProvider::new();
        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_info().id, "gmail");
    }
}
