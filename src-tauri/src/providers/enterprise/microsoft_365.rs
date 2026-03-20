//! Microsoft 365 企业邮件服务商
//!
//! 支持 Microsoft 365 (Office 365) 企业邮箱
//! 企业租户 OAuth、条件访问策略、MFA 支持

use async_trait::async_trait;
use super::super::{MailProvider, AccountType, AuthType, ImapServerConfig, SmtpServerConfig, ProviderCapabilities, OAuthConfig, EnterpriseConfig};

impl Microsoft365Provider {
    /// Microsoft 365 默认客户端 ID
    ///
    /// 注册地址: https://portal.azure.com/#blade/Microsoft_AAD_RegisteredApps/ApplicationsListBlade
    /// 应用类型: Public client (desktop)
    /// 授权重定向 URI: postium-mail://oauth/callback
    const DEFAULT_CLIENT_ID: &str = "67acce3b-a85a-40c1-be02-44d954282442";

    /// Microsoft 365 默认租户 ID
    ///
    /// "common" 表示允许使用个人账号和企业账号
    /// 企业部署时应替换为实际的租户 ID（如：8aef722a-1234-5678-9abc-123456789012）
    const DEFAULT_TENANT: &str = "common";

    /// Microsoft 365 默认重定向 URI
    ///
    /// 使用自定义 Deep Link 方案
    const DEFAULT_REDIRECT_URI: &str = "postium-mail://oauth/callback";

    /// Microsoft 365 默认授权端点
    const DEFAULT_AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";

    /// Microsoft 365 默认令牌端点
    const DEFAULT_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

    /// Microsoft 365 企业 OAuth Scopes
    ///
    /// - https://outlook.office.com/IMAP.AccessAsUser.All - 读取和管理邮箱中的所有邮件
    /// - https://outlook.office.com/SMTP.Send - 发送邮件
    /// - offline_access - 获取 refresh_token 以实现自动刷新
    /// - openid - 需要 openid 才能获取 JWT 格式的 access token
    const DEFAULT_SCOPES: &[&str] = &[
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://outlook.office.com/SMTP.Send",
        "offline_access",
        "openid",
    ];

    /// Microsoft 365 IMAP 服务器配置
    const IMAP_HOST: &str = "outlook.office365.com";
    const IMAP_PORT: u16 = 993;

    /// Microsoft 365 SMTP 服务器配置
    const SMTP_HOST: &str = "smtp.office365.com";
    const SMTP_PORT: u16 = 587;
}

/// Microsoft 365 企业邮件服务商
pub struct Microsoft365Provider {
    tenant_id: Option<String>,
}

impl Microsoft365Provider {
    /// 使用默认配置创建服务商
    ///
    /// 这是推荐的方式，使用预配置的 Microsoft 365 凭证。
    pub fn with_defaults() -> Self {
        Self {
            tenant_id: Some(Self::DEFAULT_TENANT.to_string()),
        }
    }

    /// 使用指定租户 ID 创建服务商
    ///
    /// # 参数
    ///
    /// * `tenant_id` - 企业租户 ID（如：8aef722a-1234-5678-9abc-123456789012）
    ///   或 "common" 表示允许多租户访问
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// // 使用 common 租户（允许个人+企业账号）
    /// let provider = Microsoft365Provider::with_defaults();
    ///
    /// // 使用特定企业租户
    /// let provider = Microsoft365Provider::with_tenant("8aef722a-1234-5678-9abc-123456789012");
    /// ```
    pub fn with_tenant<S: Into<String>>(tenant_id: S) -> Self {
        Self {
            tenant_id: Some(tenant_id.into()),
        }
    }

    pub fn new(tenant_id: Option<String>) -> Self {
        Self { tenant_id }
    }

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
            tenant_id: self.tenant_id.clone(),
        }
    }
}

#[async_trait]
impl MailProvider for Microsoft365Provider {
    fn provider_id(&self) -> &str {
        "microsoft365"
    }

    fn provider_name(&self) -> &str {
        "Microsoft 365"
    }

