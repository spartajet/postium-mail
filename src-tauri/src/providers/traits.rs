//! 邮件服务商 Trait 定义
//!
//! 定义所有邮件服务商必须实现的抽象接口。
//!
//! # 核心接口
//!
//! ## MailProvider Trait
//!
//! [`MailProvider`] 是所有邮件服务商必须实现的核心接口。
//!
//! ### 必需方法
//!
//! - [`provider_id()`][]: 服务商唯一标识
//! - [`provider_name()`][]: 服务商显示名称
//! - [`account_type()`][]: 账号类型（个人/企业）
//! - [`auth_types()`][]: 支持的认证类型列表
//! - [`default_imap_config()`][]: 默认 IMAP 配置
//! - [`default_smtp_config()`][]: 默认 SMTP 配置
//! - [`capabilities()`][]: 服务商能力
//! - [`detect()`][]: 检测邮箱地址是否属于此服务商
//! - [`supported_domains()`][]: 支持的域名列表
//!
//! ### 可选方法
//!
//! - [`oauth_config()`][]: OAuth 配置（如果支持 OAuth）
//! - [`enterprise_config()`][]: 企业配置（仅企业账号）
//!
//! # 数据类型
//!
//! ## 认证相关
//!
//! - [`AuthType`][]: 认证类型枚举
//! - [`OAuthConfig`][]: OAuth 配置结构体
//!
//! ## 服务器配置
//!
//! - [`ImapServerConfig`][]: IMAP 服务器配置
//! - [`SmtpServerConfig`][]: SMTP 服务器配置
//! - [`SslMode`][]: SSL 模式枚举
//!
//! ## 服务商能力
//!
//! - [`ProviderCapabilities`][]: 服务商能力描述
//! - [`ProviderInfo`][]: 服务商信息（用于前端展示）
//!
//! # 使用示例
//!
//! ## 实现自定义服务商
//!
//! ```rust
//! use crate::providers::MailProvider;
//! use async_trait::async_trait;
//!
//! pub struct MyProvider {
//!     email: String,
//! }
//!
//! #[async_trait]
//! impl MailProvider for MyProvider {
//!     fn provider_id(&self) -> &str {
//!         "my-provider"
//!     }
//!
//!     fn provider_name(&self) -> &str {
//!         "My Provider"
//!     }
//!
//!     fn account_type(&self) -> crate::providers::AccountType {
//!         crate::providers::AccountType::Personal
//!     }
//!
//!     fn auth_types(&self) -> Vec<crate::providers::AuthType> {
//!         vec![crate::providers::AuthType::Password]
//!     }
//!
//!     fn default_imap_config(&self) -> crate::providers::ImapServerConfig {
//!         // ...
//! # }
//!     # fn default_smtp_config(&self) -> crate::providers::SmtpServerConfig { ... }
//!     # fn capabilities(&self) -> crate::providers::ProviderCapabilities { ... }
//!     # async fn detect(&self, email: &str) -> crate::error::Result<bool> { ... }
//!     # fn supported_domains(&self) -> Vec<&'static str> { ... }
//!     # fn box_clone(&self) -> Box<dyn MailProvider> { ... }
//! }
//! ```
//!
//! ## 获取服务器配置
//!
//! ```rust,no_run
//! # async fn example() -> anyhow::Result<()> {
//! # let provider: Box<dyn crate::providers::MailProvider> = todo!();
//! // 获取 IMAP 配置
//! let imap = provider.default_imap_config();
//! println!("IMAP: {}:{}", imap.host, imap.port);
//!
//! // 获取 SMTP 配置
//! let smtp = provider.default_smtp_config();
//! println!("SMTP: {}:{}", smtp.host, smtp.port);
//!
//! // 检查能力
//! let caps = provider.capabilities();
//! if caps.supports_oauth {
//!     println!("支持 OAuth");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## 生成 XOAUTH2 字符串
//!
//! ```rust,no_run
//! # let provider: Box<dyn crate::providers::MailProvider> = todo!();
//! let xoauth2 = provider.generate_xoauth2(
//!     "user@example.com",
//!     "ya29.a0AfH6..."
//! );
//! // 用于 SASL XOAUTH2 认证
//! ```
//!
//! # SSL 模式说明
//!
//! - **None**: 无加密（不推荐）
//! - **StartTls**: STARTTLS 升级（端口 587，推荐）
//! - **Implicit**: 隐式 SSL/TLS（端口 465）
//!
//! # 服务商能力说明
//!
//! [`ProviderCapabilities`] 描述服务商支持的 IMAP 扩展和功能：
//!
//! - `supports_idle`: 支持 IDLE 实时推送
//! - `supports_condstore`: 支持 CONDSTORE 增量同步
//! - `supports_push`: 支持推送通知
//! - `supports_oauth`: 支持 OAuth2 认证
//! - `supports_enterprise`: 支持企业特性
//! - `supports_labels`: 支持标签（如 Gmail 标签）
//! - `supports_threads`: 支持邮件线程
//! - `supports_search`: 支持服务器搜索
//! - `max_message_size`: 最大邮件大小

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

