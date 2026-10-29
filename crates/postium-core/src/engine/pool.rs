//! 连接池：控制连接复用 + 失效重建（主体在 Task 4 实现）。

use crate::engine::capabilities::CapabilitySet;
use crate::engine::transport::ImapSession;
use crate::error::MailError;

/// 池管理的会话桶：会话 + 其能力集。
pub struct PooledSession {
    pub session: Box<dyn ImapSession>,
    pub caps: CapabilitySet,
}

/// 会话工厂的池接入点（池重建时反复调用）。
#[async_trait::async_trait]
pub trait PoolConnect: Send + Sync {
    async fn connect(&self) -> Result<PooledSession, MailError>;
}
