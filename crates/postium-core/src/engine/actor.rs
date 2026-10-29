//! AccountEngine：每账号一个 tokio Actor。

use crate::engine::commands::EngineCommand;
use crate::engine::pool::ConnectionPool;
use crate::domain::sync::SyncEventTx;
use crate::error::MailError;
use tokio::sync::mpsc;

/// 引擎运行句柄：向 Actor 发命令。
#[derive(Clone)]
pub struct EngineHandle {
    pub account_id: i32,
    tx: mpsc::Sender<EngineCommand>,
}

impl EngineHandle {
    pub async fn send(&self, cmd: EngineCommand) -> Result<(), MailError> {
        self.tx.send(cmd).await.map_err(|_| {
            MailError::ImapConnectionFailed("引擎已关闭".into())
        })
    }
}

/// 引擎主体。`F: PoolConnect` 由 Registry 装配时注入。
pub struct AccountEngine<F: crate::engine::pool::PoolConnect> {
    pub account_id: i32,
    rx: mpsc::Receiver<EngineCommand>,
    pub pool: ConnectionPool<F>,
}

impl<F: crate::engine::pool::PoolConnect> AccountEngine<F> {
    pub fn new(
        account_id: i32,
        pool: ConnectionPool<F>,
        buffer: usize,
    ) -> (EngineHandle, Self) {
        let (tx, rx) = mpsc::channel(buffer.max(16));
        (EngineHandle { account_id, tx }, Self { account_id, rx, pool })
    }

    /// 命令循环（tokio::spawn 此函数）。进度事件经事件出口转发（阶段 1 尚未产生事件）。
    pub async fn run(mut self, _events: SyncEventTx) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                EngineCommand::FetchBody { folder, uid, reply } => {
                    let result = self
                        .pool
                        .with_control(async move |s| {
                            // 阶段 1 占位：正文抓取接出现在接入点改造（Task 7）。
                            let _ = (s, &folder, uid);
                            Err(MailError::InvalidParam(
                                "FetchBody 将在接入点改造时实装".into(),
                            ))
                        })
                        .await;
                    let _ = reply.send(result);
                }
                EngineCommand::SyncNow { folder, reply } => {
                    // 阶段 2 实装；阶段 1 回执明确错误避免调用方悬挂。
                    let _ = folder;
                    let _ = reply.send(Err(MailError::InvalidParam(
                        "SyncNow 将在阶段 2 实装".into(),
                    )));
                }
                EngineCommand::Shutdown => break,
            }
        }
        self.pool.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pool::MockFactory;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn shutdown_stops_loop() {
        let count = Arc::new(AtomicUsize::new(0));
        let (handle, engine) = AccountEngine::new(
            1,
            ConnectionPool::new(Arc::new(MockFactory::new(false)), count),
            16,
        );
        let (event_tx, _event_rx) =
            smol::channel::bounded::<crate::domain::sync::SyncProgressEvent>(16);
        let task = tokio::spawn(engine.run(event_tx));
        handle.send(EngineCommand::Shutdown).await.unwrap();
        // 等命令循环处理 Shutdown 并退出（rx 被丢弃）后，通道才确定关闭：
        // 再次 send 返回 Err（引擎已停止接收）。
        task.await.unwrap();
        assert!(handle.send(EngineCommand::Shutdown).await.is_err());
    }

    #[tokio::test]
    async fn fetch_body_replies_stage1_placeholder_error() {
        let count = Arc::new(AtomicUsize::new(0));
        let (handle, engine) = AccountEngine::new(
            2,
            ConnectionPool::new(Arc::new(MockFactory::new(false)), count),
            16,
        );
        let (event_tx, _event_rx) =
            smol::channel::bounded::<crate::domain::sync::SyncProgressEvent>(16);
        let task = tokio::spawn(engine.run(event_tx));
        let (reply_tx, reply_rx) = oneshot::channel();
        handle
            .send(EngineCommand::FetchBody {
                folder: "INBOX".into(),
                uid: 1,
                reply: reply_tx,
            })
            .await
            .unwrap();
        let result = reply_rx.await.unwrap();
        assert!(result.is_err(), "阶段 1 FetchBody 为占位错误");
        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }
}
