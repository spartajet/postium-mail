use crate::domain::providers::*;

/// 网易邮箱服务商
///
/// 网易提供的邮箱服务，使用密码认证，
/// 统一支持 163.com、126.com、yeah.net 域名，
/// 不同域名对应不同的服务器地址。
pub struct NetEaseProvider {
    info: ProviderInfo,
}

impl NetEaseProvider {
    /// 创建网易邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "yi".into(),
                name: "网易邮箱".into(),
                account_type: AccountType::Personal,
                domains: vec!["163.com".into(), "126.com".into(), "yeah.net".into()],
                auth_type: AuthType::Password,
                color: Some("#D9291D".into()),
                icon: Some("yi".into()),
                sort_order: 2,
            },
        }
    }
}

/// 默认实现，等同于 [`NetEaseProvider::new`]
impl Default for NetEaseProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 网易邮箱的 [`MailProvider`] 实现
impl MailProvider for NetEaseProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：根据邮箱域名动态选择服务器（端口 993，隐式 SSL/TLS），
    /// 126.com 使用 imap.126.com，yeah.net 使用 imap.yeah.net
    fn imap_config(&self, email: &str) -> ImapServerConfig {
        let domain = email.split('@').next_back().unwrap_or("163.com");
        let host = match domain {
            "126.com" => "imap.126.com",
            "yeah.net" => "imap.yeah.net",
            _ => "imap.163.com",
        };
        ImapServerConfig {
            host: host.into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：根据邮箱域名动态选择服务器（端口 465，隐式 SSL/TLS），
    /// 126.com 使用 smtp.126.com，yeah.net 使用 smtp.yeah.net
    fn smtp_config(&self, email: &str) -> SmtpServerConfig {
        let domain = email.split('@').next_back().unwrap_or("163.com");
        let host = match domain {
            "126.com" => "smtp.126.com",
            "yeah.net" => "smtp.yeah.net",
            _ => "smtp.163.com",
        };
        SmtpServerConfig {
            host: host.into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：163.com、126.com、yeah.net
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["163.com", "126.com", "yeah.net"]
    }

    /// 网易邮箱文件夹映射，包含中文环境下 IMAP UTF-7 编码的文件夹名
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into(), "&XfJT0ZAB-".into()],
            drafts: vec!["&g0l6P3ux-".into()],
            spam: vec!["&W4xRaFeDVz6Qrk72-".into(), "&V4NXPpCuTvY-".into()],
            trash: vec!["&XfJSIJZk-".into()],
            archive: vec!["Archive".into(), "&W1hoYw-".into()],
        }
    }
}
