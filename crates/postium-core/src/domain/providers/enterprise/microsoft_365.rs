use crate::domain::providers::*;

/// Microsoft 365 企业邮箱服务商
///
/// 对应使用自有域名的 Microsoft 365（原 Office 365）企业邮箱。
/// 使用 outlook.office365.com 作为邮件服务器，通过 OAuth2 认证企业账号，
/// 不绑定特定域名（企业可使用任意自定义域名）。
pub struct Microsoft365Provider {
    /// 服务商基本信息（id 为 "microsoft_365"，OAuth2 认证）
    info: ProviderInfo,
}

impl Microsoft365Provider {
    /// 创建 Microsoft 365 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "microsoft_365".into(),
                name: "Microsoft 365".into(),
                account_type: AccountType::Enterprise,
                domains: vec![],
                auth_type: AuthType::OAuth2,
                color: Some("#0078D4".into()),
                icon: Some("microsoft_365".into()),
                sort_order: 10,
            },
        }
    }
}

impl Default for Microsoft365Provider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for Microsoft365Provider {
    /// 返回 Microsoft 365 服务商的基本信息
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// 返回 IMAP 配置：outlook.office365.com:993（隐式 SSL）
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "outlook.office365.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// 返回 SMTP 配置：smtp.office365.com:587（STARTTLS）
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.office365.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    /// 返回空列表（企业域名不固定，无法通过域名自动识别）
    fn supported_domains(&self) -> Vec<&'static str> {
        vec![]
    }

    /// 返回 Microsoft OAuth2 配置
    ///
    /// 使用编译时注入的 `MICROSOFT_CLIENT_ID` / `MICROSOFT_CLIENT_SECRET`，
    /// 启用 PKCE，通过 common 租户支持任意企业账号，
    /// 申请 IMAP/SMTP 访问、离线访问及 OpenID 范围。
    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: option_env!("MICROSOFT_CLIENT_ID")
                .unwrap_or_default()
                .to_string(),
            client_secret: option_env!("MICROSOFT_CLIENT_SECRET").map(|s| s.to_string()),
            auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string(),
            scopes: vec![
                "https://outlook.office365.com/IMAP.AccessAsUser.All".into(),
                "https://outlook.office365.com/SMTP.Send".into(),
                "offline_access".into(),
                "openid".into(),
                "email".into(),
            ],
            pkce_enabled: true,
            tenant_id: None,
        })
    }

    /// 返回 Outlook 文件夹映射
    ///
    /// 垃圾邮件文件夹为 "Junk"，已删除为 "Deleted"，无归档文件夹。
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

    /// 生成 XOAUTH2 认证字符串，用于 IMAP/SMTP 的 OAuth2 登录
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
