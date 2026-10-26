use crate::domain::providers::*;

/// 搜狐邮箱服务商
///
/// 搜狐提供的邮箱服务，使用密码认证，
/// 支持 sohu.com、vip.sohu.com、sohu.net 域名。
pub struct SohuMailProvider {
    info: ProviderInfo,
}

impl SohuMailProvider {
    /// 创建搜狐邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "sohu".into(),
                name: "搜狐邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["sohu.com".into(), "vip.sohu.com".into(), "sohu.net".into()],
                auth_type: AuthType::Password,
                color: Some("#DA1F26".into()),
                icon: Some("sohu".into()),
                sort_order: 16,
            },
        }
    }
}

/// 默认实现，等同于 [`SohuMailProvider::new`]
impl Default for SohuMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 搜狐邮箱的 [`MailProvider`] 实现
impl MailProvider for SohuMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.sohu.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.sohu.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.sohu.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.sohu.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：sohu.com、vip.sohu.com、sohu.net
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sohu.com", "vip.sohu.com", "sohu.net"]
    }
}
