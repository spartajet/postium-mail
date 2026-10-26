//! tokio Runtime 桥（单例在 postium-core，进程共享）。
//!
//! gpui 的执行器是 smol；tokio 服务的 Future 通过 JoinHandle
//! （实现 `Future + Send`，可被任意执行器 await）桥接。
//! UI 的全部异步服务调用必须经 [`spawn_tokio`]，禁止在 UI 线程 block_on。

pub fn runtime() -> &'static tokio::runtime::Runtime {
    postium_core::rt::runtime()
}

/// 在 tokio 上执行 `fut`，返回可被 gpui 执行器 await 的接收端。
pub fn spawn_tokio<F>(fut: F) -> smol::channel::Receiver<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = smol::channel::bounded(1);
    runtime().spawn(async move {
        let out = fut.await;
        let _ = tx.try_send(out);
    });
    rx
}
