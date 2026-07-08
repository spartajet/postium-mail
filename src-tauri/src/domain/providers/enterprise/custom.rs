use crate::domain::providers::*;

/// 自定义企业邮箱服务商 — 所有服务器配置由用户手动输入
///
/// 当内置服务商无法覆盖用户的邮箱环境时，允许用户自行填写
/// IMAP/SMTP 主机和端口来连接任意企业邮箱服务器。
pub struct CustomProvider {
    /// 服务商基本信息（id 固定为 "custom"，显示名为 "Custom IMAP/SMTP"）
    info: ProviderInfo,
    /// IMAP 服务器主机地址（用户输入）
    imap_host: String,
    /// IMAP 服务器端口（用户输入）
    imap_port: u16,
    /// SMTP 服务器主机地址（用户输入）
    smtp_host: String,
    /// SMTP 服务器端口（用户输入）
    smtp_port: u16,
}

impl CustomProvider {
    /// 创建自定义服务商实例
    ///
    /// 使用用户提供的 IMAP/SMTP 主机和端口初始化。认证方式固定为密码认证，
    /// 不绑定任何特定域名。
    ///
    /// # 参数
    ///
    /// - `imap_host`: IMAP 服务器主机地址
    /// - `imap_port`: IMAP 服务器端口
    /// - `smtp_host`: SMTP 服务器主机地址
    /// - `smtp_port`: SMTP 服务器端口
    pub fn new(imap_host: &str, imap_port: u16, smtp_host: &str, smtp_port: u16) -> Self {
        Self {
            info: ProviderInfo {
                id: "custom".into(),
                name: "Custom IMAP/SMTP".into(),
                account_type: AccountType::Enterprise,
                domains: vec![],
                auth_type: AuthType::Password,
                color: None,
                icon: None,
                sort_order: 99,
            },
            imap_host: imap_host.into(),
            imap_port,
            smtp_host: smtp_host.into(),
            smtp_port,
        }
    }
}

impl MailProvider for CustomProvider {
    /// 返回自定义服务商的基本信息
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// 返回用户配置的 IMAP 服务器连接信息（隐式 SSL）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: self.imap_host.clone(),
            port: self.imap_port,
            ssl: SslMode::Implicit,
        }
    }

    /// 返回用户配置的 SMTP 服务器连接信息（隐式 SSL）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: self.smtp_host.clone(),
            port: self.smtp_port,
            ssl: SslMode::Implicit,
        }
    }

    /// 返回空列表（自定义服务商不绑定特定域名，通过手动添加账号使用）
    fn supported_domains(&self) -> Vec<&'static str> {
        vec![]
    }

    /// 返回通用英文文件夹映射
    ///
    /// 自定义服务商采用最常见的英文文件夹命名约定，
    /// 无归档文件夹映射（留空）。
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Spam".into()],
            trash: vec!["Trash".into()],
            archive: vec![],
        }
    }
}
