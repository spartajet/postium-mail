use crate::domain::providers::*;

/// 263 邮箱服务商
///
/// 二六三网络提供的邮箱服务，使用密码认证，
/// 支持 263.net、263.com、x263.net 域名。
pub struct Net263MailProvider {
    info: ProviderInfo,
}

impl Net263MailProvider {
    /// 创建 263 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "net263".into(),
                name: "263邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["263.net".into(), "263.com".into(), "x263.net".into()],
                auth_type: AuthType::Password,
                color: Some("#FF6600".into()),
                icon: Some("net263".into()),
                sort_order: 18,
            },
        }
    }
}

/// 默认实现，等同于 [`Net263MailProvider::new`]
impl Default for Net263MailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 263 邮箱的 [`MailProvider`] 实现
impl MailProvider for Net263MailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.263.net:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.263.net".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.263.net:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.263.net".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：263.net、263.com、x263.net
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["263.net", "263.com", "x263.net"]
    }
}
