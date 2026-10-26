use crate::domain::providers::*;

/// iCloud 邮箱服务商
///
/// Apple 提供的邮箱服务，使用密码认证，
/// 支持 icloud.com、me.com、mac.com 域名。
pub struct ICloudProvider {
    info: ProviderInfo,
}

impl ICloudProvider {
    /// 创建 iCloud 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "icloud".into(),
                name: "iCloud Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["icloud.com".into(), "me.com".into(), "mac.com".into()],
                auth_type: AuthType::Password,
                color: Some("#3693F5".into()),
                icon: Some("icloud".into()),
                sort_order: 5,
            },
        }
    }
}

/// 默认实现，等同于 [`ICloudProvider::new`]
impl Default for ICloudProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// iCloud 邮箱的 [`MailProvider`] 实现
impl MailProvider for ICloudProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.mail.me.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.me.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.mail.me.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.me.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 支持的域名：icloud.com、me.com、mac.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["icloud.com", "me.com", "mac.com"]
    }

    /// iCloud 文件夹映射，使用英文文件夹名称
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent Messages".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Junk".into()],
            trash: vec!["Deleted Messages".into()],
            archive: vec!["Archive".into()],
        }
    }
}
