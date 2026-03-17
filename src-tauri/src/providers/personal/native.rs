//! 国内邮件服务商（163、QQ、iCloud）

use super::super::{
    AccountType, AuthType, ImapServerConfig, MailProvider, ProviderCapabilities, SmtpServerConfig,
};
use async_trait::async_trait;

/// 国内邮件服务商
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
