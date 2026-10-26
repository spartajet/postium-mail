//! 进程级 tokio Runtime 单例。
//!
//! UI 层（gpui/smol 执行器）与后台调度器都从这里取 runtime，
//! 避免在无 tokio 上下文的线程上 `tokio::spawn`（"no reactor running" panic）。

use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| Runtime::new().expect("初始化 tokio runtime 失败"))
}
