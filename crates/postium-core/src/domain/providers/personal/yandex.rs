use crate::domain::providers::*;

/// Yandex 邮箱服务商
///
/// 俄罗斯 Yandex 提供的邮箱服务，使用密码认证，
/// 支持 yandex.com、yandex.ru、ya.ru、yandex.ua 等域名。
pub struct YandexMailProvider {
    info: ProviderInfo,
}

impl YandexMailProvider {
    /// 创建 Yandex 邮箱服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "yandex".into(),
                name: "Yandex Mail".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "yandex.com".into(),
                    "yandex.ru".into(),
                    "ya.ru".into(),
                    "yandex.ua".into(),
                ],
                auth_type: AuthType::Password,
                color: Some("#FC3F1D".into()),
                icon: Some("yandex".into()),
                sort_order: 15,
            },
        }
    }
}

/// 默认实现，等同于 [`YandexMailProvider::new`]
impl Default for YandexMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Yandex 邮箱的 [`MailProvider`] 实现
impl MailProvider for YandexMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.yandex.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.yandex.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.yandex.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.yandex.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：yandex.com、yandex.ru、ya.ru、yandex.ua
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["yandex.com", "yandex.ru", "ya.ru", "yandex.ua"]
    }
}
