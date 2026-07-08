use crate::domain::providers::*;

/// Zoho 邮箱服务商
///
/// Zoho 提供的邮箱服务，使用密码认证，
/// 支持 zoho.com、zohomail.com、zoho.eu 域名。
pub struct ZohoProvider {
    info: ProviderInfo,
}

impl ZohoProvider {
    /// 创建 Zoho 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "zoho".into(),
                name: "Zoho Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["zoho.com".into(), "zohomail.com".into(), "zoho.eu".into()],
                auth_type: AuthType::Password,
                color: Some("#D4382C".into()),
                icon: Some("zoho".into()),
                sort_order: 11,
            },
        }
    }
}

/// 默认实现，等同于 [`ZohoProvider::new`]
impl Default for ZohoProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Zoho 邮箱的 [`MailProvider`] 实现
impl MailProvider for ZohoProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.zoho.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.zoho.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.zoho.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.zoho.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：zoho.com、zohomail.com、zoho.eu
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["zoho.com", "zohomail.com", "zoho.eu"]
    }

    /// Zoho 文件夹映射，使用英文文件夹名称
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec!["Archive".into()],
        }
    }
}
