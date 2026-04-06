use crate::domain::providers::*;

pub struct GmailProvider {
    info: ProviderInfo,
}

impl GmailProvider {
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

impl Default for GmailProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for GmailProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "imap.gmail.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.gmail.com".into(),
            port: 465,
            ssl: SslMode::Implicit,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["gmail.com", "googlemail.com"]
    }

    fn oauth_config(&self) -> Option<OAuthConfig> {
        Some(OAuthConfig {
            client_id: env!("GOOGLE_CLIENT_ID").to_string(),
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

    fn folder_mapping(&self) -> StandardFolder {
        StandardFolder {
            inbox: vec!["INBOX".into()],
            sent: vec![
                "[Gmail]/Sent Mail".into(),
                "[Gmail]/&XfJT0ZCuTvY-".into(),
            ],
            drafts: vec!["[Gmail]/Drafts".into(), "Drafts".into()],
            spam: vec!["[Gmail]/Spam".into(), "[Gmail]/&g0l6Pw-".into()],
            trash: vec![
                "[Gmail]/Trash".into(),
                "[Gmail]/&V4NXPpCuTvY-".into(),
            ],
            archive: vec!["[Gmail]/All Mail".into(), "Archive".into()],
        }
    }

    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
