//! IdleManager：每账号文件夹级 IDLE 监听（RFC 2177）。
//!
//! 结构：
//! - [`watch_changes`] / [`should_degrade`]：纯决策函数（单测覆盖）
//! - [`run_idle_loop`]：单文件夹 IDLE 循环（专属连接，`AccountEngine` 启动时 spawn）
//! - [`IdleSupervisor`]：watch 集合管理——INBOX 恒定 + 最多 [`MAX_WATCHED`]-1 个
//!   最近浏览文件夹，`WatchFolder` 后重算 diff 并启停任务
//!
//! 连接语义（async-imap 0.11 源码核实）：
//! - `Session::idle()` **消费**会话 → IDLE 连接是专属连接
//! - `Handle::init()` 发 IDLE 并等 `+` 续行；`wait`/`wait_with_timeout` 必须在
//!   `init` 之后调用（内部 assert）
//! - `wait_with_timeout` 返回 (future, `StopSource`)：**drop StopSource 只中断
//!   等待**（future 以 `ManualInterrupt` 结束），**不发 DONE**
//! - 显式发 DONE 并取回会话：`Handle::done()`（消费 Handle → 返回 `Session`），
//!   据此跨轮复用专属连接（`ImapClient::from_idle_session` 重建）

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::domain::sync::{SyncEventTx, SyncProgress, SyncProgressEvent, SyncStage};
use crate::engine::capabilities::CapabilitySet;
use crate::engine::commands::EngineCommand;
use crate::error::MailError;
use crate::infrastructure::protocols::imap::ImapClient;
use crate::infrastructure::storage::models::accounts;
use async_imap::extensions::idle::{Handle, IdleResponse};
use async_imap::imap_proto::{MailboxDatum, Response};
use tokio::sync::{mpsc, oneshot, watch};

/// IDLE 重发间隔：服务器普遍 29 分钟超时，25 分钟主动重发留足余量。
pub const IDLE_REISSUE: Duration = Duration::from_secs(25 * 60);
/// 降级轮询间隔（SyncNow(folder) 即轮询语义）。
pub const DEGRADE_POLL_INTERVAL: Duration = Duration::from_secs(90);
/// 退避基数：第 n 次连续失败退避 5s*n。
pub const BACKOFF_BASE: Duration = Duration::from_secs(5);
/// 退避上限。
pub const BACKOFF_CAP: Duration = Duration::from_secs(60);
/// watch 上限：INBOX + 最多 2 个最近浏览的非 INBOX 文件夹。
pub const MAX_WATCHED: usize = 3;

// ─────────────────────────────────────────────────────────────────────
// 纯决策函数
// ─────────────────────────────────────────────────────────────────────

/// 当前 watch 集合 → (需要开始的, 需要停止的)。
///
/// 规则：
/// - `inbox` 恒在目标集合；
/// - `requested` 按最近使用序（传入顺序）取 `max_watched - 1` 个非 inbox 补入
///   （去重、超上限截断）；
/// - 已 watch 但不在目标集合 → stop。
pub fn watch_changes(
    inbox: &str,
    requested: &[String],
    watching: &[String],
    max_watched: usize,
) -> (Vec<String>, Vec<String>) {
    // 目标集合：inbox + requested 顺序补齐（去重 / 排除 inbox / 截断）
    let mut desired: Vec<String> = vec![inbox.to_string()];
    for name in requested {
        if name == inbox || desired.iter().any(|d| d == name) {
            continue;
        }
        if desired.len() >= max_watched {
            break;
        }
        desired.push(name.clone());
    }
    let start: Vec<String> = desired
        .iter()
        .filter(|d| !watching.iter().any(|w| w == *d))
        .cloned()
        .collect();
    let stop: Vec<String> = watching
        .iter()
        .filter(|w| !desired.iter().any(|d| d == *w))
        .cloned()
        .collect();
    (start, stop)
}

/// 连续失败 ≥2 → 降级轮询。
pub fn should_degrade(consecutive_failures: u32) -> bool {
    consecutive_failures >= 2
}

