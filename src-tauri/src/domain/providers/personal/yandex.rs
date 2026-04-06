use crate::domain::providers::*;

pub struct YandexMailProvider {
    info: ProviderInfo,
}

impl YandexMailProvider {
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

impl Default for YandexMailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for YandexMailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.yandex.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.yandex.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["yandex.com", "yandex.ru", "ya.ru", "yandex.ua"]
    }
}
