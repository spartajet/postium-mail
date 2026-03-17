//! Gmail 个人邮件服务商
//!
//! 支持 Gmail 个人邮箱
//! OAuth 2.0、密码认证、应用专用密码

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig};

impl GmailProvider {
    /// Gmail 默认客户端 ID
    ///
    /// 注册地址: https://console.cloud.google.com/
    /// 应用类型: Desktop app
    /// 授权重定向 URI: postium-mail://oauth/callback
    const DEFAULT_CLIENT_ID: &str = "56071600997-2ggvvrf279h5391a2uka4aigisabbsja.apps.googleusercontent.com";

    /// Gmail 默认重定向 URI
    ///
    /// 使用自定义 Deep Link 方案
    const DEFAULT_REDIRECT_URI: &str = "postium-mail://oauth/callback";

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
}

/// Gmail 个人邮件服务商
pub struct GmailProvider;

impl GmailProvider {
    /// 获取 OAuth 配置
    pub fn oauth_config(&self) -> OAuthConfig {
        OAuthConfig {
            client_id: Self::DEFAULT_CLIENT_ID.to_string(),
            client_secret: None, // 桌面应用不需要 client_secret
            auth_url: Self::DEFAULT_AUTH_URL.to_string(),
            token_url: Self::DEFAULT_TOKEN_URL.to_string(),
            redirect_uri: Self::DEFAULT_REDIRECT_URI.to_string(),
            scopes: Self::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
            pkce_enabled: true,
            tenant_id: None,
        }
    }
}

#[async_trait]
impl MailProvider for GmailProvider {
    fn provider_id(&self) -> &str {
        "gmail"
    }

    fn provider_name(&self) -> &str {
        "Gmail"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![
            AuthType::OAuth2,
            AuthType::Password,
        ]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        ImapServerConfig {
            host: Self::IMAP_HOST.to_string(),
            port: Self::IMAP_PORT,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        SmtpServerConfig {
            host: Self::SMTP_HOST.to_string(),
            port: Self::SMTP_PORT,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(self.oauth_config())
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: false,
            supports_labels: true,
            supports_folders: false,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024), // 50MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(domain == "gmail.com" || domain == "googlemail.com")
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmail.com", "googlemail.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(GmailProvider)
    }
}
