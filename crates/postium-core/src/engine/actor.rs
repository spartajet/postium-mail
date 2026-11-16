//! AccountEngine：每账号一个 tokio Actor。

use crate::domain::folders::{FolderCategory, FolderRegistry, RemoteFolder};
use crate::domain::providers::pool::PROVIDER_POOL;
use crate::domain::sync::{SyncEventTx, SyncProgress, SyncProgressEvent, SyncStage};
use crate::engine::commands::EngineCommand;
use crate::engine::idle::{IdleConnect, IdleSupervisor};
#[cfg(test)]
use crate::engine::idle::DEGRADE_POLL_INTERVAL;
use crate::engine::pool::ConnectionPool;
use crate::engine::sync::{
    FolderSyncOutcome, LocalSyncState, ServerMailboxMeta, decide_plan, execute_plan,
};
use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::models::accounts;
use crate::infrastructure::storage::repository::{account_repo, sync_repo};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

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

    /// 发送带回执的命令并等待结果（send 失败 / 引擎提前退出均归一为引擎错误）。
    pub async fn request<T>(
        &self,
        build: impl FnOnce(oneshot::Sender<Result<T, MailError>>) -> EngineCommand,
    ) -> Result<T, MailError> {
        let (tx, rx) = oneshot::channel();
        self.send(build(tx)).await?;
        rx.await
            .map_err(|_| MailError::ImapConnectionFailed("引擎已关闭".into()))?
    }
}

/// 合并两个 SyncNow 的同步范围（大师 FUN_10acbea0 挂起位语义的纯函数化）。
///
/// - `None`（全量）吸收任何范围 → 仍为 `None`；
/// - 同文件夹 → 吞并（返回合并后的当前范围，挂起位不动）；
/// - 不同文件夹 → 当前范围保留执行，新范围记入挂起位（已有挂起则新范围
///   替换——挂起位只有一个，后来的范围同样只补发一次）。
///
/// 返回 `Some(scope)` 为本轮执行范围；`None` 表示本轮范围已是全量。
fn merge_sync_scope(
    current: Option<String>,
    incoming: Option<String>,
    pending: &mut Option<Option<String>>,
) -> Option<String> {
    match (current, incoming) {
        // 全量吸收一切（含已有挂起——全量同步覆盖任何范围）
        (None, _) => {
            *pending = None;
            None
        }
        (_cur, None) => {
            *pending = None; // 来的范围是全量 → 本轮升级为全量，挂起位无意义
            None
        }
        (Some(cur), Some(next)) => {
            if cur == next {
                Some(cur)
            } else {
                // 不同文件夹：当前保留，next 进挂起位（只补发一次）
                *pending = Some(Some(next));
                Some(cur)
            }
        }
    }
}

/// 引擎主体。工厂经 `ConnectionPool` 内部 `Arc<dyn PoolConnect>` 注入
/// （池去泛型化 → `run` future 单态化，Send 证明走具体类型路径）。
pub struct AccountEngine {
    /// 引擎绑定的账号快照（SyncNow 分类回写 / 日志用）。
    pub account: accounts::Model,
    rx: mpsc::Receiver<EngineCommand>,
    /// 命令发送端克隆（IDLE 循环回发 SyncNow / WatchFolder 用）。
    self_tx: mpsc::Sender<EngineCommand>,
    pub pool: ConnectionPool,
    /// 同步落库连接（sync_state 读写 + 邮件入库由执行层完成）。
    pub db: DbConn,
    /// IDLE 专属连接工厂（生产注入；测试传 None——不启动 idle 任务）。
    idle_factory: Option<Arc<dyn IdleConnect>>,
    /// 降级轮询间隔（settings.json 可配置，默认 90s）。
    poll_interval: std::time::Duration,
}

