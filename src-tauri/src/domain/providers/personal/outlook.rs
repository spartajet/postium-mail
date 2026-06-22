use crate::domain::providers::*;

pub struct OutlookProvider {
    info: ProviderInfo,
}

impl OutlookProvider {
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

impl Default for OutlookProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MailProvider for OutlookProvider {
    fn provider_info(&self) -> &ProviderInfo {
        &self.info
    }

    fn imap_config(&self, _email: &str) -> ImapServerConfig {
        ImapServerConfig {
            host: "outlook.office365.com".into(),
            port: 993,
            ssl: SslMode::Implicit,
        }
    }

    fn smtp_config(&self, _email: &str) -> SmtpServerConfig {
        SmtpServerConfig {
            host: "smtp.office365.com".into(),
            port: 587,
            ssl: SslMode::StartTls,
        }
    }

    fn supported_domains(&self) -> Vec<&'static str> {
        vec!["outlook.com", "hotmail.com", "live.com", "msn.com"]
    }

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

    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String {
        let auth_string = format!("user={}\x01auth=Bearer {}\x01\x01", email, access_token);
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, auth_string)
    }
}
