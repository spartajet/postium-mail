//! 引擎接入点：服务层获取引擎句柄的统一入口。
//!
//! 服务（mail_operation / mail_send / mail_draft）通过 [`EngineAccess`] 把
//! "按账号拿引擎句柄"抽象出来，避免直接依赖 CoreApi（CoreApi 持有服务，
//! 反向持有会形成循环依赖）。生产实现 [`RegistryEngineAccess`] 直接委托
//! [`crate::engine::registry::ensure_engine`]，与 `CoreApi::imap_engine`
//! 同一路径；registry 按账号 id 去重，不会重复建引擎。

use crate::domain::auth::AuthManager;
use crate::domain::sync::SyncEventTx;
use crate::engine::actor::EngineHandle;
use crate::engine::factory::RealSessionFactory;
use crate::error::MailError;
use crate::infrastructure::storage::models::accounts;
use async_trait::async_trait;
use std::sync::Arc;

/// 引擎句柄提供者（服务层测试 seam）。
#[async_trait]
pub trait EngineAccess: Send + Sync {
    async fn imap_engine(&self, account: &accounts::Model) -> Result<EngineHandle, MailError>;
}

/// 生产实现：ensure_engine + RealSessionFactory（凭证走 AuthManager）。
pub struct RegistryEngineAccess {
    auth: Arc<AuthManager>,
    sync_tx: SyncEventTx,
}

impl RegistryEngineAccess {
    pub fn new(auth: Arc<AuthManager>, sync_tx: SyncEventTx) -> Self {
        Self { auth, sync_tx }
    }
}

#[async_trait]
impl EngineAccess for RegistryEngineAccess {
    async fn imap_engine(&self, account: &accounts::Model) -> Result<EngineHandle, MailError> {
        crate::engine::registry::ensure_engine(
            account,
            Arc::new(RealSessionFactory {
                auth: self.auth.clone(),
                account: account.clone(),
            }),
            self.sync_tx.clone(),
        )
    }
}
