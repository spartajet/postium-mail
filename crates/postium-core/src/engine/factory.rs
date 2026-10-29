//! 连接工厂：绑定单个账号，凭据 + IMAP 配置 → 已认证会话（含能力探测与 ENABLE）。

use crate::domain::auth::AuthManager;
use crate::domain::auth::manager::Credentials;
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::engine::capabilities::CapabilitySet;
use crate::engine::pool::{PooledSession, PoolConnect};
use crate::engine::transport::{ImapSession, RealSession};
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use crate::service::account_connection::imap_config_from_account;
use async_trait::async_trait;
use std::sync::Arc;

/// 会话工厂：绑定单个账号，产出已认证 + 已 ENABLE 的会话与能力集。
#[async_trait]
pub trait SessionFactory: Send + Sync {
    async fn connect(&self) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError>;
}

/// 真实工厂：密码 / XOAUTH2 双路径（与 mail_operation.rs 连接逻辑一致）。
pub struct RealSessionFactory {
    pub auth: Arc<AuthManager>,
    pub account: accounts::Model,
}

#[async_trait]
impl SessionFactory for RealSessionFactory {
    async fn connect(&self) -> Result<(Box<dyn ImapSession>, CapabilitySet), MailError> {
        let account = &self.account;
        let provider_pool = PROVIDER_POOL
            .get()
            .ok_or_else(|| MailError::ProviderNotSupported("未找到provider pool".to_string()))?;
        let provider = provider_pool
            .get(&account.provider)
            .ok_or_else(|| MailError::ProviderNotSupported(account.provider.clone()))?;
        let credentials = self
            .auth
            .get_credentials(
                &account.email,
                &provider.provider_info().auth_type,
                Some(&account.provider),
            )
            .await?;
        let imap_config = imap_config_from_account(account)?;
        let mut session: Box<dyn ImapSession> = match credentials {
            Credentials::Password(password) => Box::new(RealSession::new(
                ImapClient::connect(&imap_config, &account.email, &password).await?,
            )),
            Credentials::OAuth2 { access_token } => Box::new(RealSession::new(
                ImapClient::connect_xoauth2(&imap_config, &account.email, &access_token).await?,
            )),
        };
        let caps = CapabilitySet::parse(&session.capabilities().await?);
        session.enable(&caps.enable_list()).await?;
        Ok((session, caps))
    }
}

#[async_trait]
impl PoolConnect for RealSessionFactory {
    async fn connect(&self) -> Result<PooledSession, MailError> {
        let (session, caps) = SessionFactory::connect(self).await?;
        Ok(PooledSession { session, caps })
    }
}
