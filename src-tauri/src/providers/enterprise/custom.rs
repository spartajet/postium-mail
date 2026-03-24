//! 自定义企业邮件服务商
//!
//! 支持自定义 IMAP/SMTP 服务器的企业邮箱
//! 灵活配置服务器地址、端口、SSL 模式等参数

use super::super::{
    AccountType, AuthType, EnterpriseConfig, ImapServerConfig, MailProvider, ProviderCapabilities,
    SmtpServerConfig, SslMode,
};
use async_trait::async_trait;

impl CustomProvider {
    /// 默认 IMAP 端口
    const DEFAULT_IMAP_PORT: u16 = 993;

    /// 默认 SMTP 端口
    const DEFAULT_SMTP_PORT: u16 = 587;

    /// 默认 IMAP SSL 模式
    const DEFAULT_IMAP_SSL: SslMode = SslMode::Implicit;

    /// 默认 SMTP SSL 模式
    const DEFAULT_SMTP_SSL: SslMode = SslMode::StartTls;
}

/// 自定义企业邮件服务商
pub struct CustomProvider {
    name: String,
    imap_host: String,
    imap_port: u16,
    imap_ssl: SslMode,
    smtp_host: String,
    smtp_port: u16,
    smtp_ssl: SslMode,
}

impl CustomProvider {
    /// 使用服务器地址创建服务商（使用默认端口和 SSL 模式）
    ///
    /// # 参数
    ///
    /// * `name` - 服务商名称（用于显示）
    /// * `imap_host` - IMAP 服务器地址
    /// * `smtp_host` - SMTP 服务器地址
    ///
    /// # 示例
    ///
    /// ```rust,ignore
    /// let provider = CustomProvider::with_servers(
    ///     "My Company Mail",
    ///     "mail.example.com",
    ///     "smtp.example.com"
    /// );
    /// ```
    pub fn with_servers<S: Into<String>>(
        name: S,
        imap_host: S,
        smtp_host: S,
    ) -> Self {
        Self {
            name: name.into(),
            imap_host: imap_host.into(),
            imap_port: Self::DEFAULT_IMAP_PORT,
            imap_ssl: Self::DEFAULT_IMAP_SSL,
            smtp_host: smtp_host.into(),
            smtp_port: Self::DEFAULT_SMTP_PORT,
            smtp_ssl: Self::DEFAULT_SMTP_SSL,
        }
    }

    /// 创建完全自定义的服务商配置
    ///
    /// # 参数
    ///
    /// * `name` - 服务商名称
    /// * `imap_host` - IMAP 服务器地址
    /// * `imap_port` - IMAP 端口
    /// * `imap_ssl` - IMAP SSL 模式
    /// * `smtp_host` - SMTP 服务器地址
    /// * `smtp_port` - SMTP 端口
    /// * `smtp_ssl` - SMTP SSL 模式
    pub fn new(
        name: String,
        imap_host: String,
        imap_port: u16,
        imap_ssl: SslMode,
        smtp_host: String,
        smtp_port: u16,
        smtp_ssl: SslMode,
    ) -> Self {
        Self {
            name,
            imap_host,
            imap_port,
            imap_ssl,
            smtp_host,
            smtp_port,
            smtp_ssl,
        }
    }

    /// 简化的构造方法（使用默认 SSL 模式）
    ///
    /// # 参数
    ///
    /// * `name` - 服务商名称
    /// * `imap_host` - IMAP 服务器地址
    /// * `imap_port` - IMAP 端口
    /// * `smtp_host` - SMTP 服务器地址
    /// * `smtp_port` - SMTP 端口
    pub fn with_defaults(
        name: String,
        imap_host: String,
        imap_port: u16,
        smtp_host: String,
        smtp_port: u16,
    ) -> Self {
        Self {
            name,
            imap_host,
            imap_port,
            imap_ssl: Self::DEFAULT_IMAP_SSL,
            smtp_host,
            smtp_port,
            smtp_ssl: Self::DEFAULT_SMTP_SSL,
        }
    }
}

