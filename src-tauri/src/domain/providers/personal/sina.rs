use crate::domain::providers::*;

/// 新浪邮箱服务商
///
/// 新浪提供的邮箱服务，使用密码认证，
/// 支持 sina.com、sina.cn、vip.sina.com 域名，
/// 不同域名对应不同的服务器地址。
pub struct SinaMailProvider {
    info: ProviderInfo,
}

impl SinaMailProvider {
    /// 创建新浪邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "sina".into(),
                name: "新浪邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["sina.com".into(), "sina.cn".into(), "vip.sina.com".into()],
                auth_type: AuthType::Password,
                color: Some("#E6162D".into()),
                icon: Some("sina".into()),
                sort_order: 8,
            },
        }
    }
}

/// 默认实现，等同于 [`SinaMailProvider::new`]
impl Default for SinaMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 新浪邮箱的 [`MailProvider`] 实现
impl MailProvider for SinaMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：根据邮箱域名动态选择服务器（端口 993，隐式 SSL/TLS），
    /// sina.cn 使用 imap.sina.cn，vip.sina.com 使用 imap.mail.sina.com.cn
    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').next_back().unwrap_or("sina.com");
        let host = match domain {
            "sina.cn" => "imap.sina.cn",
            "vip.sina.com" => "imap.mail.sina.com.cn",
            _ => "imap.sina.com",
        };
        ImapServerConfig {
            host: host.into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：根据邮箱域名动态选择服务器（端口 465，隐式 SSL/TLS），
    /// sina.cn 使用 smtp.sina.cn，vip.sina.com 使用 smtp.mail.sina.com.cn
    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        let domain = email.split('@').next_back().unwrap_or("sina.com");
        let host = match domain {
            "sina.cn" => "smtp.sina.cn",
            "vip.sina.com" => "smtp.mail.sina.com.cn",
            _ => "smtp.sina.com",
        };
        SmtpServerConfig {
            host: host.into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：sina.com、sina.cn、vip.sina.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["sina.com", "sina.cn", "vip.sina.com"]
    }
}
