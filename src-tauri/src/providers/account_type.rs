//! 账号类型定义
//!
//! 区分个人邮箱和企业邮箱

use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

// 重新导出 SslMode，避免重复定义
pub use crate::providers::traits::SslMode;

/// 账号类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy, Default)]
pub enum AccountType {
    /// 个人邮件账号
    #[default]
    Personal,
    /// 企业邮件账号
    Enterprise,
}

impl Display for AccountType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Personal => write!(f, "personal"),
            Self::Enterprise => write!(f, "enterprise"),
        }
    }
}

/// 企业配置（仅企业账号）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnterpriseConfig {
    /// Azure AD 租户 ID 或 Google Workspace 域
    pub tenant_id: Option<String>,
    /// 企业域名
    pub domain: Option<String>,
    /// 是否启用条件访问策略
    pub conditional_access: bool,
    /// 是否强制 MFA
    pub mfa_required: bool,
    /// 是否使用自定义服务器
    pub custom_server: bool,
    /// 自定义 IMAP 服务器（如果使用）
    pub custom_imap: Option<ImapConfig>,
    /// 自定义 SMTP 服务器（如果使用）
    pub custom_smtp: Option<SmtpConfig>,
}

/// IMAP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

impl Default for ImapConfig {
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
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_type_default() {
        let account_type = AccountType::default();
        assert_eq!(account_type, AccountType::Personal);
    }

    #[test]
    fn test_account_type_display() {
        assert_eq!(AccountType::Personal.to_string(), "personal");
        assert_eq!(AccountType::Enterprise.to_string(), "enterprise");
    }

    #[test]
    fn test_enterprise_config_default() {
        let config = EnterpriseConfig::default();
        assert!(!config.conditional_access);
        assert!(!config.mfa_required);
        assert!(!config.custom_server);
    }

    #[test]
    fn test_imap_config_default() {
        let config = ImapConfig::default();
        assert_eq!(config.port, 993);
        assert_eq!(config.ssl, SslMode::Implicit);
    }

    #[test]
    fn test_smtp_config_default() {
        let config = SmtpConfig::default();
        assert_eq!(config.port, 587);
        assert_eq!(config.ssl, SslMode::StartTls);
    }
}
