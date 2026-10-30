//! 同步引擎：策略决策（纯函数）+ 执行（Task 4 补执行函数）。

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
