use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::providers::{ImapServerConfig, SslMode};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use crate::service::account_service::CreateAccountRequest;
use async_trait::async_trait;
use std::time::Duration;

const IMAP_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(10);

#[async_trait]
pub trait ImapConnectionVerifier: Send + Sync {
    async fn verify(
        &self,
        config: &ImapServerConfig,
        email: &str,
        password: &str,
    ) -> Result<(), MailError>;
}

pub struct RealImapConnectionVerifier;

#[async_trait]
impl ImapConnectionVerifier for RealImapConnectionVerifier {
    async fn verify(
        &self,
        config: &ImapServerConfig,
        email: &str,
        password: &str,
    ) -> Result<(), MailError> {
        verify_imap_with_timeout(
            config,
            verify_imap_without_timeout(config, email, password),
            IMAP_VERIFICATION_TIMEOUT,
        )
        .await
    }
}

async fn verify_imap_with_timeout<F>(
    config: &ImapServerConfig,
    verify: F,
    timeout: Duration,
) -> Result<(), MailError>
where
    F: std::future::Future<Output = Result<(), MailError>>,
{
    match tokio::time::timeout(timeout, verify).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(
                host = %config.host,
                port = config.port,
                timeout_secs = timeout.as_secs(),
                "IMAP 账号添加连接校验超时"
            );
            Err(MailError::ImapConnectionFailed(format!(
                "连接 {}:{} 超时，请检查网络、服务器地址或端口",
                config.host, config.port
            )))
        }
    }
}

async fn verify_imap_without_timeout(
    config: &ImapServerConfig,
    email: &str,
    password: &str,
) -> Result<(), MailError> {
    tracing::debug!(
        host = %config.host,
        port = config.port,
        email,
        "IMAP: 开始账号添加连接校验"
    );
    let client = ImapClient::connect(config, email, password).await?;
    if let Err(err) = client.logout().await {
        tracing::debug!(error = %err, "IMAP 校验登录成功但登出失败");
    }
    Ok(())
}

pub struct NoopImapConnectionVerifier;

#[async_trait]
impl ImapConnectionVerifier for NoopImapConnectionVerifier {
    async fn verify(
        &self,
        _config: &ImapServerConfig,
        _email: &str,
        _password: &str,
    ) -> Result<(), MailError> {
        Ok(())
    }
}

pub fn imap_config_from_create_request(
    req: &CreateAccountRequest,
) -> Result<ImapServerConfig, MailError> {
    if let Some(config) = manual_imap_config(
        req.imap_host.as_deref(),
        req.imap_port,
        req.imap_ssl_mode.as_deref(),
    )? {
        return Ok(config);
    }

    provider_imap_config(&req.provider, &req.email)
}

pub fn imap_config_from_account(account: &accounts::Model) -> Result<ImapServerConfig, MailError> {
    if let Some(config) = manual_imap_config(
        account.imap_host.as_deref(),
        account.imap_port,
        account.imap_ssl_mode.as_deref(),
    )? {
        return Ok(config);
    }

    provider_imap_config(&account.provider, &account.email)
}

fn manual_imap_config(
    host: Option<&str>,
    port: Option<i32>,
    ssl_mode: Option<&str>,
) -> Result<Option<ImapServerConfig>, MailError> {
    let Some(host) = host.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let Some(port) = port else {
        return Ok(None);
    };
    let port = u16::try_from(port)
        .map_err(|_| MailError::InvalidParam(format!("IMAP 端口无效: {port}")))?;
    let ssl = parse_ssl_mode(ssl_mode.unwrap_or("Tls"))?;

    Ok(Some(ImapServerConfig {
        host: host.trim().to_string(),
        port,
        ssl,
    }))
}

fn provider_imap_config(provider_id: &str, email: &str) -> Result<ImapServerConfig, MailError> {
    let provider_pool = PROVIDER_POOL
        .get()
        .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
    let provider = provider_pool
        .get(provider_id)
        .ok_or_else(|| MailError::ProviderNotSupported(provider_id.to_string()))?;

    Ok(provider.imap_config(email))
}

fn parse_ssl_mode(value: &str) -> Result<SslMode, MailError> {
    match value {
        "Tls" | "TLS" | "Implicit" => Ok(SslMode::Implicit),
        "StartTls" | "STARTTLS" => Ok(SslMode::StartTls),
        "None" => Ok(SslMode::None),
        other => Err(MailError::InvalidParam(format!(
            "IMAP 加密模式无效: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::providers::pool::init_provider_pool;

    #[test]
    fn manual_imap_config_should_override_provider_config() {
        let req = CreateAccountRequest {
            name: "Manual".to_string(),
            email: "manual@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password".to_string(),
            imap_host: Some("imap.example.com".to_string()),
            imap_port: Some(143),
            imap_ssl_mode: Some("StartTls".to_string()),
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        };

        let config = imap_config_from_create_request(&req).unwrap();

        assert_eq!(config.host, "imap.example.com");
        assert_eq!(config.port, 143);
        assert!(matches!(config.ssl, SslMode::StartTls));
    }

    #[test]
    fn account_model_manual_imap_config_should_override_provider_config() {
        let account = accounts::Model {
            id: 1,
            name: "Manual".to_string(),
            email: "manual@example.com".to_string(),
            display_name: None,
            provider: "custom".to_string(),
            imap_host: Some("imap.manual.example.com".to_string()),
            imap_port: Some(143),
            imap_ssl: Some(true),
            imap_ssl_mode: Some("StartTls".to_string()),
            smtp_host: None,
            smtp_port: None,
            smtp_ssl: Some(true),
            smtp_ssl_mode: None,
            color: None,
            sync_enabled: Some(true),
            last_sync_at: None,
            auth_type: Some("Password".to_string()),
            account_type: "personal".to_string(),
            created_at: 1,
            updated_at: 1,
        };

        let config = imap_config_from_account(&account).unwrap();

        assert_eq!(config.host, "imap.manual.example.com");
        assert_eq!(config.port, 143);
        assert!(matches!(config.ssl, SslMode::StartTls));
    }

    #[test]
    fn provider_imap_config_should_be_used_when_manual_config_missing() {
        init_provider_pool();
        let req = CreateAccountRequest {
            name: "Gmail".to_string(),
            email: "user@gmail.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            auth_type: "Password".to_string(),
            password: "password".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl_mode: None,
            color: None,
            account_type: None,
        };

        let config = imap_config_from_create_request(&req).unwrap();

        assert_eq!(config.host, "imap.gmail.com");
        assert_eq!(config.port, 993);
        assert!(matches!(config.ssl, SslMode::Implicit));
    }

    #[tokio::test]
    async fn imap_verification_should_return_error_when_timed_out() {
        let config = ImapServerConfig {
            host: "imap.example.com".to_string(),
            port: 993,
            ssl: SslMode::Implicit,
        };

        let result = verify_imap_with_timeout(
            &config,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok(())
            },
            Duration::from_millis(1),
        )
        .await;

        assert!(matches!(
            result,
            Err(MailError::ImapConnectionFailed(message))
                if message.contains("超时") && message.contains("imap.example.com:993")
        ));
    }
}
