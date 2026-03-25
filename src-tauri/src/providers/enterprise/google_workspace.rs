//! Google Workspace 企业邮件服务商
//!
//! 支持 Google Workspace (formerly G Suite) 企业邮箱
//! 企业域名 OAuth、单点登录 (SSO)、企业安全策略支持

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, ProviderInfo, OAuthConfig, EnterpriseConfig};

impl GoogleWorkspaceProvider {
    /// Google Workspace 默认客户端 ID
    ///
    /// 注册地址: https://console.cloud.google.com/
    /// 应用类型: Desktop app
    /// 授权重定向 URI: postium-mail://oauth/callback
    const DEFAULT_CLIENT_ID: &str = "56071600997-2ggvvrf279h5391a2uka4aigisabbsja.apps.googleusercontent.com";

    /// Google Workspace 默认重定向 URI
    ///
    /// 使用自定义 Deep Link 方案
    const DEFAULT_REDIRECT_URI: &str = "postium-mail://oauth/callback";

    /// Google Workspace 默认授权端点
    const DEFAULT_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";

    /// Google Workspace 默认令牌端点
    const DEFAULT_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

    /// Google Workspace 默认 OAuth Scopes
    ///
    /// - https://mail.google.com/ - 完整访问 Gmail（读取、发送、管理邮件）
    /// - https://www.googleapis.com/auth/userinfo.email - 获取用户邮箱地址
    const DEFAULT_SCOPES: &[&str] = &[
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/userinfo.email",
    ];

    /// Google Workspace IMAP 服务器配置
    const IMAP_HOST: &str = "imap.gmail.com";
    const IMAP_PORT: u16 = 993;

    /// Google Workspace SMTP 服务器配置
    const SMTP_HOST: &str = "smtp.gmail.com";
    const SMTP_PORT: u16 = 587;
}

/// Google Workspace 企业邮件服务商
pub struct GoogleWorkspaceProvider {
    domain: Option<String>,
    info: ProviderInfo,
}

impl GoogleWorkspaceProvider {
    /// 使用默认配置创建服务商
    ///
    /// 这是推荐的方式，使用预配置的 Google Workspace 凭证。
    pub fn with_defaults() -> Self {
        Self::new(None)
    }

    /// 使用指定企业域名创建服务商
    ///
    /// # 参数
    ///
    /// * `domain` - 企业域名（如：example.com）
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 使用默认配置（任意域名）
    /// let provider = GoogleWorkspaceProvider::with_defaults();
    ///
    /// // 使用特定企业域名
    /// let provider = GoogleWorkspaceProvider::with_domain("example.com");
    /// ```
    pub fn with_domain<S: Into<String>>(domain: S) -> Self {
        Self::new(Some(domain.into()))
    }

    pub fn new(domain: Option<String>) -> Self {
        let info = ProviderInfo {
            id: "google-workspace".to_string(),
            name: "Google Workspace".to_string(),
            account_type: AccountType::Enterprise,
            domains: vec![],
            auth_types: vec![AuthType::OAuth2],
            capabilities: ProviderCapabilities {
                supports_idle: true,
                supports_push: true,
                supports_oauth: true,
                supports_enterprise: true,
                supports_labels: true,
                supports_folders: false,
                supports_threads: true,
                supports_search: true,
                max_message_size: Some(50 * 1024 * 1024),
            },
            icon: Some("google-workspace".to_string()),
        };
        Self { domain, info }
    }
}

impl Default for GoogleWorkspaceProvider {
    fn default() -> Self {
        Self::new(None)
    }
}