impl AccountEngine {
    pub fn new(
        account: accounts::Model,
        pool: ConnectionPool,
        db: DbConn,
        buffer: usize,
        idle_factory: Option<Arc<dyn IdleConnect>>,
        poll_interval: std::time::Duration,
    ) -> (EngineHandle, Self) {
        let account_id = account.id;
        let (tx, rx) = mpsc::channel(buffer.max(16));
        let self_tx = tx.clone();
        (
            EngineHandle { account_id, tx },
            Self { account, rx, self_tx, pool, db, idle_factory, poll_interval },
        )
    }

    /// 命令循环（tokio::spawn 此函数）。同步进度经事件出口转发。
    ///
    /// 入口即启动 IdleManager（工厂注入时）：INBOX 恒定 watch，
    /// `WatchFolder` 命令驱动最近浏览文件夹的启停。
    pub async fn run(mut self, events: SyncEventTx) {
        let mut supervisor = self
            .idle_factory
            .take()
            .map(|f| IdleSupervisor::new(self.account.clone(), f, self.self_tx.clone(), self.poll_interval));
        if let Some(sup) = supervisor.as_mut() {
            sup.watch_folder(&events, "INBOX".to_string());
        }

        // 同文件夹 SyncNow 合并位（对照大师 FUN_10acbea0 挂起通知语义：
        // 同步进行中到达的事件只记一个挂起位，完成后补发一次，绝不堆队列）。
        // postium 等价物：执行前把通道里积压的 SyncNow 排干合并——
        // 多条 IDLE EXISTS / 轮询触发叠在一起时只做一轮 IMAP 同步。
        let mut pending_sync: Option<Option<String>> = None;
        // 排干 SyncNow 时遇到的非 SyncNow 命令（按序补执行）
        let mut deferred: std::collections::VecDeque<EngineCommand> =
            std::collections::VecDeque::new();

        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                EngineCommand::FetchBody { folder, uid, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let folder = folder.clone();
                        Box::pin(async move {
                            s.session.fetch_body(folder.as_str(), uid).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::Move { folder, uid, to_folder, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, to_folder) = (folder.clone(), to_folder.clone());
                        Box::pin(async move {
                            s.session.move_uid(folder.as_str(), uid, to_folder.as_str()).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::SetFlags { folder, uid, flags, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, flags) = (folder.clone(), flags.clone());
                        Box::pin(async move {
                            s.session.set_flags(folder.as_str(), uid, flags).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::AddFlags { folder, uid, flag, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, flag) = (folder.clone(), flag.clone());
                        Box::pin(async move {
                            s.session.add_flags(folder.as_str(), uid, flag.as_str()).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::RemoveFlags { folder, uid, flag, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, flag) = (folder.clone(), flag.clone());
                        Box::pin(async move {
                            s.session.remove_flags(folder.as_str(), uid, flag.as_str()).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::Append { folder, email, flags, message_id, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, email, flags, message_id) = (
                            folder.clone(),
                            email.clone(),
                            flags.clone(),
                            message_id.clone(),
                        );
                        Box::pin(async move {
                            s.session
                                .append(folder.as_str(), email.as_bytes(), &flags, message_id.as_deref())
                                .await
                        })
                    })
                    .await;
                    let _ = reply.send(result);
                }
                EngineCommand::Delete { folder, uid, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let folder = folder.clone();
                        Box::pin(async move {
                            s.session.delete(folder.as_str(), uid).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::FetchEmailByUid { folder, uid, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let folder = folder.clone();
                        Box::pin(async move {
                            s.session.fetch_email_by_uid(folder.as_str(), uid).await
                        })
                    }).await;
                    let _ = reply.send(result);
                }
                EngineCommand::FetchBodySection { folder, uid, section_path, reply } => {
                    let result = self.pool.with_control(&|s| {
                        let (folder, section_path) = (folder.clone(), section_path.clone());
                        Box::pin(async move {
                            s.session
                                .fetch_body_section(folder.as_str(), uid, section_path.as_str())
                                .await
                        })
                    })
                    .await;
                    let _ = reply.send(result);
                }
                EngineCommand::SyncNow { folder, events: events_override, reply } => {
                    // ── 合并积压 SyncNow（对照大师 FUN_10acbea0：挂起位 + 补发一次，不堆队列）──
                    // 排干通道中已积压的 SyncNow：None（全量）吸收 Some；不同
                    // 文件夹各留一条（记入 pending，完成后补发）；同文件夹吞并。
                    // 被吞并命令的回执通道已由发送方（IDLE trigger_sync /
                    // poll_once）主动关闭或丢弃，不会有人等它——若仍开着，
                    // 以合并轮的结果回执（无语义损失：同步结果一致）。
                    let mut scope = folder;
                    while let Ok(queued) = self.rx.try_recv() {
                        match queued {
                            EngineCommand::SyncNow { folder: next, events: _, reply: r } => {
                                if let Some(merged) = merge_sync_scope(scope.take(), next, &mut pending_sync) {
                                    scope = Some(merged);
                                }
                                // 被吞并命令的回执：发送方通常已丢弃（IDLE
                                // trigger_sync / poll_once 各自处理）；仍开着就
                                // 以合并轮结果回执，语义一致
                                let _ = r;
                            }
                            // 非 SyncNow 命令不能丢：按序补执行
                            other => deferred.push_back(other),
                        }
                    }
                    let result = match events_override {
                        Some(tx) => self.sync_folders(scope, tx).await,
                        None => self.sync_folders(scope, events.clone()).await,
                    };
                    let _ = reply.send(result);
                    // 补发挂起（清挂起位；大师 0xf9 语义：只补发一次）
                    if let Some(pending) = pending_sync.take() {
                        tracing::info!(
                            account_id = self.account.id,
                            pending = ?pending,
                            "补发挂起的 SyncNow（合并）"
                        );
                        let _ = self.sync_folders(pending, events.clone()).await;
                    }
                    // 按序执行排干时遇到的非 SyncNow 命令
                    while let Some(cmd) = deferred.pop_front() {
                        match cmd {
                            EngineCommand::WatchFolder { folder } => {
                                if let Some(sup) = supervisor.as_mut() {
                                    sup.watch_folder(&events, folder);
                                }
                            }
                            // 带回执的命令不能积压后静默丢弃：同步前排干阶段
                            // 不会遇到（排干只收 SyncNow），防御性兜底走回执
                            other => self.reply_dropped(other).await,
                        }
                    }
                }
                EngineCommand::WatchFolder { folder } => {
                    if let Some(sup) = supervisor.as_mut() {
                        sup.watch_folder(&events, folder);
                    }
                }
                EngineCommand::Shutdown => {
                    if let Some(sup) = supervisor.as_mut() {
                        sup.shutdown_all();
                    }
                    break;
                }
            }
        }
        // rx 关闭（全部发送端丢弃）同样需要停掉 idle 任务
        if let Some(sup) = supervisor.as_mut() {
            sup.shutdown_all();
        }
        self.pool.shutdown().await;
    }

    /// 排干阶段兜底：把不该出现在 deferred 里的带回执命令以引擎错误回执，
    /// 避免调用方在 oneshot 上永久挂起。正常路径不会走到（排干只吸收 SyncNow）。
    async fn reply_dropped(&mut self, cmd: EngineCommand) {
        let err = || MailError::SyncFailed("命令在 SyncNow 合并期间被丢弃".into());
        match cmd {
            EngineCommand::FetchBody { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::Move { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::SetFlags { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::AddFlags { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::RemoveFlags { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::Append { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::Delete { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::FetchEmailByUid { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::FetchBodySection { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::SyncNow { reply, .. } => {
                let _ = reply.send(Err(err()));
            }
            EngineCommand::WatchFolder { .. } | EngineCommand::Shutdown => {}
        }
    }

    /// SyncNow：同步指定文件夹（`Some`）或服务器全部可选文件夹（`None`）。
    ///
    /// 流程：LIST 发现文件夹（`None` 分支，排除 `\NoSelect`，标准类别回写
    /// `folder_category`）→ 逐文件夹发 `SyncingEmails` 进度 → EXAMINE →
    /// `decide_plan` → `execute_plan`。`Completed` 事件由执行层每文件夹发送
    /// （emit_done），此处不重复。
    ///
    /// 参数按 owned 传入：async fn 链捕获共享引用在 tokio::spawn 的 Send
    /// 证明下会触发 "not general enough"（rustc 已知限制）。
    async fn sync_folders(
        &mut self,
        folder: Option<String>,
        events: SyncEventTx,
    ) -> Result<(), MailError> {
        let account_id = self.account.id;
        let db = self.db.clone();
        tracing::info!(account_id, folder = ?folder, "SyncNow 开始");

        let folders = self.discover_folders(folder, db.clone()).await?;
        let total = folders.len();

        for (idx, target) in folders.iter().enumerate() {
            // 进度事件（try_send 与执行层 emit_done 一致：通道满 / 接收端关闭
            // 时静默丢弃，不阻塞 Actor，也不让 &Sender 借用跨 await）
            let _ = events.try_send(SyncProgressEvent {
                progress: SyncProgress {
                    account_id,
                    stage: SyncStage::SyncingEmails,
                    folder: Some(target.clone()),
                    folder_display_name: None,
                    current: idx + 1,
                    total,
                    new_emails: 0,
                    message: format!("正在同步 {target} ({}/{})", idx + 1, total),
                },
            });

            match self.sync_one_folder(db.clone(), events.clone(), target).await {
                Ok(outcome) => {
                    tracing::info!(
                        account_id,
                        folder = %target,
                        new = outcome.new_emails,
                        flags = outcome.flag_updates,
                        deleted = outcome.deleted_emails,
                        "文件夹同步完成"
                    );
                }
                Err(e) => {
                    tracing::warn!(account_id, folder = %target, error = %e, "文件夹同步失败");
                    return Err(e);
                }
            }
        }

        account_repo::update_last_sync(&db, account_id).await?;
        tracing::info!(account_id, folders = total, "SyncNow 完成");
        Ok(())
    }

    /// 文件夹集合：`Some(name)` 直接单个；`None` 走 LIST 全量（排除
    /// `\NoSelect`），并对识别为标准类别的文件夹回写 `folder_category`
    /// （与旧 dispatcher 行为一致，供侧栏分类 / get_folder_stats 解析）。
    async fn discover_folders(
        &mut self,
        folder: Option<String>,
        db: DbConn,
    ) -> Result<Vec<String>, MailError> {
        let Some(listed) = folder.map(|name| vec![name]) else {
            let listed = self
                .pool
                .with_control(&|s| Box::pin(async move { s.session.list_folders().await }))
                .await?;
            let targets: Vec<String> = listed
                .iter()
                .filter(|f| !f.no_select)
                .map(|f| f.name.clone())
                .collect();

            // 标准类别回写（沿用旧 dispatcher：供侧栏分类 / get_folder_stats 解析）。
            // 分类在无 await 区间算完（owned 对列表），避免迭代器借用跨 await
            // 触发泛型上下文下的 Send "not general enough"。
            let provider = PROVIDER_POOL
                .get()
                .and_then(|pool| pool.get(&self.account.provider));
            if let Some(provider) = provider {
                let remote: Vec<RemoteFolder> = listed
                    .iter()
                    .map(|f| RemoteFolder {
                        name: f.name.clone(),
                        special_use: f.special_use.clone(),
                        no_select: f.no_select,
                    })
                    .collect();
                let registry = FolderRegistry::builder()
                    .remote_folders(remote)
                    .provider_mapping(provider.folder_mapping())
                    .build();
                let classified: Vec<(String, FolderCategory)> = targets
                    .iter()
                    .filter_map(|t| registry.classify(t).map(|c| (t.clone(), c)))
                    .collect();
                for (target, category) in classified {
                    sync_repo::upsert_folder_category(
                        &db,
                        self.account.id,
                        target.as_str(),
                        category.as_str(),
                    )
                    .await?;
                }
                tracing::debug!(
                    account_id = self.account.id,
                    folders = ?targets,
                    "标准类别文件夹回写完成"
                );
            }
            return Ok(targets);
        };
        Ok(listed)
    }

    /// 单文件夹同步：EXAMINE → decide_plan → execute_plan（同一控制连接内）。
    async fn sync_one_folder(
        &mut self,
        db: DbConn,
        events: SyncEventTx,
        target: &str,
    ) -> Result<FolderSyncOutcome, MailError> {
        let account_id = self.account.id;
        let local = local_sync_state_from_db(&db, account_id, target).await?;
        let target = target.to_string();

        // with_control 闭包为 Fn（失败重建重试会再次调用）：捕获值在闭包体内
        // clone 后再移入 async 块
        self.pool
            .with_control(&|pooled| {
                let (db, local, events, target) = (
                    db.clone(),
                    local.clone(),
                    events.clone(),
                    target.clone(),
                );
                Box::pin(async move {
                    let meta = pooled.session.examine_folder(target.as_str()).await?;
                    let uidvalidity = meta.uid_validity.ok_or_else(|| {
                        MailError::SyncFailed(format!("EXAMINE {target} 未返回 UIDVALIDITY"))
                    })?;
                    let server = ServerMailboxMeta {
                        uidvalidity,
                        highest_modseq: meta.highest_modseq,
                        exists: meta.exists,
                    };
                    let plan = decide_plan(
                        &server,
                        &local,
                        pooled.caps.qresync(),
                        pooled.caps.condstore(),
                    );
                    tracing::debug!(account_id, folder = %target, plan = ?plan, "同步计划");
                    execute_plan(
                        &mut *pooled.session,
                        db,
                        account_id,
                        target.as_str(),
                        &plan,
                        local,
                        events,
                    )
                    .await
                })
            })
            .await
    }
}

/// 从 sync_state 行组装引擎本地状态（行不存在 → 空状态，触发 FullResync）。
async fn local_sync_state_from_db(
    db: &DbConn,
    account_id: i32,
    folder: &str,
) -> Result<LocalSyncState, MailError> {
    let state = sync_repo::get_sync_state(db, account_id, folder).await?;
    Ok(state
        .map(|s| LocalSyncState {
            uidvalidity: s.uidvalidity,
            highest_modseq: s.highest_modseq,
            last_seen_uid: s.last_sync_uid,
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pool::{MockFactory, MockSession};

    // ---- merge_sync_scope（SyncNow 合并；对照大师 FUN_10acbea0 挂起位语义）----

    #[test]
    fn merge_same_folder_is_absorbed() {
        let mut pending = None;
        let out = merge_sync_scope(Some("INBOX".into()), Some("INBOX".into()), &mut pending);
        assert_eq!(out.as_deref(), Some("INBOX"));
        assert!(pending.is_none(), "同文件夹吞并不产生挂起");
    }

    #[test]
    fn merge_full_sync_absorbs_any_scope() {
        let mut pending = Some(Some("INBOX".into())); // 已有挂起
        let out = merge_sync_scope(None, Some("INBOX".into()), &mut pending);
        assert!(out.is_none(), "全量范围保持全量");
        assert!(pending.is_none(), "全量覆盖挂起位");
    }

    #[test]
    fn merge_different_folder_defers_once() {
        let mut pending = None;
        let out = merge_sync_scope(Some("INBOX".into()), Some("Archive".into()), &mut pending);
        assert_eq!(out.as_deref(), Some("INBOX"), "当前范围保留");
        assert_eq!(
            pending.clone().flatten(),
            Some("Archive".into()),
            "不同文件夹进挂起位（只补发一次）"
        );
        // 第三个文件夹再来：挂起位被替换（仍然只有一个）
        let out2 = merge_sync_scope(Some("INBOX".into()), Some("Sent".into()), &mut pending);
        assert_eq!(out2.as_deref(), Some("INBOX"));
        assert_eq!(
            pending.clone().flatten(),
            Some("Sent".into()),
            "挂起位只有一个，后来的范围替换"
        );
    }

    #[test]
    fn merge_none_scope_from_incoming_upgrades_to_full() {
        let mut pending = None;
        let out = merge_sync_scope(Some("INBOX".into()), None, &mut pending);
        assert!(out.is_none(), "来的范围是全量 → 本轮升级为全量");
        assert!(pending.is_none());
    }

    use crate::infrastructure::protocols::imap::FolderInfo;
    use crate::infrastructure::protocols::types::WholeEmailDto;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    fn test_account(id: i32) -> accounts::Model {
        accounts::Model {
            id,
            name: format!("Account {id}"),
            email: "engine@example.com".to_string(),
            display_name: None,
            provider: "gmail".to_string(),
            imap_host: None,
            imap_port: None,
            imap_ssl: None,
            imap_ssl_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_ssl: None,
            smtp_ssl_mode: None,
            color: None,
            sync_enabled: Some(true),
            last_sync_at: None,
            auth_type: Some("Password".to_string()),
            account_type: "personal".to_string(),
            created_at: 0,
            updated_at: 0,
        }
    }

    async fn seed_account_row(db: &DbConn) {
        db.call(|conn| {
            conn.execute(
                "INSERT INTO accounts (
                    id, name, email, provider, auth_type, account_type, created_at, updated_at
                ) VALUES (1, 'Test', 'engine@example.com', 'custom', 'password', 'personal', 1, 1)",
                [],
            )?;
            Ok(())
        })
        .await
        .unwrap();
    }

    fn whole_email(uid: u32) -> WholeEmailDto {
        WholeEmailDto {
            id: 0,
            account_id: 0,
            folder: "INBOX".to_string(),
            uid,
            message_id: Some(format!("<actor-sync-{uid}@example.com>")),
            sender_name: Some("Sender".to_string()),
            sender_email: "sender@example.com".to_string(),
            recipient_emails: "to@example.com".to_string(),
            cc_emails: None,
            bcc_emails: None,
            subject: Some(format!("subject-{uid}")),
            preview: None,
            body_text: Some(format!("body {uid}")),
            body_html: None,
            attachments: vec![],
            is_read: false,
            is_starred: false,
            is_draft: false,
            is_answered: false,
            is_deleted: false,
            sent_at: 1_800_000_000 + i64::from(uid),
            received_at: 1_800_000_000 + i64::from(uid),
            created_at: 0,
        }
    }

    /// 带预设会话的工厂：SyncNow 集成测试用（LIST / SEARCH / FETCH 可控）。
    struct ScriptedSyncFactory {
        session: MockSession,
    }

    #[async_trait::async_trait]
    impl crate::engine::pool::PoolConnect for ScriptedSyncFactory {
        async fn connect(
            &self,
        ) -> Result<crate::engine::pool::PooledSession, MailError> {
            Ok(crate::engine::pool::PooledSession {
                session: Box::new(MockSession {
                    list_folders: self.session.list_folders.clone(),
                    search_uids: self.session.search_uids.clone(),
                    fetch_emails: self.session.fetch_emails.clone(),
                    ..Default::default()
                }),
                caps: crate::engine::CapabilitySet::parse(&["IMAP4REV1".into()]),
            })
        }
    }

    fn inbox_folder() -> FolderInfo {
        FolderInfo {
            name: "INBOX".to_string(),
            delimiter: None,
            flags: vec![],
            special_use: vec![],
            no_select: false,
        }
    }

    fn progress_channel() -> (
        SyncEventTx,
        smol::channel::Receiver<SyncProgressEvent>,
    ) {
        let (tx, rx) = smol::channel::unbounded::<SyncProgressEvent>();
        (tx, rx)
    }

    #[tokio::test]
    async fn shutdown_stops_loop() {
        let count = Arc::new(AtomicUsize::new(0));
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let (handle, engine) = AccountEngine::new(
            test_account(1),
            ConnectionPool::new(Arc::new(MockFactory::new(false)), count),
            db,
            16,
            None,
            DEGRADE_POLL_INTERVAL,
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
    async fn fetch_body_dispatches_to_session_and_replies() {
        let count = Arc::new(AtomicUsize::new(0));
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let (handle, engine) = AccountEngine::new(
            test_account(2),
            ConnectionPool::new(Arc::new(MockFactory::new(false)), count),
            db,
            16,
            None,
            DEGRADE_POLL_INTERVAL,
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
        assert!(result.is_ok(), "FetchBody 应分发到会话并回执成功: {result:?}");
        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }

    #[tokio::test]
    async fn move_command_dispatches_to_session() {
        let count = Arc::new(AtomicUsize::new(0));
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        let (handle, engine) = AccountEngine::new(
            test_account(3),
            ConnectionPool::new(Arc::new(MockFactory::new(false)), count),
            db,
            16,
            None,
            DEGRADE_POLL_INTERVAL,
        );
        let (event_tx, _event_rx) =
            smol::channel::bounded::<crate::domain::sync::SyncProgressEvent>(16);
        let task = tokio::spawn(engine.run(event_tx));
        let result = handle
            .request(|reply| EngineCommand::Move {
                folder: "INBOX".into(),
                uid: 7,
                to_folder: "Archive".into(),
                reply,
            })
            .await;
        assert!(result.is_ok(), "Move 应分发到会话并回执成功: {result:?}");
        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }

    /// SyncNow 集成：LIST → 逐文件夹 FullResync（空本地状态）→ 邮件入库 +
    /// sync_state 回写 + 进度/完成事件。
    #[tokio::test]
    async fn sync_now_syncs_listed_folders_and_emits_progress() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        seed_account_row(&db).await;

        let factory = ScriptedSyncFactory {
            session: MockSession {
                list_folders: vec![inbox_folder()],
                search_uids: vec![5],
                fetch_emails: vec![whole_email(5)],
                ..Default::default()
            },
        };
        let (handle, engine) = AccountEngine::new(
            test_account(1),
            ConnectionPool::new(Arc::new(factory), Arc::new(AtomicUsize::new(0))),
            db.clone(),
            16,
            None,
            DEGRADE_POLL_INTERVAL,
        );
        let (event_tx, event_rx) = progress_channel();
        let task = tokio::spawn(engine.run(event_tx.clone()));

        let result = handle
            .request(|reply| EngineCommand::SyncNow { folder: None, events: None, reply })
            .await;
        assert!(result.is_ok(), "SyncNow 应回执成功: {result:?}");

        // 邮件入库（fetch_emails_by_uids → save_batch_emails）
        let email = crate::infrastructure::storage::repository::email_repo::get_by_uid(
            &db, 1, "INBOX", 5,
        )
        .await
        .unwrap()
        .expect("uid 5 应已入库");
        assert_eq!(email.body_text.as_deref(), Some("body 5"));

        // sync_state 回写（FullResync：uidvalidity + last_seen_uid + modseq）
        let state = crate::infrastructure::storage::repository::sync_repo::get_sync_state(
            &db, 1, "INBOX",
        )
        .await
        .unwrap()
        .expect("sync_state 应已回写");
        assert_eq!(state.uidvalidity, Some(1));
        assert_eq!(state.last_sync_uid, Some(5));

        // 进度事件：SyncingEmails(INBOX) + Completed(INBOX)
        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
        drop(event_tx);
        let mut stages = Vec::new();
        while let Ok(event) = event_rx.try_recv() {
            stages.push((event.progress.stage, event.progress.folder));
        }
        assert!(
            stages.iter().any(|(stage, folder)| matches!(stage, SyncStage::SyncingEmails)
                && folder.as_deref() == Some("INBOX")),
            "应收到 INBOX 的 SyncingEmails 进度: {stages:?}"
        );
        assert!(
            stages.iter().any(|(stage, folder)| matches!(stage, SyncStage::Completed)
                && folder.as_deref() == Some("INBOX")),
            "应收到 INBOX 的 Completed 事件: {stages:?}"
        );
    }

    /// SyncNow LIST 后的 SPECIAL-USE 角色回写：文件夹名字不在关键词表、也不在
    /// provider 候选名里，仅凭服务器 \Sent 标注也应把 `folder_category` 落库。
    #[tokio::test]
    async fn sync_now_persists_special_use_folder_category() {
        crate::domain::providers::pool::init_provider_pool();
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        seed_account_row(&db).await;

        let factory = ScriptedSyncFactory {
            session: MockSession {
                list_folders: vec![FolderInfo {
                    name: "SysVault".into(),
                    delimiter: None,
                    flags: vec![],
                    special_use: vec![crate::domain::folders::special_use::SpecialUseFlag::Sent],
                    no_select: false,
                }],
                ..Default::default()
            },
        };
        let (handle, engine) = AccountEngine::new(
            test_account(1),
            ConnectionPool::new(Arc::new(factory), Arc::new(AtomicUsize::new(0))),
            db.clone(),
            16,
            None,
            DEGRADE_POLL_INTERVAL,
        );
        let (event_tx, _event_rx) = progress_channel();
        let task = tokio::spawn(engine.run(event_tx));

        let result = handle
            .request(|reply| EngineCommand::SyncNow { folder: None, events: None, reply })
            .await;
        assert!(result.is_ok(), "SyncNow 应回执成功: {result:?}");

        let state = crate::infrastructure::storage::repository::sync_repo::get_sync_state(
            &db, 1, "SysVault",
        )
        .await
        .unwrap()
        .expect("SPECIAL-USE 文件夹应已写入 sync_state");
        assert_eq!(
            state.folder_category.as_deref(),
            Some("sent"),
            "服务器 \\Sent 标注应回写为 sent 类别"
        );

        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }

    /// SyncNow 指定单文件夹：即使服务器 LIST 不含该文件夹也应同步。
    #[tokio::test]
    async fn sync_now_with_explicit_folder_skips_listing() {
        let db = DbConn::open_in_memory_for_test().await.unwrap();
        seed_account_row(&db).await;

        let factory = ScriptedSyncFactory {
            session: MockSession {
                // LIST 返回空（服务器未报告），指定文件夹仍应被同步
                list_folders: vec![],
                search_uids: vec![3],
                fetch_emails: vec![whole_email(3)],
                ..Default::default()
            },
        };
        let (handle, engine) = AccountEngine::new(
            test_account(1),
            ConnectionPool::new(Arc::new(factory), Arc::new(AtomicUsize::new(0))),
            db.clone(),
            16,
            None,
            DEGRADE_POLL_INTERVAL,
        );
        let (event_tx, _event_rx) = progress_channel();
        let task = tokio::spawn(engine.run(event_tx));

        let result = handle
            .request(|reply| EngineCommand::SyncNow {
                folder: Some("Archive".into()),
                events: None,
                reply,
            })
            .await;
        assert!(result.is_ok(), "指定文件夹 SyncNow 应回执成功: {result:?}");
        let email = crate::infrastructure::storage::repository::email_repo::get_by_uid(
            &db, 1, "Archive", 3,
        )
        .await
        .unwrap()
        .expect("Archive uid 3 应已入库");
        assert_eq!(email.body_text.as_deref(), Some("body 3"));

        handle.send(EngineCommand::Shutdown).await.unwrap();
        task.await.unwrap();
    }
}
