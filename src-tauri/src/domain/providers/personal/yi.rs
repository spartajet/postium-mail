use crate::domain::providers::*;

/// 网易邮箱 — 统一支持 163.com、126.com、yeah.net
pub struct NetEaseProvider {
    info: ProviderInfo,
}

impl NetEaseProvider {
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

impl Default for NetEaseProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for NetEaseProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

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

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["163.com", "126.com", "yeah.net"]
    }

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
