use crate::domain::providers::*;

/// 中国移动 139 邮箱服务商
///
/// 中国移动旗下的邮箱服务，使用密码认证，
/// 支持 139.com 等域名。
pub struct CmccMailProvider {
    info: ProviderInfo,
}

impl CmccMailProvider {
    /// 创建 139 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "cmcc".into(),
                name: "中国移动139邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["139.com".into(), "139.com.cn".into(), "10086.cn".into()],
                auth_type: AuthType::Password,
                color: Some("#0066FF".into()),
                icon: Some("cmcc".into()),
                sort_order: 7,
            },
        }
    }
}

/// 默认实现，等同于 [`CmccMailProvider::new`]
impl Default for CmccMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 139 邮箱的 [`MailProvider`] 实现
impl MailProvider for CmccMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.139.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.139.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.139.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.139.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：139.com、139.com.cn、10086.cn
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["139.com", "139.com.cn", "10086.cn"]
    }
}
