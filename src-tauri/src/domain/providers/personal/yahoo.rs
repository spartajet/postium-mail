use crate::domain::providers::*;

/// Yahoo 邮箱服务商
///
/// Yahoo 提供的邮箱服务，使用密码认证，
/// 支持 yahoo.com 及多国域名，垃圾邮件文件夹名为 Bulk Mail。
pub struct YahooProvider {
    info: ProviderInfo,
}

impl YahooProvider {
    /// 创建 Yahoo 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "yahoo".into(),
                name: "Yahoo Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "yahoo.com".into(),
                    "yahoo.co.jp".into(),
                    "yahoo.co.uk".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#6001D2".into()),
                icon: Some("yahoo".into()),
                sort_order: 6,
            },
        }
    }
}

/// 默认实现，等同于 [`YahooProvider::new`]
impl Default for YahooProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Yahoo 邮箱的 [`MailProvider`] 实现
impl MailProvider for YahooProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.mail.yahoo.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.mail.yahoo.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.mail.yahoo.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.mail.yahoo.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：yahoo.com、yahoo.co.jp、yahoo.co.uk
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["yahoo.com", "yahoo.co.jp", "yahoo.co.uk"]
    }

    /// Yahoo 文件夹映射，草稿文件夹名为 Draft，垃圾邮件文件夹名为 Bulk Mail
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Draft".into()],
            spam: vec!["Bulk Mail".into(), "Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec!["Archive".into()],
        }
    }
}
