use crate::domain::providers::*;

/// AOL 邮箱服务商
///
/// 美国 Verizon 旗下（现归 Yahoo）的邮箱服务，
/// 使用密码认证，支持 aol.com、aim.com 等多个域名。
pub struct AolMailProvider {
    info: ProviderInfo,
}

impl AolMailProvider {
    /// 创建 AOL 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "aol".into(),
                name: "AOL Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "aol.com".into(),
                    "aim.com".into(),
                    "netscape.net".into(),
                    "verizon.net".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#231F20".into()),
                icon: Some("aol".into()),
                sort_order: 12,
            },
        }
    }
}

/// 默认实现，等同于 [`AolMailProvider::new`]
impl Default for AolMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// AOL 邮箱的 [`MailProvider`] 实现
impl MailProvider for AolMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.aol.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.aol.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.aol.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.aol.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 支持的域名：aol.com、aim.com、netscape.net、verizon.net
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["aol.com", "aim.com", "netscape.net", "verizon.net"]
    }
}
