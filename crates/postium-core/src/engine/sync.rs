//! 同步引擎：策略决策（纯函数）+ 执行（面向 `ImapSession` + DB）。

use std::collections::HashSet;

use crate::domain::sync::{
    SyncEventTx, SyncProgress, SyncProgressEvent, SyncStage, HISTORY_UID_BATCH_SIZE,
    whole_emails_to_contacts,
};
use crate::domain::sync::folder_sync_full::delete_account_folder_emails;
use crate::engine::transport::ImapSession;
use crate::error::MailError;
use crate::infrastructure::protocols::types::WholeEmailDto;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::repository::{email_repo, sync_repo};

/// 每文件夹的同步策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncPlan {
    /// UIDVALIDITY 变化（或无本地状态）：清空本地 + 全量重同步。
    FullResync,
    /// QRESYNC：SELECT 参数化，一次往返拿 flag 变更 + vanished。
    QresyncResync,
    /// CONDSTORE：CHANGEDSINCE 拉 flag 变更 + UID 集合对比 vanished。
    CondstoreResync,
    /// UID 增量：last_seen_uid 之后拉新 + UID 集合对比 vanished。
    UidIncremental,
}

/// 本地持久化状态（来自 sync_state 行，可能为空）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalSyncState {
    pub uidvalidity: Option<u32>,
    pub highest_modseq: Option<u64>,
    pub last_seen_uid: Option<u32>,
}

/// 服务器 SELECT/EXAMINE 元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMailboxMeta {
    pub uidvalidity: u32,
    pub highest_modseq: Option<u64>,
    pub exists: u32,
}

