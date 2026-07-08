use crate::domain::providers::*;

/// Tom 邮箱服务商
///
/// TOM 集团提供的邮箱服务，使用密码认证，
/// 支持 tom.com 域名。
pub struct TomMailProvider {
    info: ProviderInfo,
}

impl TomMailProvider {
    /// 创建 Tom 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "tom".into(),
                name: "Tom邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["tom.com".into()],
                auth_type: AuthType::Password,
                color: Some("#0066CC".into()),
                icon: Some("tom".into()),
                sort_order: 17,
            },
        }
    }
}

/// 默认实现，等同于 [`TomMailProvider::new`]
impl Default for TomMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Tom 邮箱的 [`MailProvider`] 实现
impl MailProvider for TomMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.tom.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.tom.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.tom.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.tom.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：tom.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["tom.com"]
    }
}