    fn account_type(&self) -> AccountType {
        AccountType::Enterprise
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![
            AuthType::OAuth2,
            AuthType::Password,
        ]
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "outlook.office365.com".to_string(),
            port: 993,
            ssl: crate::providers::SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.office365.com".to_string(),
            port: 587,
            ssl: crate::providers::SslMode::StartTls,
        }
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(self.oauth_config())
    }

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: self.tenant_id.clone(),
            domain: None,
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
            supports_condstore: true,
            supports_push: true,
            supports_oauth: true,
            supports_enterprise: true,
            supports_labels: false,
            supports_folders: true,
            supports_threads: true,
            supports_search: true,
            max_message_size: Some(150 * 1024 * 1024), // 150MB
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        // Microsoft 365 企业邮箱通常使用 onmicrosoft.com 域名
        // 或者需要管理员配置的自定义域名
        let domain = email.split('@').nth(1).unwrap_or("");
        Ok(domain.ends_with(".onmicrosoft.com"))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec![".onmicrosoft.com"]
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(Microsoft365Provider::new(self.tenant_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_microsoft365_with_defaults() {
        let provider = Microsoft365Provider::with_defaults();

        assert_eq!(provider.tenant_id, Some("common".to_string()));
    }

    #[test]
    fn test_microsoft365_with_tenant() {
        let provider = Microsoft365Provider::with_tenant("8aef722a-1234-5678-9abc-123456789012");

        assert_eq!(provider.tenant_id, Some("8aef722a-1234-5678-9abc-123456789012".to_string()));
    }

    #[test]
    fn test_microsoft365_with_none() {
        let provider = Microsoft365Provider::new(None);

        assert_eq!(provider.tenant_id, None);
    }

    #[test]
    fn test_microsoft365_oauth_config() {
        let provider = Microsoft365Provider::with_defaults();
        let oauth_config = provider.oauth_config();

        assert_eq!(oauth_config.client_id, Microsoft365Provider::DEFAULT_CLIENT_ID);
        assert_eq!(oauth_config.auth_url, Microsoft365Provider::DEFAULT_AUTH_URL);
        assert_eq!(oauth_config.token_url, Microsoft365Provider::DEFAULT_TOKEN_URL);
        assert_eq!(oauth_config.redirect_uri, Microsoft365Provider::DEFAULT_REDIRECT_URI);
        assert_eq!(oauth_config.scopes.len(), Microsoft365Provider::DEFAULT_SCOPES.len());
        assert!(oauth_config.pkce_enabled);
        assert_eq!(oauth_config.tenant_id, Some("common".to_string()));
        assert!(oauth_config.client_secret.is_none());
    }

    #[test]
    fn test_microsoft365_oauth_scopes() {
        let provider = Microsoft365Provider::with_defaults();
        let oauth_config = provider.oauth_config();

        let expected_scopes = vec![
            "https://outlook.office.com/IMAP.AccessAsUser.All",
            "https://outlook.office.com/SMTP.Send",
            "offline_access",
            "openid",
        ];

        assert_eq!(oauth_config.scopes, expected_scopes);
    }

    #[test]
    fn test_microsoft365_imap_config() {
        let provider = Microsoft365Provider::with_defaults();
        let imap_config = provider.imap_config("user@example.com");

        assert_eq!(imap_config.host, "outlook.office365.com");
        assert_eq!(imap_config.port, 993);
        assert_eq!(imap_config.ssl, crate::providers::SslMode::Implicit);
    }

    #[test]
    fn test_microsoft365_smtp_config() {
        let provider = Microsoft365Provider::with_defaults();
        let smtp_config = provider.smtp_config("user@example.com");

        assert_eq!(smtp_config.host, "smtp.office365.com");
        assert_eq!(smtp_config.port, 587);
        assert_eq!(smtp_config.ssl, crate::providers::SslMode::StartTls);
    }

    #[test]
    fn test_microsoft365_capabilities() {
        let provider = Microsoft365Provider::with_defaults();
        let caps = provider.capabilities();

        assert!(caps.supports_idle);
        assert!(caps.supports_condstore);
        assert!(caps.supports_push);
        assert!(caps.supports_oauth);
        assert!(caps.supports_enterprise);
        assert!(!caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(150 * 1024 * 1024));
    }

    #[test]
    fn test_microsoft365_enterprise_config() {
        let provider = Microsoft365Provider::with_defaults();
        let enterprise_config = provider.enterprise_config().unwrap();

        assert!(enterprise_config.conditional_access);
        assert!(enterprise_config.mfa_required);
        assert!(!enterprise_config.custom_server);
        assert!(enterprise_config.custom_imap.is_none());
        assert!(enterprise_config.custom_smtp.is_none());
    }

    #[test]
    fn test_microsoft365_detect_onmicrosoft() {
        let provider = Microsoft365Provider::with_defaults();

        // 需要使用运行时检测
        let runtime = tokio::runtime::Runtime::new().unwrap();

        // 测试 onmicrosoft.com 域名
        let result = runtime.block_on(provider.detect("user@contoso.onmicrosoft.com"));
        assert!(result.is_ok());
        assert!(result.unwrap());

        // 测试非 onmicrosoft.com 域名
        let result = runtime.block_on(provider.detect("user@gmail.com"));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_microsoft365_supported_domains() {
        let provider = Microsoft365Provider::with_defaults();
        let domains = provider.supported_domains();

        assert_eq!(domains, vec![".onmicrosoft.com"]);
    }

    #[test]
    fn test_microsoft365_provider_info() {
        let provider = Microsoft365Provider::with_defaults();

        assert_eq!(provider.provider_id(), "microsoft365");
        assert_eq!(provider.provider_name(), "Microsoft 365");
        assert_eq!(provider.account_type(), AccountType::Enterprise);

        let auth_types = provider.auth_types();
        assert_eq!(auth_types, vec![AuthType::OAuth2, AuthType::Password]);
    }
}
