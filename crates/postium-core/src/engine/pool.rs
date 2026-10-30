//! 连接池：控制连接复用 + 失效重建（主体在 Task 4 实现）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// 连接池：控制连接复用；命令失败时销毁重连一次，仍失败则向上报错。
pub struct ConnectionPool<F: PoolConnect> {
    factory: Arc<F>,
    control: Option<PooledSession>,
    /// 已执行的连接次数（测试断言复用/重建行为）。
    connect_count: Arc<AtomicUsize>,
}

impl<F: PoolConnect> ConnectionPool<F> {
    pub fn new(factory: Arc<F>, connect_count: Arc<AtomicUsize>) -> Self {
        Self { factory, control: None, connect_count }
    }

    /// 获取控制连接执行命令：无连接则新建；命令失败时销毁重建一次重试。
    pub async fn with_control<R>(
        &mut self,
        f: impl AsyncFn(&mut PooledSession) -> Result<R, MailError>,
    ) -> Result<R, MailError> {
        if self.control.is_none() {
            self.control = Some(self.factory.connect().await?);
            self.connect_count.fetch_add(1, Ordering::SeqCst);
        }
        let session = self.control.as_mut().expect("control 刚刚建立");
        match f(session).await {
            Ok(r) => Ok(r),
            Err(e) => {
                // 命令失败：连接可疑 → 销毁重建一次重试
                tracing::warn!(error = %e, "控制连接命令失败，重建连接重试");
                self.control = None;
                let mut fresh = self.factory.connect().await?;
                self.connect_count.fetch_add(1, Ordering::SeqCst);
                let r = f(&mut fresh).await?;
                self.control = Some(fresh);
                Ok(r)
            }
        }
    }

    /// 关闭池（登出控制连接）。
    pub async fn shutdown(&mut self) {
        if let Some(pooled) = self.control.take() {
            let _ = pooled.session.logout().await;
        }
    }
}

// ---- 测试用 Mock（同文件，供 tests 模块与后续阶段复用） ----

pub struct MockSession {
    pub fail_select_with: Option<MailError>,
}

#[async_trait::async_trait]
impl ImapSession for MockSession {
    async fn capabilities(&mut self) -> Result<Vec<String>, MailError> {
        Ok(vec!["IMAP4REV1".into()])
    }
    async fn enable(&mut self, _caps: &[&str]) -> Result<(), MailError> {
        Ok(())
    }
    async fn select_folder(
        &mut self,
        _folder: &str,
    ) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError> {
        match &self.fail_select_with {
            Some(e) => Err(MailError::ImapConnectionFailed(e.to_string())),
            None => Ok(crate::infrastructure::protocols::imap::MailboxInfo {
                exists: 0,
                recent: 0,
                unseen: None,
                uid_validity: Some(1),
                uid_next: Some(1),
                highest_modseq: None,
            }),
        }
    }
    async fn examine_folder(
        &mut self,
        _folder: &str,
    ) -> Result<crate::infrastructure::protocols::imap::MailboxInfo, MailError> {
        Ok(crate::infrastructure::protocols::imap::MailboxInfo {
            exists: 0,
            recent: 0,
            unseen: None,
            uid_validity: Some(1),
            uid_next: Some(1),
            highest_modseq: None,
        })
    }
    async fn select_qresync(
        &mut self,
        _folder: &str,
        _uidvalidity: u32,
        _modseq: u64,
    ) -> Result<(), MailError> {
        Ok(())
    }
    async fn fetch_flags_changed_since(
        &mut self,
        _folder: &str,
        _uid_set: &str,
        _modseq: u64,
    ) -> Result<Vec<(u32, Vec<String>)>, MailError> {
        Ok(Vec::new())
    }
    async fn uid_search_all(&mut self, _folder: &str) -> Result<Vec<u32>, MailError> {
        Ok(Vec::new())
    }
    async fn logout(self: Box<Self>) -> Result<(), MailError> {
        Ok(())
    }
    async fn fetch_body(&mut self, _folder: &str, _uid: u32) -> Result<String, MailError> {
        Ok(String::new())
    }
    async fn move_uid(&mut self, _folder: &str, _uid: u32, _to_folder: &str) -> Result<(), MailError> {
        Ok(())
    }
    async fn set_flags(
        &mut self,
        _folder: &str,
        _uid: u32,
        _flags: Vec<String>,
    ) -> Result<(), MailError> {
        Ok(())
    }
    async fn add_flags(&mut self, _folder: &str, _uid: u32, _flag: &str) -> Result<(), MailError> {
        Ok(())
    }
    async fn remove_flags(&mut self, _folder: &str, _uid: u32, _flag: &str) -> Result<(), MailError> {
        Ok(())
    }
    async fn append(
        &mut self,
        _folder: &str,
        _email: &[u8],
        _flags: &[String],
        _message_id: Option<&str>,
    ) -> Result<u32, MailError> {
        Ok(1)
    }
    async fn delete(&mut self, _folder: &str, _uid: u32) -> Result<(), MailError> {
        Ok(())
    }
    async fn fetch_email_by_uid(
        &mut self,
        _folder: &str,
        _uid: u32,
    ) -> Result<Option<crate::infrastructure::protocols::types::WholeEmailDto>, MailError> {
        Ok(None)
    }
    async fn fetch_body_section(
        &mut self,
        _folder: &str,
        _uid: u32,
        _section_path: &str,
    ) -> Result<Option<crate::infrastructure::protocols::types::FetchedBodySection>, MailError>
    {
        Ok(None)
    }
}

pub struct MockFactory {
    /// 第一次连接产出的会话在 select 时失败（模拟陈旧连接），其后正常。
    pub first_select_fails: bool,
    connects: AtomicUsize,
}

impl MockFactory {
    pub fn new(first_select_fails: bool) -> Self {
        Self { first_select_fails, connects: AtomicUsize::new(0) }
    }
}

#[async_trait::async_trait]
impl PoolConnect for MockFactory {
    async fn connect(&self) -> Result<PooledSession, MailError> {
        let n = self.connects.fetch_add(1, Ordering::SeqCst);
        let fail = self.first_select_fails && n == 0;
        Ok(PooledSession {
            session: Box::new(MockSession {
                fail_select_with: fail.then(|| MailError::ImapConnectionFailed("stale".into())),
            }),
            caps: CapabilitySet::parse(&["IMAP4REV1".into()]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reuses_control_connection_when_healthy() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut pool = ConnectionPool::new(
            Arc::new(MockFactory::new(false)),
            count.clone(),
        );
        for _ in 0..3 {
            pool.with_control(async |s| {
                s.session.select_folder("INBOX").await.map(|_| ())
            })
            .await
            .unwrap();
        }
        assert_eq!(count.load(Ordering::SeqCst), 1, "健康连接应复用");
    }

    #[tokio::test]
    async fn rebuilds_once_on_stale_connection_and_retries() {
        let count = Arc::new(AtomicUsize::new(0));
        let mut pool = ConnectionPool::new(
            Arc::new(MockFactory::new(true)),
            count.clone(),
        );
        pool.with_control(async |s| {
            s.session.select_folder("INBOX").await.map(|_| ())
        })
        .await
        .unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2, "陈旧连接应重建一次并重试成功");
    }
}
