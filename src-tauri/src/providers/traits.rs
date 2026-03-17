//! 服务商 Trait 定义
//!
//! 定义邮件服务商的抽象接口

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::AccountType;
use crate::error::Result;

/// 认证类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthType {
    /// 密码认证
    Password,
    /// OAuth 2.0 认证
    OAuth2,
    /// 应用专用密码
    AppPassword,
    /// 自动检测
    Auto,
}

/// IMAP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

impl Default for ImapServerConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }
}

/// SMTP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmtpServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

impl Default for SmtpServerConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }
}

/// SSL 模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SslMode {
    /// 无加密
    None,
    /// STARTTLS 升级
    StartTls,
    /// 隐式 SSL/TLS
    Implicit,
}

/// OAuth 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub pkce_enabled: bool,
    /// 企业租户 ID（仅企业账号）
    pub tenant_id: Option<String>,
}

/// 服务商标识类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProviderAccountType {
    /// Gmail
    Gmail,
    /// Outlook / Hotmail
    Outlook,
    /// Yahoo Mail
    Yahoo,
    /// iCloud
    ICloud,
    /// 163 邮箱
    Mail163,
    /// QQ 邮箱
    QqMail,
    /// Microsoft 365
    Microsoft365,
    /// Google Workspace
    GoogleWorkspace,
    /// 自定义 IMAP/SMTP
    Custom,
}

/// 服务商能力
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderCapabilities {
    /// 是否支持 IDLE 推送
    pub supports_idle: bool,
    /// 是否支持 CONDSTORE
    pub supports_condstore: bool,
    /// 是否支持推送通知
    pub supports_push: bool,
    /// 是否支持 OAuth
    pub supports_oauth: bool,
    /// 是否支持企业特性
    pub supports_enterprise: bool,
    /// 是否支持标签（如 Gmail）
    pub supports_labels: bool,
    /// 是否支持文件夹层级
    pub supports_folders: bool,
    /// 是否支持线程
    pub supports_threads: bool,
    /// 是否支持搜索
    pub supports_search: bool,
    /// 最大附件大小（字节）
    pub max_message_size: Option<u64>,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            supports_idle: false,
            supports_condstore: false,
            supports_push: false,
            supports_oauth: false,
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(25 * 1024 * 1024), // 25MB
        }
    }
}

/// 服务商信息（用于前端展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// 服务商唯一标识
    pub id: String,
    /// 服务商显示名称
    pub name: String,
    /// 账号类型（个人/企业）
    pub account_type: AccountType,
    /// 支持的域名列表
    pub domains: Vec<String>,
    /// 支持的认证类型
    pub auth_types: Vec<AuthType>,
    /// 服务商能力
    pub capabilities: ProviderCapabilities,
    /// 图标
    pub icon: Option<String>,
}

/// 邮件服务商 Trait
#[async_trait]
pub trait MailProvider: Send + Sync {
    /// 服务商唯一标识
    fn provider_id(&self) -> &str;

    /// 服务商显示名称
    fn provider_name(&self) -> &str;

    /// 账号类型（个人/企业）
    fn account_type(&self) -> AccountType;

    /// 支持的认证类型
    fn auth_types(&self) -> Vec<AuthType>;

    /// 默认 IMAP 配置
    fn default_imap_config(&self) -> ImapServerConfig;

    /// 默认 SMTP 配置
    fn default_smtp_config(&self) -> SmtpServerConfig;

    /// OAuth 配置（如果支持）
    fn oauth_config(&self) -> Option<OAuthConfig> {
        None
    }

    /// 企业配置（仅企业账号）
    fn enterprise_config(&self) -> Option<super::EnterpriseConfig> {
        None
    }

    /// 服务商能力
    fn capabilities(&self) -> ProviderCapabilities;

    /// 根据邮箱地址检测是否为此服务商
    async fn detect(&self, email: &str) -> Result<bool>;

    /// 获取支持的域名列表
    fn supported_domains(&self) -> Vec<&'static str>;

    /// 生成 XOAUTH2 字符串
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        use base64::engine::general_purpose::STANDARD as BASE64;
        use base64::Engine;

        let auth_string = format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            email, access_token
        );
        BASE64.encode(auth_string)
    }

    /// 克隆为 Box
    fn box_clone(&self) -> Box<dyn MailProvider>;
}

/// 用于 Box<dyn MailProvider> 的 Clone 实现
impl Clone for Box<dyn MailProvider> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imap_config_default() {
        let config = ImapServerConfig::default();
        assert_eq!(config.port, 993);
        assert_eq!(config.ssl, SslMode::Implicit);
    }

    #[test]
    fn test_smtp_config_default() {
        let config = SmtpServerConfig::default();
        assert_eq!(config.port, 587);
        assert_eq!(config.ssl, SslMode::StartTls);
    }

    #[test]
    fn test_provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.supports_idle);
        assert!(caps.supports_folders);
        assert!(caps.supports_search);
        assert_eq!(caps.max_message_size, Some(25 * 1024 * 1024));
    }

    #[test]
    fn test_generate_xoauth2() {
        struct TestProvider;
        #[async_trait]
        impl MailProvider for TestProvider {
            fn provider_id(&self) -> &str { "test" }
            fn provider_name(&self) -> &str { "Test" }
            fn account_type(&self) -> AccountType { AccountType::Personal }
            fn auth_types(&self) -> Vec<AuthType> { vec![AuthType::Password] }
            fn default_imap_config(&self) -> ImapServerConfig { Default::default() }
            fn default_smtp_config(&self) -> SmtpServerConfig { Default::default() }
            fn capabilities(&self) -> ProviderCapabilities { Default::default() }
            async fn detect(&self, _email: &str) -> Result<bool> { Ok(true) }
            fn supported_domains(&self) -> Vec<&'static str> { vec!["example.com"] }
            fn box_clone(&self) -> Box<dyn MailProvider> {
                Box::new(TestProvider)
            }
        }

        let provider = TestProvider;
        let xoauth2 = provider.generate_xoauth2("user@example.com", "token123");

        // 验证 XOAUTH2 字符串不为空且是 base64 编码
        assert!(!xoauth2.is_empty());
        // base64 编码应该只包含这些字符
        assert!(xoauth2.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));
    }
}
