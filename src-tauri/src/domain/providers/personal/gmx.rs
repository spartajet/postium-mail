use crate::domain::providers::*;

/// GMX 邮箱服务商
///
/// 德国 GMX 提供的邮箱服务，使用密码认证，
/// 支持 gmx.com、gmx.net、gmx.de、gmx.fr、gmx.co.uk 等多国域名。
pub struct GmxMailProvider {
    info: ProviderInfo,
}

impl GmxMailProvider {
    /// 创建 GMX 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "gmx".into(),
                name: "GMX Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "gmx.com".into(),
                    "gmx.net".into(),
                    "gmx.de".into(),
                    "gmx.fr".into(),
                    "gmx.co.uk".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#1C449B".into()),
                icon: Some("gmx".into()),
                sort_order: 13,
            },
        }
    }
}

/// 默认实现，等同于 [`GmxMailProvider::new`]
impl Default for GmxMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// GMX 邮箱的 [`MailProvider`] 实现
impl MailProvider for GmxMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.gmx.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmx.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：mail.gmx.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "mail.gmx.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 支持的域名：gmx.com、gmx.net、gmx.de、gmx.fr、gmx.co.uk
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmx.com", "gmx.net", "gmx.de", "gmx.fr", "gmx.co.uk"]
    }
}
