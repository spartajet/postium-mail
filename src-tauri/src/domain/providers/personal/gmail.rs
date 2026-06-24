use crate::domain::providers::*;

/// Gmail 邮箱服务商
///
/// Google 提供的邮件服务，使用 OAuth2 令牌认证，
/// 支持 gmail.com、googlemail.com 域名，
/// 支持 XOAUTH2 认证方式。
pub struct GmailProvider {
    info: ProviderInfo,
}

impl GmailProvider {
    /// 创建 Gmail 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "gmail".into(),
                name: "Gmail".into(),
                account_type: AccountType::Personal,
                domains: vec!["gmail.com".into(), "googlemail.com".into()],
                auth_type: AuthType::OAuth2,
                color: Some("#EA4335".into()),
                icon: Some("gmail".into()),
                sort_order: 3,
            },
        }
    }
}

/// 默认实现，等同于 [`GmailProvider::new`]
impl Default for GmailProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Gmail 的 [`MailProvider`] 实现
impl MailProvider for GmailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：imap.gmail.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmail.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.gmail.com:465（隐式 SSL/TLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.gmail.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 支持的域名：gmail.com、googlemail.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmail.com", "googlemail.com"]
    }

    /// OAuth2 配置（启用 PKCE），客户端凭证从环境变量读取
    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: option_env!("GOOGLE_CLIENT_ID")
                .unwrap_or_default()
                .to_string(),
            client_secret: option_env!("GOOGLE_CLIENT_SECRET").map(|s| s.to_string()),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            scopes: vec![
                "https://mail.google.com/".into(),
                "https://www.googleapis.com/auth/userinfo.email".into(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        })
    }

    /// Gmail 文件夹映射，使用 `[Gmail]/` 前缀的特殊文件夹名称，
    /// 同时兼容中文环境下 IMAP UTF-7 编码的文件夹名
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["[Gmail]/Sent Mail".into(), "[Gmail]/&XfJT0ZCuTvY-".into()],
            drafts: vec!["[Gmail]/Drafts".into(), "Drafts".into()],
            spam: vec!["[Gmail]/Spam".into(), "[Gmail]/&g0l6Pw-".into()],
            trash: vec!["[Gmail]/Trash".into(), "[Gmail]/&V4NXPpCuTvY-".into()],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        }
    }

    /// 生成 XOAUTH2 认证字符串，用于 IMAP/SMTP 的 OAuth2 认证，
    /// 将用户名与 Bearer 令牌拼接后进行 Base64 编码
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
