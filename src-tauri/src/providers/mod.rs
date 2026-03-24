//! 邮件服务商适配层
//!
//! 提供统一抽象接口，屏蔽不同邮件服务商的差异。
//!
//! # 核心概念
//!
//! ## MailProvider Trait
//!
//! 定义了所有邮件服务商必须实现的接口：
//!
//! - **配置获取**: IMAP/SMTP 服务器地址、端口、SSL 配置
//! - **认证支持**: 支持的认证方式（密码、OAuth2）
//! - **功能能力**: 是否支持 IDLE 等扩展
//!
//! ## 个人邮箱 vs 企业邮箱
//!
//! ### 个人邮箱 (Personal)
//!
//! 面向个人用户的主流邮件服务：
//!
//! | 服务商 | 代码 | 特点 |
//! |--------|------|------|
//! | Gmail | GmailProvider | OAuth2 支持 |
//! | Outlook | OutlookProvider | OAuth2 支持 |
//! | iCloud | ICloudProvider | 应用专用密码 |
//! | QQ 邮箱 | QqMailProvider | 应用专用密码 |
//! | 163 邮箱 | Mail163Provider | 应用专用密码 |
//! | Yahoo | YahooProvider | 应用专用密码 |
//! | 本地 IMAP | NativeProvider | 自定义服务器 |
//!
//! ### 企业邮箱 (Enterprise)
//!
//! 面向企业组织的生产力服务：
//!
//! | 服务商 | 代码 | 特点 |
//! |--------|------|------|
//! | Microsoft 365 | Microsoft365Provider | OAuth2/SAML |
//! | Google Workspace | GoogleWorkspaceProvider | OAuth2/SAML |
//! | 自定义 | CustomProvider | 自定义配置 |
//!
//! # 架构设计
//!
//! ```
//! ┌─────────────────────────────────────────┐
//! │              应用层                      │
//! │  (账号管理、邮件同步、UI 显示)           │
//! └────────────────┬────────────────────────┘
//!                  │
//! ┌────────────────▼────────────────────────┐
//! │          ProviderPool                   │
//! │  (服务商池、路由、统一入口)              │
//! └────────────────┬────────────────────────┘
//!                  │
//! ┌────────────────▼────────────────────────┐
//! │           MailProvider Trait            │
//! │  (get_imap_config, get_smtp_config, ...)  │
//! └──────┬──────────────┬───────────────────┘
//!        │              │
//!   ┌────▼────┐   ┌────▼────┐
//!   │ Personal│   │Enterprise│
//!   │  个人   │   │  企业    │
//!   └────┬────┘   └────┬────┘
//!        │              │
//!   ┌────▼────────────▼────┐
//!   │ Gmail, Outlook, ...  │
//!   │ M365, GWorkspace, ...│
//!   └─────────────────────┘
//! ```
//!
//! # 使用示例
//!
//! ## 基本用法
//!
//! ```rust,no_run
//! use crate::providers::{MailProvider, GmailProvider};
//!
//! let provider = GmailProvider::new("user@gmail.com");
//!
//! // 获取 IMAP 配置
//! let imap_config = provider.get_imap_config()?;
//! println!("IMAP: {}:{}", imap_config.host, imap_config.port);
//!
//! // 获取 SMTP 配置
//! let smtp_config = provider.get_smtp_config()?;
//! println!("SMTP: {}:{}", smtp_config.host, smtp_config.port);
//!
//! // 检查能力
//! let caps = provider.get_capabilities();
//! if caps.supports_idle {
//!     println!("支持 IDLE");
//! }
//! ```
//!
//! ## 使用 ProviderPool
//!
//! ```rust,no_run
//! use crate::providers::{ProviderPool, AccountType};
//!
//! let pool = ProviderPool::new();
//!
//! // 注册提供商
//! pool.register(AccountType::Gmail);
//! pool.register(AccountType::Outlook);
//!
//! // 获取提供商
//! let provider = pool.get_provider(AccountType::Gmail, "user@gmail.com")?;
//! ```
//!
//! # 实现新服务商
//!
//! ## 步骤 1: 实现 MailProvider Trait
//!
//! ```rust
//! use crate::providers::MailProvider;
//!
//! pub struct MyCustomProvider {
//!     email: String,
//! }
//!
//! impl MailProvider for MyCustomProvider {
//!     fn get_imap_config(&self) -> ProviderResult<ImapServerConfig> {
//!         Ok(ImapServerConfig {
//!             host: "imap.example.com".to_string(),
//!             port: 993,
//!             ssl: SslMode::Implicit,
//!             // ...
//!         })
//!     }
//!     // ... 其他方法
//! }
//! }
//! ```
//!
//! ## 步骤 2: 注册到 ProviderPool
//!
//! 在 `ProviderPool::new()` 中注册：
//!
//! ```rust
//! providers.insert(AccountType::MyCustom, Box::new(|email| {
//!     Ok(Box::new(MyCustomProvider::new(email)))
//! }));
//! ```
//!
//! # OAuth2 认证流程
//!
//! 本模块还提供 OAuth2 相关工具：
//!
//! - [`generate_xoauth2_string()`][]: 生成 XOAUTH2 认证字符串
//! - [`validate_access_token()`][]: 验证访问令牌有效性
//! - [`OAuthTokenResponse`][]: OAuth 令牌响应结构
//! - [`PkceVerifierStore`][]: PKCE 验证器存储
//!
//! # 模块结构
//!
//! - [`traits`][] - 核心接口定义
//! - [`account_type`][] - 账号类型枚举
//! - [`config`][] - 配置结构体
//! - [`oauth_utils`][] - OAuth 工具函数
//! - [`provider_pool`][] - 服务商池
//! - [`personal`][] - 个人邮箱实现
//! - [`enterprise`][] - 企业邮箱实现
//!
//! # 注意事项
//!
//! - 服务商特定配置（如客户端 ID）需要在对应模块中配置
//! - OAuth2 令牌需要定期刷新
//! - 某些服务商可能不支持所有功能
//! - 企业邮箱可能需要额外的管理员配置

#[allow(unused_imports, deprecated)]
mod account_type;
mod config;
mod enterprise;
mod oauth_utils;
mod personal;
mod provider_pool;
mod traits;

// 重新导出核心类型
pub use account_type::{AccountType, EnterpriseConfig, ImapConfig, SmtpConfig};
pub use traits::{
    AuthType, ImapServerConfig, MailProvider, OAuthConfig, ProviderCapabilities, SmtpServerConfig,
    SslMode, StandardFolder,
};

// OAuth 工具导出（供 auth 模块使用）
pub use oauth_utils::{
    generate_xoauth2_string, validate_access_token, OAuthTokenResponse, PkceVerifierStore,
};
pub use provider_pool::ProviderPool;

// 个人邮箱（阶段2使用）
#[allow(unused_imports)]
pub use personal::{
    GmailProvider, ICloudProvider, Mail163Provider, OutlookProvider, QqMailProvider, YahooProvider,
};

// 企业邮箱（阶段2使用）
#[allow(unused_imports)]
pub use enterprise::{CustomProvider, GoogleWorkspaceProvider, Microsoft365Provider};