/// 第 n 次连续失败的退避时长（5s*n，封顶 60s）。
fn backoff(consecutive_failures: u32) -> Duration {
    BACKOFF_CAP.min(BACKOFF_BASE.saturating_mul(consecutive_failures))
}

// ─────────────────────────────────────────────────────────────────────
// 专属连接工厂
// ─────────────────────────────────────────────────────────────────────

/// IDLE 专属连接工厂：产出**具体** [`ImapClient`]（`into_idle` 消费会话所需）
/// 与能力集（IDLE 支持探测）。
///
/// 与 [`crate::engine::pool::PoolConnect`]（池侧 `Box<dyn ImapSession>` 擦除）
/// 分离：trait object 无法回到具体会话转 IDLE。
#[async_trait::async_trait]
pub trait IdleConnect: Send + Sync {
    async fn connect_idle(&self) -> Result<(ImapClient, CapabilitySet), MailError>;
}

// ─────────────────────────────────────────────────────────────────────
// 单文件夹 IDLE 循环
// ─────────────────────────────────────────────────────────────────────

/// 单文件夹 IDLE 循环（专属连接，由 `AccountEngine` 启动时 spawn）。
///
/// - 连续失败 ≥2（[`should_degrade`]) 或服务器不支持 IDLE → 降级 90s 轮询
///   （SyncNow(folder) 即轮询语义；轮询成功后回到 IDLE 重试）；
/// - IDLE 收到 EXISTS → 进度事件 + 触发 SyncNow → done() 退出本轮 → 重进 IDLE；
/// - 25 分钟无推送（`Timeout`）→ done() → 正常重发；
/// - 失败退避 5s×次数（封顶 60s），连接销毁下轮重建；
/// - shutdown watch 置 true（或发送端全部关闭）→ 发 DONE 归还连接后退出。
pub async fn run_idle_loop(
    account: accounts::Model,
    folder: String,
    factory: Arc<dyn IdleConnect>,
    sync_tx: SyncEventTx,
    engine_tx: mpsc::Sender<EngineCommand>,
    mut shutdown: watch::Receiver<bool>,
) {
    let account_id = account.id;
    let mut consecutive_failures = 0u32;
    // 专属连接跨轮复用（`idle_round` 成功时经 done() 归还）
    let mut client: Option<ImapClient> = None;
    // 服务器 IDLE 能力：随连接探测（默认 true——失败轮会重建连接再探）。
    let mut supports_idle = true;

    loop {
        if is_shutdown(&shutdown) {
            break;
        }

        // 确保专属连接（首连或上一轮失败销毁后重建）
        if client.is_none() {
            match factory.connect_idle().await {
                Ok((c, caps)) => {
                    supports_idle = caps.idle();
                    client = Some(c);
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!(
                        account_id,
                        folder = %folder,
                        n = consecutive_failures,
                        error = %e,
                        "IDLE 专属连接失败"
                    );
                    if sleep_or_shutdown(backoff(consecutive_failures), &mut shutdown).await {
                        break;
                    }
                    continue;
                }
            }
        }

        // 降级轮询：连续失败 ≥2 或服务器不支持 IDLE
        if should_degrade(consecutive_failures) || !supports_idle {
            if poll_once(&folder, &sync_tx, &engine_tx, &mut consecutive_failures).await.is_none()
            {
                // 引擎已关闭 → 循环随之退出
                break;
            }
            if sleep_or_shutdown(DEGRADE_POLL_INTERVAL, &mut shutdown).await {
                break;
            }
            continue;
        }

        // IDLE 一轮（连接所有权交给轮函数，成功时归还）
        let taken = client.take().expect("client 刚刚确保非 None");
        match idle_round(account_id, taken, &folder, &sync_tx, &engine_tx, &mut shutdown).await {
            Ok(returned) => {
                consecutive_failures = 0;
                client = Some(returned);
            }
            Err(e) => {
                // 连接已随 Handle 丢弃（下轮重建），退避后重试
                consecutive_failures += 1;
                tracing::warn!(
                    account_id,
                    folder = %folder,
                    n = consecutive_failures,
                    error = %e,
                    "IDLE 轮失败"
                );
                if sleep_or_shutdown(backoff(consecutive_failures), &mut shutdown).await {
                    break;
                }
            }
        }
    }

    // 退出：登出剩余专属连接
    if let Some(c) = client.take() {
        let _ = c.logout().await;
    }
    tracing::info!(account_id, folder = %folder, "IDLE 循环退出");
}

