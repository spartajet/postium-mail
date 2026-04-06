pub mod detect;
pub mod enterprise;
pub mod personal;
pub mod pool;

pub use pool::ProviderPool;
use serde::{Deserialize, Serialize};
use specta::Type;

// ─── 值对象 ───

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum AccountType {
    Personal,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub domains: Vec<String>,
    pub auth_type: AuthType,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub sort_order: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum AuthType {
    Password,
    OAuth2,
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthType::Password => "Password",
            AuthType::OAuth2 => "OAuth2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImapServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SmtpServerConfig {
    pub host: String,
    pub port: u16,
    pub ssl: SslMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
pub enum SslMode {
    None,
    StartTls,
    Implicit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct StandardFolder {
    pub inbox: Vec<String>,
    pub sent: Vec<String>,
    pub drafts: Vec<String>,
    pub spam: Vec<String>,
    pub trash: Vec<String>,
    pub archive: Vec<String>,
}

impl StandardFolder {
    /// 默认英文文件夹映射
    pub fn default_english() -> Self {
        Self {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into(), "Sent Items".into(), "Sent Messages".into()],
            drafts: vec!["Drafts".into(), "Draft".into()],
            spam: vec!["Spam".into(), "Junk".into(), "Bulk Mail".into()],
            trash: vec!["Trash".into(), "Deleted".into(), "Deleted Messages".into()],
            archive: vec!["Archive".into(), "Archived".into()],
        }
    }

    /// 从 IMAP 文件夹名称查找对应的标准文件夹类型
    ///
    /// 先精确匹配，再包含匹配。返回 "inbox", "sent", "drafts", "spam", "trash", "archive" 或 "other"
    pub fn find_standard_type(&self, imap_name: &str) -> &str {
        let fields: &[(&str, &[String])] = &[
            ("inbox", &self.inbox),
            ("sent", &self.sent),
            ("drafts", &self.drafts),
            ("spam", &self.spam),
            ("trash", &self.trash),
            ("archive", &self.archive),
        ];

        // 精确匹配
        for (name, folders) in fields {
            if folders.iter().any(|n| n == imap_name) {
                return name;
            }
        }

        // 包含匹配（忽略大小写）
        let imap_lower = imap_name.to_lowercase();
        for (name, folders) in fields {
            if folders.iter().any(|n| {
                let n_lower = n.to_lowercase();
                imap_lower.contains(&n_lower) || n_lower.contains(&imap_lower)
            }) {
                return name;
            }
        }

        "other"
    }

    pub fn list(&self) -> Vec<String> {
        self.inbox
            .iter()
            .chain(self.sent.iter())
            .chain(self.drafts.iter())
            .chain(self.spam.iter())
            .chain(self.trash.iter())
            .chain(self.archive.iter())
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub scopes: Vec<String>,
    pub pkce_enabled: bool,
    pub tenant_id: Option<String>,
}

// ─── 核心配置 Trait ───

pub trait MailProvider: Send + Sync {
    fn provider_info(&self) -> &ProviderInfo;
    fn imap_config(&self, _email: &str) -> ImapServerConfig;
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig;
    fn supported_domains(&self) -> Vec<&'static str>;

    // 带默认实现的方法
    fn detect(&self, email: &str) -> bool {
        let email_domain = email.split('@').next_back().unwrap_or("").to_lowercase();
        self.supported_domains()
            .iter()
            .any(|d| d.to_lowercase() == email_domain)
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        None
    }

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder::default_english()
    }

    fn generate_xoauth2(&self, _email: &str, _access_token: &str) -> String {
        String::new()
    }

    fn generate_redirect_uri(&self, port: u16) -> String {
        format!("http://127.0.0.1:{}/oauth/callback", port)
    }
}
