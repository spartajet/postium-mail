//! 引擎注册表：账号 id → 引擎句柄（进程级单例）。

use crate::engine::actor::{AccountEngine, EngineHandle};
use crate::engine::pool::{ConnectionPool, PoolConnect};
use crate::domain::sync::SyncEventTx;
use crate::infrastructure::storage::models::accounts;
use crate::error::MailError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

static REGISTRY: OnceLock<Mutex<HashMap<i32, EngineHandle>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<i32, EngineHandle>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 确保某账号的引擎在运行（已运行则返回句柄克隆，否则启动新引擎）。
pub fn ensure_engine<F>(
    account: &accounts::Model,
    factory: Arc<F>,
    events: SyncEventTx,
) -> Result<EngineHandle, MailError>
where
    F: PoolConnect + 'static,
{
    let mut map = registry().lock().expect("registry 锁");
    if let Some(h) = map.get(&account.id) {
        return Ok(h.clone());
    }
    let pool = ConnectionPool::new(factory, Arc::new(std::sync::atomic::AtomicUsize::new(0)));
    let (handle, engine) = AccountEngine::new(account.id, pool, 32);
    tokio::spawn(engine.run(events));
    map.insert(account.id, handle.clone());
    Ok(handle)
}