impl OAuthConfig {
    /// 从环境变量加载指定服务商的配置
    pub fn from_env_for_provider(provider_id: &str) -> Result<Self> {
        let config = crate::config::load_oauth_config_for_provider(provider_id).map_err(|e| {
            crate::error::MailError::Internal(format!("加载 OAuth 配置失败: {}", e))
        })?;

        // 将 config::OAuthConfig 转换为 traits::OAuthConfig
        Ok(Self {
            client_id: config.client_id,
            client_secret: None, // 公共客户端不需要 secret
            auth_url: config.auth_url,
            token_url: config.token_url,
            redirect_uri: config.redirect_uri,
            scopes: config.scopes,
            pkce_enabled: true, // 默认启用 PKCE
            tenant_id: if config.tenant.is_empty() {
                None
            } else {
                Some(config.tenant)
            },
        })
    }

    /// 验证配置是否有效
    pub fn validate(&self) -> Result<()> {
        use crate::error::{AuthError, MailError};

        // 检查 client_id
        if self.client_id.is_empty() {
            return Err(MailError::Authentication(AuthError::InvalidCredentials));
        }

        // 检查 redirect_uri
        if self.redirect_uri.is_empty() {
            return Err(MailError::Authentication(AuthError::InvalidCredentials));
        }

        // 检查 auth_url 格式
        if !self.auth_url.starts_with("https://") {
            return Err(MailError::Authentication(AuthError::InvalidCredentials));
        }

        // 检查 token_url 格式
        if !self.token_url.starts_with("https://") {
            return Err(MailError::Authentication(AuthError::InvalidCredentials));
        }

        // 检查 scopes
        if self.scopes.is_empty() {
            return Err(MailError::Authentication(AuthError::InvalidCredentials));
        }

        Ok(())
    }
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

    /// 获取 IMAP 配置（根据邮箱地址返回对应域名服务器配置）
    fn imap_config(&self, email: &str) -> ImapServerConfig;

    /// 获取 SMTP 配置（根据邮箱地址返回对应域名服务器配置）
    fn smtp_config(&self, email: &str) -> SmtpServerConfig;

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

        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
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
            fn provider_id(&self) -> &str {
                "test"
            }
            fn provider_name(&self) -> &str {
                "Test"
            }
            fn account_type(&self) -> AccountType {
                AccountType::Personal
            }
            fn auth_types(&self) -> Vec<AuthType> {
                vec![AuthType::Password]
            }
            fn imap_config(&self, _email: &str) -> ImapServerConfig {
                Default::default()
            }
            fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
                Default::default()
            }
            fn capabilities(&self) -> ProviderCapabilities {
                Default::default()
            }
            async fn detect(&self, _email: &str) -> Result<bool> {
                Ok(true)
            }
            fn supported_domains(&self) -> Vec<&'static str> {
                vec!["example.com"]
            }
            fn box_clone(&self) -> Box<dyn MailProvider> {
                Box::new(TestProvider)
            }
        }

        let provider = TestProvider;
        let xoauth2 = provider.generate_xoauth2("user@example.com", "token123");

        // 验证 XOAUTH2 字符串不为空且是 base64 编码
        assert!(!xoauth2.is_empty());
        // base64 编码应该只包含这些字符
        assert!(xoauth2
            .chars()
            .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='));
    }
}
