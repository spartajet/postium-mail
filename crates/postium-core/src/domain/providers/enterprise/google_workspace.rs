use crate::domain::providers::*;

/// Google Workspace 企业邮箱服务商
///
/// 对应使用自有域名的 Google Workspace（原 G Suite）企业邮箱。
/// 与个人 Gmail 使用相同的服务器地址，但通过 OAuth2 认证企业账号，
/// 不绑定特定域名（企业可使用任意自定义域名）。
pub struct GoogleWorkspaceProvider {
    /// 服务商基本信息（id 为 "google_workspace"，OAuth2 认证）
    info: ProviderInfo,
}

impl GoogleWorkspaceProvider {
    /// 创建 Google Workspace 服务商实例
    pub fn new() -> Self {
        Self {
            info: ProviderInfo {
                id: "google_workspace".into(),
                name: "Google Workspace".into(),
                account_type: AccountType::Enterprise,
                domains: vec![],
                auth_type: AuthType::OAuth2,
                color: Some("#EA4335".into()),
                icon: Some("google_workspace".into()),
                sort_order: 9,
            },
        }
    }
}

impl Default for GoogleWorkspaceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for GoogleWorkspaceProvider {
    /// 返回 Google Workspace 服务商的基本信息
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    /// 返回 IMAP 配置：imap.gmail.com:993（隐式 SSL）
    ///
    /// 与个人 Gmail 使用相同的 IMAP 服务器。
    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmail.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    /// 返回 SMTP 配置：smtp.gmail.com:465（隐式 SSL）
    ///
    /// 与个人 Gmail 使用相同的 SMTP 服务器。
    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.gmail.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    /// 返回空列表（企业域名不固定，无法通过域名自动识别）
    fn supported_domains(&self) -> Vec<&'static str> {
        vec![]
    }

    /// 返回 Google OAuth2 配置
    ///
    /// 使用编译时注入的 `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET`，
    /// 启用 PKCE，申请 Gmail 完整访问权限和邮箱信息读取权限。
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

    /// 返回 Gmail 文件夹映射
    ///
    /// 包含 Gmail 特有的 `[Gmail]/` 前缀文件夹名及通用别名，
    /// 兼容不同语言环境下的文件夹命名。
    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec!["[Gmail]/Sent Mail".into(), "Sent".into()],
            drafts: vec!["[Gmail]/Drafts".into(), "Drafts".into()],
            spam: vec!["[Gmail]/Spam".into(), "Spam".into()],
            trash: vec!["[Gmail]/Trash".into(), "Trash".into()],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        }
    }

    /// 生成 XOAUTH2 认证字符串，用于 IMAP/SMTP 的 OAuth2 登录
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
