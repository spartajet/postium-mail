//! 国内邮件服务商（已废弃）

#![allow(deprecated)]
//!
//! 此模块已被拆分为独立的服务商实现：
//! - [`Mail163Provider`] - 支持 163.com、126.com、yeah.net
//! - [`QqMailProvider`] - 支持 qq.com、foxmail.com
//! - [`ICloudProvider`] - 支持 icloud.com、me.com、mac.com
//!
//! # 迁移指南
//!
//! ## 1. 替换导入
//!
//! ```rust,ignore
//! // 旧代码
//! use crate::providers::personal::NativeProvider;
//!
//! // 新代码
//! use crate::providers::personal::{Mail163Provider, QqMailProvider, ICloudProvider};
//! ```
//!
//! ## 2. 替换服务商创建
//!
//! ```rust,ignore
//! // 旧代码
//! let provider = NativeProvider::Mail163;
//!
//! // 新代码
//! let provider = Mail163Provider;
//! ```
//!
//! ## 3. 替换邮箱检测
//!
//! ```rust,ignore
//! // 旧代码
//! let provider = NativeProvider::from_email("user@163.com");
//!
//! // 新代码
//! let pool = ProviderPool::with_defaults();
//! let provider = pool.detect_provider("user@163.com").await?;
//! ```
//!
//! # 为什么废弃？
//!
//! - **架构一致性**：与其他服务商（Gmail、Outlook）保持一致的实现模式
//! - **代码可维护性**：每个服务商独立文件，更易于维护和扩展
//! - **类型安全**：独立结构体提供更好的类型安全性
//! - **测试隔离**：每个服务商可以独立测试，互不影响

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, ProviderCapabilities, SmtpServerConfig,
};
use async_trait::async_trait;

/// 国内邮件服务商（已废弃）
///
/// 请使用以下替代品：
/// - [`Mail163Provider`] - 网易邮箱
/// - [`QqMailProvider`] - QQ 邮箱
/// - [`ICloudProvider`] - iCloud
///
/// # 迁移示例
///
/// ```rust,ignore
/// // 旧代码
/// let provider = NativeProvider::Mail163;
///
/// // 新代码
/// let provider = Mail163Provider;
/// ```
#[deprecated(
    since = "0.2.0",
    note = "请使用 Mail163Provider、QqMailProvider 或 ICloudProvider 替代"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeProvider {
    Mail163,
    QqMail,
    ICloud,
}

impl NativeProvider {
    pub fn from_email(email: &str) -> Option<Self> {
        let domain = email.split('@').nth(1)?;
        match domain {
            "163.com" | "126.com" | "yeah.net" => Some(Self::Mail163),
            "qq.com" | "foxmail.com" => Some(Self::QqMail),
            "icloud.com" | "me.com" | "mac.com" => Some(Self::ICloud),
            _ => None,
        }
    }
}

#[async_trait]
impl MailProvider for NativeProvider {
    fn provider_id(&self) -> &str {
        match self {
            Self::Mail163 => "163",
            Self::QqMail => "qq",
            Self::ICloud => "icloud",
        }
    }

    fn provider_name(&self) -> &str {
        match self {
            Self::Mail163 => "网易邮箱",
            Self::QqMail => "QQ邮箱",
            Self::ICloud => "iCloud",
        }
    }

    fn account_type(&self) -> AccountType {
        AccountType::Personal
    }

    fn auth_types(&self) -> Vec<AuthType> {
        vec![AuthType::Password]
    }

    fn default_imap_config(&self) -> ImapServerConfig {
        match self {
            Self::Mail163 => ImapServerConfig {
                host: "imap.163.com".to_string(),
                port: 993,
                ssl: crate::providers::SslMode::Implicit,
            },
            Self::QqMail => ImapServerConfig {
                host: "imap.qq.com".to_string(),
                port: 993,
                ssl: crate::providers::SslMode::Implicit,
            },
            Self::ICloud => ImapServerConfig {
                host: "imap.mail.me.com".to_string(),
                port: 993,
                ssl: crate::providers::SslMode::Implicit,
            },
        }
    }

    fn default_smtp_config(&self) -> SmtpServerConfig {
        match self {
            Self::Mail163 => SmtpServerConfig {
                host: "smtp.163.com".to_string(),
                port: 465,
                ssl: crate::providers::SslMode::Implicit,
            },
            Self::QqMail => SmtpServerConfig {
                host: "smtp.qq.com".to_string(),
                port: 587,
                ssl: crate::providers::SslMode::StartTls,
            },
            Self::ICloud => SmtpServerConfig {
                host: "smtp.mail.me.com".to_string(),
                port: 587,
                ssl: crate::providers::SslMode::StartTls,
            },
        }
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_idle: true,
            supports_condstore: false,
            supports_push: false,
            supports_oauth: false,
            supports_enterprise: false,
            supports_labels: false,
            supports_folders: true,
            supports_threads: false,
            supports_search: true,
            max_message_size: Some(50 * 1024 * 1024),
        }
    }

    async fn detect(&self, email: &str) -> crate::error::Result<bool> {
        Ok(Self::from_email(email).as_ref() == Some(self))
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        match self {
            Self::Mail163 => vec!["163.com", "126.com", "yeah.net"],
            Self::QqMail => vec!["qq.com", "foxmail.com"],
            Self::ICloud => vec!["icloud.com", "me.com", "mac.com"],
        }
    }

    fn box_clone(&self) -> Box<dyn MailProvider> {
        Box::new(match self {
            Self::Mail163 => Self::Mail163,
            Self::QqMail => Self::QqMail,
            Self::ICloud => Self::ICloud,
        })
    }
}
