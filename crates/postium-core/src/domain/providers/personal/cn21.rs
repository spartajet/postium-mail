use crate::domain::providers::*;

/// 21CN 邮箱服务商
///
/// 中国电信旗下的邮箱服务，使用密码认证，
/// 支持 21cn.com 域名。
pub struct Cn21MailProvider {
    info: ProviderInfo,
}

impl Cn21MailProvider {
    /// 创建 21CN 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "cn21".into(),
                name: "21CN邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["21cn.com".into(), "21cn.net".into()],
                auth_type: AuthType::Password,
                color: Some("#0066CC".into()),
                icon: Some("cn21".into()),
                sort_order: 19,
            },
        }
    }
}

/// 默认实现，等同于 [`Cn21MailProvider::new`]
impl Default for Cn21MailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 21CN 邮箱的 [`MailProvider`] 实现
impl MailProvider for Cn21MailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.21cn.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.21cn.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.21cn.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.21cn.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：21cn.com、21cn.net
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["21cn.com", "21cn.net"]
    }
}
