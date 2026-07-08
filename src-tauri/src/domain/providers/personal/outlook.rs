use crate::domain::providers::*;

/// Outlook 邮箱服务商
///
/// Microsoft 提供的个人邮箱服务，使用 OAuth2 令牌认证，
/// 支持 outlook.com、hotmail.com、live.com、msn.com 域名，
/// 支持 XOAUTH2 认证方式。
pub struct OutlookProvider {
    info: ProviderInfo,
}

impl OutlookProvider {
    /// 创建 Outlook 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "outlook".into(),
                name: "Outlook".into(),
                account_type: AccountType::Personal,
                domains: vec![
                    "outlook.com".into(),
                    "hotmail.com".into(),
                    "live.com".into(),
                    "msn.com".into(),
                ],
                auth_type: AuthType::OAuth2,
                color: Some("#0078D4".into()),
                icon: Some("outlook".into()),
                sort_order: 4,
            },
        }
    }
}

/// 默认实现，等同于 [`OutlookProvider::new`]
impl Default for OutlookProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Outlook 的 [`MailProvider`] 实现
impl MailProvider for OutlookProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// IMAP 配置：outlook.office365.com:993（隐式 SSL/TLS）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "outlook.office365.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// SMTP 配置：smtp.office365.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.office365.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 支持的域名：outlook.com、hotmail.com、live.com、msn.com
    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["outlook.com", "hotmail.com", "live.com", "msn.com"]
    }

    /// OAuth2 配置（启用 PKCE），使用 Microsoft 公共授权端点，
    /// 客户端凭证从环境变量读取
    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: option_env!("MICROSOFT_CLIENT_ID")
                .unwrap_or_default()
                .to_string(),
            client_secret: option_env!("MICROSOFT_CLIENT_SECRET").map(|s| s.to_string()),
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            scopes: vec![
                "https://outlook.office.com/IMAP.AccessAsUser.All".into(),
                "https://outlook.office.com/SMTP.Send".into(),
                "offline_access".into(),
                "openid".into(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        })
    }

    /// Outlook 文件夹映射，使用英文文件夹名称
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["Sent".into()],
            drafts: vec!["Drafts".into()],
            spam: vec!["Junk".into()],
            trash: vec!["Deleted".into()],
            archive: vec![],
        }
    }

    /// 生成 XOAUTH2 认证字符串，用于 IMAP/SMTP 的 OAuth2 认证，
    /// 将用户名与 Bearer 令牌拼接后进行 Base64 编码
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