/// shutdown 判定：置 true 或发送端全部关闭。
fn is_shutdown(shutdown: &watch::Receiver<bool>) -> bool {
    *shutdown.borrow() || shutdown.has_changed().is_err()
}

/// 等待 shutdown 触发（值检查优先——`changed()` 的通知可能已被上一次调用消费）。
async fn wait_for_shutdown(shutdown: &mut watch::Receiver<bool>) {
    loop {
        if is_shutdown(shutdown) {
            return;
        }
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}

/// sleep，期间 shutdown 触发则提前返回 true（应退出循环）。
async fn sleep_or_shutdown(dur: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        biased;
        _ = wait_for_shutdown(shutdown) => true,
        _ = tokio::time::sleep(dur) => is_shutdown(shutdown),
    }
}

/// 降级轮询一次：SyncNow(folder) + 回执处理。
///
/// 返回 `None` 表示引擎已关闭（应退出循环）；`Some(())` 正常（含同步失败——
/// 同步错误经事件出口上报，此处只关心轮询成功后允许回到 IDLE 重试）。
async fn poll_once(
    folder: &str,
    sync_tx: &SyncEventTx,
    engine_tx: &mpsc::Sender<EngineCommand>,
    consecutive_failures: &mut u32,
) -> Option<()> {
    let (reply_tx, reply_rx) = oneshot::channel();
    engine_tx
        .send(EngineCommand::SyncNow {
            folder: Some(folder.to_string()),
            events: Some(sync_tx.clone()),
            reply: reply_tx,
        })
        .await
        .ok()?;
    match reply_rx.await {
        Ok(Ok(())) => {
            // 轮询成功：计数清零 → 下一轮回到 IDLE 重试（失败则再次降级）
            *consecutive_failures = 0;
        }
        Ok(Err(e)) => {
            tracing::warn!(folder = %folder, error = %e, "降级轮询 SyncNow 失败");
        }
        Err(_) => return None, // 引擎已关闭
    }
    Some(())
}

