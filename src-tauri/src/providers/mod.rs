//! 服务商适配层
//!
//! 提供不同邮件服务商的抽象接口和实现

#[allow(unused_imports)]
mod account_type;
mod config;
mod oauth_utils;
mod personal;
mod enterprise;
mod provider_pool;
mod traits;

// 重新导出核心类型
pub use account_type::{
    AccountType, EnterpriseConfig, ImapConfig, SmtpConfig
};
pub use traits::{
    MailProvider, ProviderCapabilities,
    AuthType, ImapServerConfig, SmtpServerConfig, SslMode,
    OAuthConfig
};
pub use provider_pool::ProviderPool;

// 个人邮箱（阶段2使用）
#[allow(unused_imports)]
pub use personal::{
    GmailProvider, OutlookProvider, YahooProvider, NativeProvider
};

// 企业邮箱（阶段2使用）
#[allow(unused_imports)]
pub use enterprise::{
    Microsoft365Provider, GoogleWorkspaceProvider, CustomProvider
};