/// 决策同步计划。
///
/// - 本地无状态 或 uidvalidity != 服务器 → FullResync
/// - 本地与服务器都有 modseq → QRESYNC 可用则 QresyncResync，否则
///   CONDSTORE 可用则 CondstoreResync
/// - 无 modseq 支持 → 本地有 last_seen_uid 则 UidIncremental，否则 FullResync
pub fn decide_plan(
    server: &ServerMailboxMeta,
    local: &LocalSyncState,
    caps_qresync: bool,
    caps_condstore: bool,
) -> SyncPlan {
    let uidvalidity_changed = local.uidvalidity.is_some_and(|v| v != server.uidvalidity);
    if local.uidvalidity.is_none() || uidvalidity_changed {
        return SyncPlan::FullResync;
    }
    let both_have_modseq = local.highest_modseq.is_some() && server.highest_modseq.is_some();
    if caps_qresync && both_have_modseq {
        return SyncPlan::QresyncResync;
    }
    if caps_condstore && both_have_modseq {
        return SyncPlan::CondstoreResync;
    }
    if local.last_seen_uid.is_some() {
        SyncPlan::UidIncremental
    } else {
        SyncPlan::FullResync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(uv: u32, modseq: Option<u64>, exists: u32) -> ServerMailboxMeta {
        ServerMailboxMeta { uidvalidity: uv, highest_modseq: modseq, exists }
    }
    fn local(uv: Option<u32>, modseq: Option<u64>, uid: Option<u32>) -> LocalSyncState {
        LocalSyncState { uidvalidity: uv, highest_modseq: modseq, last_seen_uid: uid }
    }

    #[test]
    fn no_local_state_is_full_resync() {
        assert_eq!(decide_plan(&server(1, Some(5), 10), &local(None, None, None), true, true), SyncPlan::FullResync);
    }

    #[test]
    fn uidvalidity_change_is_full_resync() {
        assert_eq!(decide_plan(&server(2, Some(9), 10), &local(Some(1), Some(5), Some(3)), true, true), SyncPlan::FullResync);
    }

    #[test]
    fn qresync_when_capable_and_state_present() {
        assert_eq!(decide_plan(&server(1, Some(9), 10), &local(Some(1), Some(5), Some(3)), true, true), SyncPlan::QresyncResync);
    }

    #[test]
    fn condstore_when_no_qresync() {
        assert_eq!(decide_plan(&server(1, Some(9), 10), &local(Some(1), Some(5), Some(3)), false, true), SyncPlan::CondstoreResync);
    }

    #[test]
    fn uid_incremental_when_no_modseq_support() {
        assert_eq!(decide_plan(&server(1, None, 10), &local(Some(1), None, Some(3)), false, false), SyncPlan::UidIncremental);
    }

    #[test]
    fn full_resync_when_no_modseq_and_no_local_uid() {
        assert_eq!(decide_plan(&server(1, None, 10), &local(Some(1), None, None), false, false), SyncPlan::FullResync);
    }

    #[test]
    fn falls_back_to_uid_incremental_when_server_has_no_modseq_but_client_has_state() {
        // 本地有 modseq（曾连过支持 CONDSTORE 的服务器）但当前服务器不报 → 降级 UID 增量
        assert_eq!(decide_plan(&server(1, None, 10), &local(Some(1), Some(5), Some(3)), true, true), SyncPlan::UidIncremental);
    }
}

// ─────────────────────────────────────────────────────────────────────
// 执行层：四档同步策略落地
// ─────────────────────────────────────────────────────────────────────

/// 单文件夹同步执行结果。
#[derive(Debug, Default, Clone)]
pub struct FolderSyncOutcome {
    /// 新入库（含 upsert）的邮件数。
    pub new_emails: usize,
    /// 远端 flag 变更落库的行数。
    pub flag_updates: usize,
    /// vanished 清理物理删除的邮件数。
    pub deleted_emails: usize,
}

/// 抓取新邮件并入库的回调（由调用方注入，保持 engine 与入库细节解耦）：
/// 按 UID 集合批量拉取完整邮件，返回 DTO 列表；落库（save_batch_emails +
/// 联系人收集）由执行函数内部完成。
pub type FetchNewEmails<'a> = &'a (dyn Fn(
        &mut dyn ImapSession,
        &str,
        &[u32],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Vec<WholeEmailDto>, MailError>> + Send + 'a>,
    > + Send
    + Sync);

/// 执行同步计划（面向 `ImapSession` + DB）。
///
/// 按 [`SyncPlan`] 分派到四档执行体：
/// - `FullResync`：EXAMINE → 清空本地文件夹 → 全量抓取入库 → 状态回写
/// - `QresyncResync` / `CondstoreResync`：CHANGEDSINCE flag 同步 + vanished
///   清理 + 新邮件补抓 → 状态回写（含服务器当前 highest_modseq）
/// - `UidIncremental`：仅抓取 `last_seen_uid` 之后的新邮件 → 状态回写
#[allow(clippy::too_many_arguments)]
pub async fn execute_plan(
    session: &mut dyn ImapSession,
    db: DbConn,
    account_id: i32,
    folder: &str,
    plan: &SyncPlan,
    local: &LocalSyncState,
    fetch_new_emails: FetchNewEmails<'_>,
    events: &crate::domain::sync::SyncEventTx,
) -> Result<FolderSyncOutcome, MailError> {
    match plan {
        SyncPlan::FullResync => {
            execute_full(session, db, account_id, folder, fetch_new_emails, events).await
        }
        SyncPlan::QresyncResync => {
            execute_modseq_resync(session, db, account_id, folder, local, fetch_new_emails, events, true).await
        }
        SyncPlan::CondstoreResync => {
            execute_modseq_resync(session, db, account_id, folder, local, fetch_new_emails, events, false).await
        }
        SyncPlan::UidIncremental => {
            execute_uid_incremental(session, db, account_id, folder, local, fetch_new_emails, events).await
        }
    }
}

/// 本地状态齐全时发起 QRESYNC 参数化 SELECT（缺 uidvalidity / modseq 则静默跳过，
/// 由后续 EXAMINE 兜底）。
async fn select_qresync_with_local_state(
    session: &mut dyn ImapSession,
    folder: &str,
    local: &LocalSyncState,
) -> Result<(), MailError> {
    if let (Some(uidvalidity), Some(modseq)) = (local.uidvalidity, local.highest_modseq) {
        session.select_qresync(folder, uidvalidity, modseq).await?;
    }
    Ok(())
}

/// 分批抓取新邮件 → 入库 → 联系人收集（失败不阻断同步）。
///
/// 返回成功落库的行数。
async fn fetch_and_store(
    session: &mut dyn ImapSession,
    db: &DbConn,
    account_id: i32,
    folder: &str,
    uids: &[u32],
    fetch_new_emails: FetchNewEmails<'_>,
) -> Result<usize, MailError> {
    let mut inserted = 0usize;
    for group in uids.chunks(HISTORY_UID_BATCH_SIZE) {
        let emails = fetch_new_emails(session, folder, group).await?;
        inserted += email_repo::save_batch_emails(db, account_id, folder, &emails).await?;
        // 落库成功后把发件人（含 sender_name）/ 收件人 / 抄送并入联系人表
        whole_emails_to_contacts(db, &emails).await;
    }
    Ok(inserted)
}

/// UID 增量：`uid_search_all` 后按 `last_seen_uid` 过滤，只补抓新 UID。
///
/// 状态回写 `upsert_sync_state(db, account, folder, None, None, Some(max_uid),
/// None)`——uidvalidity/modseq 传 `None`（COALESCE 保留原值）。
async fn execute_uid_incremental(
    session: &mut dyn ImapSession,
    db: DbConn,
    account_id: i32,
    folder: &str,
    local: &LocalSyncState,
    fetch_new_emails: FetchNewEmails<'_>,
    events: &SyncEventTx,
) -> Result<FolderSyncOutcome, MailError> {
    let last_seen_uid = local.last_seen_uid.unwrap_or(0);
    let server_uids = session.uid_search_all(folder).await?;
    let new_uids: Vec<u32> = server_uids
        .iter()
        .copied()
        .filter(|uid| *uid > last_seen_uid)
        .collect();

    let inserted =
        fetch_and_store(session, &db, account_id, folder, &new_uids, fetch_new_emails).await?;

    let max_uid = new_uids.iter().max().copied().unwrap_or(last_seen_uid);
    // modseq None = COALESCE 保留原值
    sync_repo::upsert_sync_state(&db, account_id, folder, None, None, Some(max_uid), None).await?;

    let outcome = FolderSyncOutcome {
        new_emails: inserted,
        ..Default::default()
    };
    emit_done(events, account_id, folder, &outcome);
    Ok(outcome)
}

/// QRESYNC 与 CONDSTORE 共用执行体。
///
/// `qresync_only = true` 时先带本地 uidvalidity/modseq 走 QRESYNC 参数化
/// SELECT（flag 变更 / VANISHED 由响应附带），随后 `examine_folder` 二次确认
/// 最新 meta；`false` 时直接 EXAMINE。之后依次：
///
/// 1. flag 变更：CHANGEDSINCE 逐条按 UID 覆盖 is_read / is_starred
/// 2. vanished：服务器 UID 集合 vs 本地集合 → 差集物理删除
/// 3. 新邮件：`last_seen_uid` 之后补抓入库（同增量路径）
/// 4. 状态回写（含服务器当前 highest_modseq）
#[allow(clippy::too_many_arguments)]
async fn execute_modseq_resync(
    session: &mut dyn ImapSession,
    db: DbConn,
    account_id: i32,
    folder: &str,
    local: &LocalSyncState,
    fetch_new_emails: FetchNewEmails<'_>,
    events: &SyncEventTx,
    qresync_only: bool,
) -> Result<FolderSyncOutcome, MailError> {
    // 1. 选文件夹：QRESYNC 参数化或 EXAMINE（都返回最新 uidvalidity / modseq）
    if qresync_only {
        select_qresync_with_local_state(session, folder, local).await?;
    }
    let meta = session.examine_folder(folder).await?;
    let server_modseq = meta.highest_modseq;

    let mut outcome = FolderSyncOutcome::default();

    // 2. flag 变更（本地无 modseq 时跳过，等价于退化为增量+vanished）
    if let Some(since_modseq) = local.highest_modseq {
        let changed = session
            .fetch_flags_changed_since(folder, "1:*", since_modseq)
            .await?;
        for (uid, flags) in &changed {
            outcome.flag_updates +=
                email_repo::update_flags_by_uid(&db, account_id, folder, *uid, flags).await?;
        }
    }

    // 3. vanished：服务器有、本地无 → 服务器为准；本地有、服务器无 → 清理
    let server_uids = session.uid_search_all(folder).await?;
    let server_set: HashSet<u32> = server_uids.iter().copied().collect();
    let local_uids = email_repo::list_uids_by_folder(&db, account_id, folder).await?;
    let vanished: Vec<u32> = local_uids
        .into_iter()
        .filter(|uid| !server_set.contains(uid))
        .collect();
    outcome.deleted_emails =
        email_repo::delete_by_uids(&db, account_id, folder, &vanished).await? as usize;

    // 4. 新邮件：last_seen_uid 之后的新 UID 补抓入库
    let last_seen_uid = local.last_seen_uid.unwrap_or(0);
    let new_uids: Vec<u32> = server_uids
        .iter()
        .copied()
        .filter(|uid| *uid > last_seen_uid)
        .collect();
    outcome.new_emails =
        fetch_and_store(session, &db, account_id, folder, &new_uids, fetch_new_emails).await?;

    // 5. 状态回写（highest_modseq = 服务器当前值）
    let max_uid = server_uids.iter().max().copied().or(local.last_seen_uid);
    sync_repo::upsert_sync_state(
        &db,
        account_id,
        folder,
        meta.uid_next,
        meta.uid_validity,
        max_uid,
        server_modseq,
    )
    .await?;

    emit_done(events, account_id, folder, &outcome);
    Ok(outcome)
}

/// 全量重同步：EXAMINE → 清空本地文件夹 → 全量抓取入库 → 状态回写。
///
/// 相比旧 `folder_sync_full` 的 uidvalidity 窗口 / 时间窗口过滤，引擎版
/// 简化为"该文件夹全量"（窗口决策由上层做）。
async fn execute_full(
    session: &mut dyn ImapSession,
    db: DbConn,
    account_id: i32,
    folder: &str,
    fetch_new_emails: FetchNewEmails<'_>,
    events: &SyncEventTx,
) -> Result<FolderSyncOutcome, MailError> {
    // 1. EXAMINE 取 meta
    let meta = session.examine_folder(folder).await?;

    // 2. 清空本地该文件夹邮件（含标签 / 附件元数据，照旧全量路径用法）
    let deleted = delete_account_folder_emails(db.clone(), account_id, folder).await?;

    // 3. 全量抓取入库
    let server_uids = session.uid_search_all(folder).await?;
    let inserted =
        fetch_and_store(session, &db, account_id, folder, &server_uids, fetch_new_emails).await?;

    // 4. 状态回写（uidvalidity + last_seen_uid=最大 + highest_modseq）
    sync_repo::upsert_sync_state(
        &db,
        account_id,
        folder,
        meta.uid_next,
        meta.uid_validity,
        server_uids.iter().max().copied(),
        meta.highest_modseq,
    )
    .await?;

    let outcome = FolderSyncOutcome {
        new_emails: inserted,
        flag_updates: 0,
        deleted_emails: deleted,
    };
    emit_done(events, account_id, folder, &outcome);
    Ok(outcome)
}

/// 文件夹同步完成后向前端发一条进度事件（通道满 / 接收端关闭时静默丢弃）。
fn emit_done(events: &SyncEventTx, account_id: i32, folder: &str, outcome: &FolderSyncOutcome) {
    let processed = outcome.new_emails + outcome.flag_updates + outcome.deleted_emails;
    let _ = events.try_send(SyncProgressEvent {
        progress: SyncProgress {
            account_id,
            stage: SyncStage::Completed,
            folder: Some(folder.to_string()),
            folder_display_name: Some(folder.to_string()),
            current: processed,
            total: processed,
            message: format!(
                "同步完成: 新增 {} 标志更新 {} 删除 {}",
                outcome.new_emails, outcome.flag_updates, outcome.deleted_emails
            ),
        },
    });
}