#[async_trait]
impl MailProvider for CustomProvider {
    fn provider_id(&self) -> &str {
        "custom"
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    fn account_type(&self) -> AccountType {
        AccountType::Enterprise
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password, AuthType::OAuth2]
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: self.imap_host.clone(),
            port: self.imap_port,
            ssl: self.imap_ssl.clone(),
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: self.smtp_host.clone(),
            port: self.smtp_port,
            ssl: self.smtp_ssl.clone(),
        }
    }

    fn enterprise_config(&self) -> Option<EnterpriseConfig> {
        Some(EnterpriseConfig {
            tenant_id: None,
            domain: None,
            conditional_access: false,
            mfa_required: false,
            custom_server: true,
            custom_imap: Some(crate::providers::ImapConfig {
                host: self.imap_host.clone(),
                port: self.imap_port,
                ssl: self.imap_ssl.clone(),
            }),
            custom_smtp: Some(crate::providers::SmtpConfig {
                host: self.smtp_host.clone(),
                port: self.smtp_port,
                ssl: self.smtp_ssl.clone(),
            }),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: false,
            supports_push: false,
            supports_oauth: false,
            supports_enterprise: true,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(25 * 1024 * 1024),
        }
    }

    async fn detect(&self, _email: &str) -> crate::error::Result<bool> {
        Ok(false) // 自定义服务器不自动检测
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        Vec::new()
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(CustomProvider::new(
            self.name.clone(),
            self.imap_host.clone(),
            self.imap_port,
            self.imap_ssl.clone(),
            self.smtp_host.clone(),
            self.smtp_port,
            self.smtp_ssl.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_provider_with_servers() {
        let provider = CustomProvider::with_servers(
            "Test Company",
            "imap.test.com",
            "smtp.test.com"
        );

        assert_eq!(provider.name, "Test Company");
        assert_eq!(provider.imap_host, "imap.test.com");
        assert_eq!(provider.smtp_host, "smtp.test.com");
        assert_eq!(provider.imap_port, 993);
        assert_eq!(provider.smtp_port, 587);
        assert_eq!(provider.imap_ssl, SslMode::Implicit);
        assert_eq!(provider.smtp_ssl, SslMode::StartTls);
    }

    #[test]
    fn test_custom_provider_with_defaults() {
        let provider = CustomProvider::with_defaults(
            "Test Company".to_string(),
            "imap.test.com".to_string(),
            993,
            "smtp.test.com".to_string(),
            587,
        );

        assert_eq!(provider.name, "Test Company");
        assert_eq!(provider.imap_host, "imap.test.com");
        assert_eq!(provider.smtp_host, "smtp.test.com");
        assert_eq!(provider.imap_port, 993);
        assert_eq!(provider.smtp_port, 587);
        assert_eq!(provider.imap_ssl, SslMode::Implicit);
        assert_eq!(provider.smtp_ssl, SslMode::StartTls);
    }

    #[test]
    fn test_custom_provider_new() {
        let provider = CustomProvider::new(
            "Test Company".to_string(),
            "imap.test.com".to_string(),
            143,
            SslMode::None,
            "smtp.test.com".to_string(),
            25,
            SslMode::None,
        );

        assert_eq!(provider.name, "Test Company");
        assert_eq!(provider.imap_port, 143);
        assert_eq!(provider.imap_ssl, SslMode::None);
        assert_eq!(provider.smtp_port, 25);
        assert_eq!(provider.smtp_ssl, SslMode::None);
    }

    #[test]
    fn test_custom_provider_imap_config() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );
        let imap_config = provider.imap_config("user@example.com");

        assert_eq!(imap_config.host, "imap.test.com");
        assert_eq!(imap_config.port, 993);
        assert_eq!(imap_config.ssl, SslMode::Implicit);
    }

    #[test]
    fn test_custom_provider_smtp_config() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );
        let smtp_config = provider.smtp_config("user@example.com");

        assert_eq!(smtp_config.host, "smtp.test.com");
        assert_eq!(smtp_config.port, 587);
        assert_eq!(smtp_config.ssl, SslMode::StartTls);
    }

    #[test]
    fn test_custom_provider_capabilities() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );
        let caps = provider.capabilities();

        assert!(!caps.supports_idle);
        assert!(!caps.supports_push);
        assert!(!caps.supports_oauth);
        assert!(caps.supports_enterprise);
        assert!(!caps.supports_labels);
        assert!(caps.supports_folders);
        assert!(!caps.supports_threads);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(25 * 1024 * 1024));
    }

    #[test]
    fn test_custom_provider_enterprise_config() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );
        let enterprise_config = provider.enterprise_config().unwrap();

        assert!(enterprise_config.custom_server);
        assert!(enterprise_config.custom_imap.is_some());
        assert!(enterprise_config.custom_smtp.is_some());
        assert!(!enterprise_config.conditional_access);
        assert!(!enterprise_config.mfa_required);

        let imap_config = enterprise_config.custom_imap.unwrap();
        assert_eq!(imap_config.host, "imap.test.com");
        assert_eq!(imap_config.port, 993);
        assert_eq!(imap_config.ssl, SslMode::Implicit.clone());

        let smtp_config = enterprise_config.custom_smtp.unwrap();
        assert_eq!(smtp_config.host, "smtp.test.com");
        assert_eq!(smtp_config.port, 587);
        assert_eq!(smtp_config.ssl, SslMode::StartTls.clone());
    }

    #[test]
    fn test_custom_provider_detect() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();

        // 自定义服务商不自动检测
        let result = runtime.block_on(provider.detect("user@test.com"));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_custom_provider_supported_domains() {
        let provider = CustomProvider::with_servers(
            "Test",
            "imap.test.com",
            "smtp.test.com"
        );
        let domains = provider.supported_domains();

        assert_eq!(domains, Vec::<&str>::new());
    }

    #[test]
    fn test_custom_provider_box_clone() {
        let provider = CustomProvider::new(
            "Test".to_string(),
            "imap.test.com".to_string(),
            143,
            SslMode::None,
            "smtp.test.com".to_string(),
            25,
            SslMode::None,
        );

        let cloned = provider.box_clone();

        assert_eq!(cloned.provider_id(), "custom");
        assert_eq!(cloned.provider_name(), "Test");
        assert_eq!(cloned.account_type(), AccountType::Enterprise);
    }

    #[test]
    fn test_custom_provider_info() {
        let provider = CustomProvider::with_servers(
            "My Company Mail",
            "imap.company.com",
            "smtp.company.com"
        );

        assert_eq!(provider.provider_id(), "custom");
        assert_eq!(provider.provider_name(), "My Company Mail");
        assert_eq!(provider.account_type(), AccountType::Enterprise);

        let auth_types = provider.auth_types();
        assert_eq!(auth_types, vec![AuthType::Password, AuthType::OAuth2]);
    }

    #[test]
    fn test_custom_provider_default_ssl_modes() {
        // 测试默认 SSL 模式常量
        assert_eq!(CustomProvider::DEFAULT_IMAP_SSL, SslMode::Implicit);
        assert_eq!(CustomProvider::DEFAULT_SMTP_SSL, SslMode::StartTls);
        assert_eq!(CustomProvider::DEFAULT_IMAP_PORT, 993);
        assert_eq!(CustomProvider::DEFAULT_SMTP_PORT, 587);
    }
}
