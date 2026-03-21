//! Outlook 个人邮件服务商
//!
//! 支持 Outlook 个人邮箱（@outlook.com, @hotmail.com, @live.com 等）
//! OAuth 2.0、密码认证

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig, StandardFolder};

impl OutlookProvider {
    /// Outlook 默认客户端 ID
    ///
    /// 注册地址: https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps/ApplicationsListBlade
    /// 应用类型: Public client (desktop)
    const DEFAULT_CLIENT_ID: &str = "67acce3b-a85a-40c1-be02-44d954282442";

    /// Outlook 默认租户 ID
    ///
    /// "common" 表示允许使用个人账号和企业账号
    const DEFAULT_TENANT: &str = "common";

    /// Outlook 默认授权端点（使用 common 租户）
    const DEFAULT_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";

    /// Outlook 默认令牌端点（使用 common 租户）
    const DEFAULT_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

    /// Outlook 默认 OAuth Scopes
    ///
    /// - https://outlook.office.com/IMAP.AccessAsUser.All - 读取和管理邮箱中的所有邮件
    /// - https://outlook.office.com/SMTP.Send - 发送邮件
    /// - offline_access - 获取 refresh_token 以实现自动刷新
    const DEFAULT_SCOPES: &[&str] = &[
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://outlook.office.com/SMTP.Send",
        "offline_access",
        "openid",
    ];

    /// Outlook IMAP 服务器配置
    const IMAP_HOST: &str = "outlook.office365.com";
    const IMAP_PORT: u16 = 993;

    /// Outlook SMTP 服务器配置
    const SMTP_HOST: &str = "smtp-mail.outlook.com";
    const SMTP_PORT: u16 = 587;
}

/// Outlook 个人邮件服务商
pub struct OutlookProvider;

impl OutlookProvider {
    /// 获取 OAuth 配置
    pub fn oauth_config(&self) -> OAuthConfig {
        match OAuthConfig::from_env_for_provider("outlook") {
            Ok(config) => config,
            Err(_) => {
                tracing::warn!("使用 Outlook 硬编码 OAuth 配置，建议配置 src-tauri/.env 文件");
                let port = crate::config::get_oauth_callback_port();
                let redirect_uri = crate::config::generate_redirect_uri(port);

                tracing::info!("Outlook OAuth 配置: redirect_uri = {}", redirect_uri);

                OAuthConfig {
                    client_id: Self::DEFAULT_CLIENT_ID.to_string(),
                    client_secret: None, // Microsoft 不需要
                    auth_url: Self::DEFAULT_AUTH_URL.to_string(),
                    token_url: Self::DEFAULT_TOKEN_URL.to_string(),
                    redirect_uri,
                    scopes: Self::DEFAULT_SCOPES.iter().map(|s| s.to_string()).collect(),
                    pkce_enabled: true,
                    tenant_id: Some(Self::DEFAULT_TENANT.to_string()),
                }
            }
        }
    }
}

#[async_trait]
impl MailProvider for OutlookProvider {
    fn provider_id(&self) -> &str {
        "outlook"
    }

    fn provider_name(&self) -> &str {
        "Outlook"
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

    fn imap_config(&self, email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: Self::IMAP_HOST.to_string(),
            port: Self::IMAP_PORT,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
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
            supports_labels: false,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(35 * 1024 * 1024), // 35MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(matches!(
            domain,
            "outlook.com" | "hotmail.com" | "live.com" | "msn.com"
        ))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["outlook.com", "hotmail.com", "live.com", "msn.com"]
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["收件箱".to_string(), "INBOX".to_string()],
            sent: vec!["已发送".to_string(), "Sent".to_string(), "Sent Items".to_string()],
            drafts: vec!["草稿".to_string(), "Drafts".to_string()],
            spam: vec!["垃圾邮件".to_string(), "Junk".to_string(), "Junk Email".to_string()],
            trash: vec!["已删除邮件".to_string(), "Deleted".to_string(), "Deleted Items".to_string(), "Trash".to_string()],
            archive: vec!["归档".to_string(), "Archive".to_string()],
        }
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(OutlookProvider)
    }
}
