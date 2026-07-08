use crate::domain::providers::*;

/// QQ 邮箱服务商
///
/// 腾讯旗下的邮箱服务，支持 qq.com 和 foxmail.com 域名，
/// 使用授权码（密码）认证。
pub struct QqMailProvider {
    info: ProviderInfo,
}

impl QqMailProvider {
    /// 创建 QQ 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "qq".into(),
                name: "QQ Mail".into(),
                account_type: AccountType::Personal,
                domains: vec!["qq.com".into(), "foxmail.com".into()],
                auth_type: AuthType::Password,
                color: Some("#12B7F5".into()),
                icon: Some("qq".into()),
                sort_order: 1,
            },
        }
    }
}

/// 默认实现，等同于 [`QqMailProvider::new`]
impl Default for QqMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// QQ 邮箱的 [`MailProvider`] 实现
impl MailProvider for QqMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.qq.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.qq.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.qq.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.qq.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：qq.com、foxmail.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["qq.com", "foxmail.com"]
    }

    /// QQ 邮箱文件夹映射，使用英文文件夹名称
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent Messages".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Junk".into()],
            trash: vec!["Deleted Messages".into()],
            archive: vec![],
        }
    }
}