impl GoogleWorkspaceProvider {
    /// 获取 OAuth 配置
    pub fn oauth_config(&self) -> OAuthConfig {
        match OAuthConfig::from_env_for_provider("googleworkspace") {
            Ok(config) => config,
            Err(_) => {
                tracing::warn!("使用 Google Workspace 硬编码 OAuth 配置，建议配置 src-tauri/.env 文件");
                let port = crate::config::get_oauth_callback_port();
                let redirect_uri = crate::config::generate_redirect_uri(port);

                tracing::info!("GoogleWorkspace OAuth 配置: redirect_uri = {}", redirect_uri);

                OAuthConfig {
                    client_id: Self::DEFAULT_CLIENT_ID.to_string(),
                    client_secret: None,
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

#[async_trait]
impl MailProvider for GoogleWorkspaceProvider {
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

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: None,
            domain: self.domain.clone(),
            conditional_access: true,
            mfa_required: true,
            custom_server: false,
            custom_imap: None,
            custom_smtp: None,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: true,
            supports_labels: true,
            supports_folders: false,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024),
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        // 如果配置了域名，检查邮箱是否属于该域名
        if let Some(ref domain) = self.domain {
            let email_domain = email.split('@').nth(1).unwrap_or("");
            return Ok(email_domain == domain);
        }
        Ok(false)
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![] // Google Workspace 使用动态域名，不在静态列表中
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(GoogleWorkspaceProvider::new(self.domain.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_workspace_with_defaults() {
        let provider = GoogleWorkspaceProvider::with_defaults();

        assert_eq!(provider.domain, None);
    }

    #[test]
    fn test_google_workspace_with_domain() {
        let provider = GoogleWorkspaceProvider::with_domain("example.com");

        assert_eq!(provider.domain, Some("example.com".to_string()));
    }

    #[test]
    fn test_google_workspace_with_none() {
        let provider = GoogleWorkspaceProvider::new(None);

        assert_eq!(provider.domain, None);
    }

    #[test]
    fn test_google_workspace_oauth_config() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let oauth_config = provider.oauth_config();

        assert_eq!(oauth_config.client_id, GoogleWorkspaceProvider::DEFAULT_CLIENT_ID);
        assert_eq!(oauth_config.auth_url, GoogleWorkspaceProvider::DEFAULT_AUTH_URL);
        assert_eq!(oauth_config.token_url, GoogleWorkspaceProvider::DEFAULT_TOKEN_URL);
        assert_eq!(oauth_config.redirect_uri, GoogleWorkspaceProvider::DEFAULT_REDIRECT_URI);
        assert_eq!(oauth_config.scopes.len(), GoogleWorkspaceProvider::DEFAULT_SCOPES.len());
        assert!(oauth_config.pkce_enabled);
        assert!(oauth_config.tenant_id.is_none());
        assert!(oauth_config.client_secret.is_none());
    }

    #[test]
    fn test_google_workspace_oauth_scopes() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let oauth_config = provider.oauth_config();

        let expected_scopes = vec![
            "https://mail.google.com/",
            "https://www.googleapis.com/auth/userinfo.email",
        ];

        assert_eq!(oauth_config.scopes, expected_scopes);
    }

    #[test]
    fn test_google_workspace_imap_config() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let imap_config = provider.imap_config("user@example.com");

        assert_eq!(imap_config.host, "imap.gmail.com");
        assert_eq!(imap_config.port, 993);
        assert_eq!(imap_config.ssl, crate::providers::SslMode::Implicit);
    }

    #[test]
    fn test_google_workspace_smtp_config() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let smtp_config = provider.smtp_config("user@example.com");

        assert_eq!(smtp_config.host, "smtp.gmail.com");
        assert_eq!(smtp_config.port, 587);
        assert_eq!(smtp_config.ssl, crate::providers::SslMode::StartTls);
    }

    #[test]
    fn test_google_workspace_capabilities() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(caps.supports_push);
        assert!(caps.supports_oauth);
        assert!(caps.supports_enterprise);
        assert!(caps.supports_labels);
        assert!(!caps.supports_folders);
        assert!(caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(50 * 1024 * 1024));
    }

    #[test]
    fn test_google_workspace_enterprise_config() {
        let provider = GoogleWorkspaceProvider::with_domain("example.com");
        let enterprise_config = provider.enterprise_config().unwrap();

        assert_eq!(enterprise_config.domain, Some("example.com".to_string()));
        assert!(enterprise_config.conditional_access);
        assert!(enterprise_config.mfa_required);
        assert!(!enterprise_config.custom_server);
        assert!(enterprise_config.custom_imap.is_none());
        assert!(enterprise_config.custom_smtp.is_none());
    }

    #[test]
    fn test_google_workspace_detect_with_domain() {
        let provider = GoogleWorkspaceProvider::with_domain("example.com");

        let runtime = tokio::runtime::Runtime::new().unwrap();

        // 测试匹配的域名
        let result = runtime.block_on(provider.detect("user@example.com"));
        assert!(result.is_ok());
        assert!(result.unwrap());

        // 测试不匹配的域名
        let result = runtime.block_on(provider.detect("user@other.com"));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_google_workspace_detect_without_domain() {
        let provider = GoogleWorkspaceProvider::with_defaults();

        let runtime = tokio::runtime::Runtime::new().unwrap();

        // 没有配置域名时，不检测任何邮箱
        let result = runtime.block_on(provider.detect("user@example.com"));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_google_workspace_supported_domains() {
        let provider = GoogleWorkspaceProvider::with_defaults();
        let domains = provider.supported_domains();

        assert_eq!(domains, Vec::<&str>::new());
    }

    #[test]
    fn test_google_workspace_provider_info() {
        let provider = GoogleWorkspaceProvider::new(None);
        let info = provider.provider_info();

        assert_eq!(info.id, "google-workspace");
        assert_eq!(info.name, "Google Workspace");
        assert_eq!(info.account_type, AccountType::Enterprise);
        assert_eq!(info.auth_types, vec![AuthType::OAuth2]);
        assert!(info.capabilities.supports_idle);
        assert!(info.capabilities.supports_push);
        assert!(info.capabilities.supports_oauth);
        assert!(info.capabilities.supports_enterprise);
        assert!(info.capabilities.supports_labels);
        assert!(!info.capabilities.supports_folders);
        assert!(info.capabilities.supports_threads);
        assert!(info.capabilities.supports_search);
        assert_eq!(info.capabilities.max_message_size, Some(50 * 1024 * 1024));
        assert_eq!(info.icon, Some("google-workspace".to_string()));
    }
}