/// 一轮 IDLE：into_idle → init → wait_with_timeout(25min)。
///
/// 成功（含 NewData / Timeout / shutdown 中断）→ `done()` 显式发 DONE →
/// 返回重建的客户端（专属连接跨轮复用）；连接级错误 → 连接随 Handle 丢弃，
/// 报 Err。
///
/// 借用说明：`wait_with_timeout` 的 future 持有 `&mut Handle` 且有 Drop
/// 构造，借用要到 future 销毁才结束——故把 wait/select 圈进独立作用域，
/// 块尾 future 已销毁，之后才能再借 `handle` 发 DONE。
async fn idle_round(
    account_id: i32,
    client: ImapClient,
    folder: &str,
    sync_tx: &SyncEventTx,
    engine_tx: &mpsc::Sender<EngineCommand>,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<ImapClient, MailError> {
    let mut handle: Handle<tokio_native_tls::TlsStream<tokio::net::TcpStream>> =
        client.into_idle();
    handle
        .init()
        .await
        .map_err(|e| MailError::ImapConnectionFailed(format!("IDLE init 失败: {e}")))?;

    // shutdown 可中断最长 25 分钟的等待：drop StopSource → 等待流终止，
    // wait future 以 ManualInterrupt 收尾（不发 DONE，DONE 由 finish_idle 统一发出）
    let resp = {
        let (wait_fut, stop_source) = handle.wait_with_timeout(IDLE_REISSUE);
        tokio::pin!(wait_fut);
        let shutdown_wait = wait_for_shutdown(shutdown);
        tokio::pin!(shutdown_wait);
        let outcome = tokio::select! {
            biased;
            _ = &mut shutdown_wait => {
                drop(stop_source);
                wait_fut.await
            }
            res = &mut wait_fut => res,
        };
        outcome.map_err(|e| MailError::ImapConnectionFailed(format!("IDLE 等待失败: {e}")))?
    };

    match resp {
        IdleResponse::NewData(data) => {
            let is_exists = matches!(
                data.parsed(),
                Response::MailboxData(MailboxDatum::Exists(_))
            );
            if is_exists {
                emit_new_mail_progress(account_id, folder, sync_tx);
                trigger_sync(folder, sync_tx, engine_tx).await;
            } else {
                tracing::debug!(account_id, folder = %folder, "IDLE 收到非 EXISTS 推送，重发");
            }
        }
        // 25 分钟正常重发 / shutdown 中断：无需处理，直接 DONE → 归还连接
        IdleResponse::Timeout | IdleResponse::ManualInterrupt => {}
    }

    finish_idle(handle).await
}

/// EXISTS 推送 → 进度事件（try_send：通道满/关闭静默丢弃，不阻塞 IDLE 循环）。
fn emit_new_mail_progress(account_id: i32, folder: &str, sync_tx: &SyncEventTx) {
    let _ = sync_tx.try_send(SyncProgressEvent {
        progress: SyncProgress {
            account_id,
            stage: SyncStage::SyncingEmails,
            folder: Some(folder.to_string()),
            folder_display_name: None,
            current: 0,
            total: 0,
            message: "新邮件到达".to_string(),
        },
    });
}

/// EXISTS 推送 → 触发引擎 SyncNow（回执丢弃：同步结果经事件出口上报，
/// 失败由下一轮 SyncNow/25min 重发兜底）。
async fn trigger_sync(
    folder: &str,
    sync_tx: &SyncEventTx,
    engine_tx: &mpsc::Sender<EngineCommand>,
) {
    let (reply_tx, _reply_rx) = oneshot::channel();
    let _ = engine_tx
        .send(EngineCommand::SyncNow {
            folder: Some(folder.to_string()),
            events: Some(sync_tx.clone()),
            reply: reply_tx,
        })
        .await;
}

/// 发 DONE 结束 IDLE 并取回会话，重建客户端（专属连接跨轮复用）。
async fn finish_idle(
    handle: Handle<tokio_native_tls::TlsStream<tokio::net::TcpStream>>,
) -> Result<ImapClient, MailError> {
    let session = handle
        .done()
        .await
        .map_err(|e| MailError::ImapConnectionFailed(format!("IDLE DONE 失败: {e}")))?;
    Ok(ImapClient::from_idle_session(session))
}

// ─────────────────────────────────────────────────────────────────────
// watch 管理器
// ─────────────────────────────────────────────────────────────────────

/// watch 管理器：INBOX 恒定 + 最多 [`MAX_WATCHED`]-1 个最近浏览文件夹的
/// idle 任务启停（`AccountEngine` Actor 持有，`WatchFolder` 时重算 diff）。
pub struct IdleSupervisor {
    account: accounts::Model,
    factory: Arc<dyn IdleConnect>,
    engine_tx: mpsc::Sender<EngineCommand>,
    /// 最近浏览的非 INBOX 文件夹（头部 = 最近）。
    requested: Vec<String>,
    /// 已 spawn 的任务（folder → 专属 shutdown 发送端）。
    tasks: HashMap<String, watch::Sender<bool>>,
}

impl IdleSupervisor {
    pub fn new(
        account: accounts::Model,
        factory: Arc<dyn IdleConnect>,
        engine_tx: mpsc::Sender<EngineCommand>,
    ) -> Self {
        Self {
            account,
            factory,
            engine_tx,
            requested: Vec::new(),
            tasks: HashMap::new(),
        }
    }

    fn watching(&self) -> Vec<String> {
        self.tasks.keys().cloned().collect()
    }

    /// 浏览文件夹：更新 requested（头插去重，INBOX 除外）→ 重算 diff → 启停任务。
    pub fn watch_folder(&mut self, sync_tx: &SyncEventTx, folder: String) {
        if folder != "INBOX" {
            self.requested.retain(|f| f != &folder);
            self.requested.insert(0, folder);
        }
        let (start, stop) = watch_changes("INBOX", &self.requested, &self.watching(), MAX_WATCHED);
        for name in stop {
            if let Some(tx) = self.tasks.remove(&name) {
                let _ = tx.send(true);
            }
        }
        for name in start {
            self.spawn_task(sync_tx, name);
        }
    }

    fn spawn_task(&mut self, sync_tx: &SyncEventTx, folder: String) {
        let (tx, rx) = watch::channel(false);
        self.tasks.insert(folder.clone(), tx);
        tokio::spawn(run_idle_loop(
            self.account.clone(),
            folder,
            self.factory.clone(),
            sync_tx.clone(),
            self.engine_tx.clone(),
            rx,
        ));
    }

    /// 停所有 idle 任务（引擎 Shutdown / Actor 退出时）。
    pub fn shutdown_all(&mut self) {
        for (_, tx) in self.tasks.drain() {
            let _ = tx.send(true);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| x.to_string()).collect()
    }

    // ---- watch_changes ----

    #[test]
    fn empty_start_starts_requested_with_inbox() {
        let (start, stop) = watch_changes("INBOX", &s(&["INBOX", "A"]), &[], 3);
        assert_eq!(start, s(&["INBOX", "A"]));
        assert!(stop.is_empty());
    }

    #[test]
    fn inbox_alone_watching_needs_no_change() {
        let (start, stop) = watch_changes("INBOX", &[], &s(&["INBOX"]), 3);
        assert!(start.is_empty());
        assert!(stop.is_empty());
    }

    #[test]
    fn lru_evicts_least_recently_requested() {
        // requested 按最近使用序 [B, A]，max=2 → 目标=[INBOX, B] → 停 A
        let (start, stop) =
            watch_changes("INBOX", &s(&["B", "A"]), &s(&["INBOX", "A", "B"]), 2);
        assert!(start.is_empty());
        assert_eq!(stop, s(&["A"]));
    }

    #[test]
    fn requested_inbox_duplicates_are_deduped() {
        let (start, stop) = watch_changes("INBOX", &s(&["INBOX", "INBOX", "A", "A"]), &[], 5);
        assert_eq!(start, s(&["INBOX", "A"]));
        assert!(stop.is_empty());
    }

    #[test]
    fn all_non_inbox_requested_beyond_cap_is_truncated() {
        let (start, stop) = watch_changes("INBOX", &s(&["A", "B", "C"]), &[], 2);
        // max=2 → INBOX + 1 个非 INBOX（最近使用的 A）
        assert_eq!(start, s(&["INBOX", "A"]));
        assert!(stop.is_empty());
    }

    #[test]
    fn watching_consistent_with_requested_is_noop() {
        let (start, stop) = watch_changes("INBOX", &s(&["A"]), &s(&["INBOX", "A"]), 3);
        assert!(start.is_empty());
        assert!(stop.is_empty());
    }

    #[test]
    fn stale_watching_folder_is_stopped() {
        let (start, stop) = watch_changes("INBOX", &[], &s(&["INBOX", "X"]), 3);
        assert!(start.is_empty());
        assert_eq!(stop, s(&["X"]));
    }

    #[test]
    fn max_one_watches_inbox_only() {
        let (start, stop) = watch_changes("INBOX", &s(&["A", "B"]), &s(&["INBOX", "B"]), 1);
        assert_eq!(stop, s(&["B"]));
        // 目标集合只剩 INBOX：A 不进、B 出
        assert!(start.is_empty());
    }

    // ---- should_degrade ----

    #[test]
    fn single_failure_does_not_degrade() {
        assert!(!should_degrade(0));
        assert!(!should_degrade(1));
    }

    #[test]
    fn two_consecutive_failures_degrade() {
        assert!(should_degrade(2));
        assert!(should_degrade(10));
    }

    // ---- backoff ----

    #[test]
    fn backoff_grows_linearly_and_caps() {
        assert_eq!(backoff(1), Duration::from_secs(5));
        assert_eq!(backoff(2), Duration::from_secs(10));
        assert_eq!(backoff(100), BACKOFF_CAP);
    }
}
