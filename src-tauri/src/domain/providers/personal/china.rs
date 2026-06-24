use crate::domain::providers::*;

/// 中华网邮箱服务商
///
/// 中国的老牌门户邮箱服务，使用密码认证，
/// 支持 china.com 域名。
pub struct ChinaMailProvider {
    info: ProviderInfo,
}

impl ChinaMailProvider {
    /// 创建中华网邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "china".into(),
                name: "中华网邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["china.com".into(), "mail.china.com".into()],
                auth_type: AuthType::Password,
                color: Some("#CC0000".into()),
                icon: Some("china".into()),
                sort_order: 20,
            },
        }
    }
}

/// 默认实现，等同于 [`ChinaMailProvider::new`]
impl Default for ChinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 中华网邮箱的 [`MailProvider`] 实现
impl MailProvider for ChinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.china.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.china.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.china.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.china.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：china.com、mail.china.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["china.com", "mail.china.com"]
    }
}
