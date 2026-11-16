# IMAP 引擎 · 阶段 2（同步引擎）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 三档同步策略（QRESYNC → CONDSTORE → UID 增量）落地 `engine/sync.rs`，替换 `domain/sync/` 旧同步层（dispatcher/full/increment，保留 scheduler 触发入口），同步状态持久化到 `sync_state` 表（加 `highest_modseq` 列）。

**Architecture:** `SyncEngine` 是纯决策+执行结构：输入（服务器能力集、SELECT 元数据、本地 `sync_state`）→ 输出 `FolderSyncPlan`（枚举：QresyncResync/CondstoreResync/UidIncremental/FullResync）→ 按计划执行并回写状态。决策函数为纯函数（可无 IO 单测）；执行函数面向 `trait ImapSession`。旧 `sync_service::sync_account_with_range` 公共签名不变，内部改走引擎；`AccountEngine::SyncNow` 实装。

**Tech Stack:** 现有 `engine/`（Task 1-6 产物：CapabilitySet/ImapSession/ConnectionPool/AccountEngine/registry）、rusqlite migration、现有 `save_batch_emails`/`whole_emails_to_contacts` 入库函数。

**设计文档:** `docs/superpowers/specs/2026-10-29-imap-engine-design.md` §4/§8。

**对 spec 的两处实现级修正**（写码时遵循，勿照抄 spec 字面）：
1. spec 说"新表 `folder_sync_state`"——实际 `sync_state` 表已有 `account_id/folder/uidvalidity/uidnext/last_sync_uid`，**ALTER TABLE 加 `highest_modseq INTEGER` 列**即可，不新建表。
2. spec 说 FETCH 带 BODYSTRUCTURE"解析失败自动回退原文"——流层解析失败无法容错（163 实证：TakeWhile1 炸整批）。**维持不带 BODYSTRUCTURE**（现有 whole 路径：FLAGS+INTERNALDATE+BODY.PEEK[]+UID，附件/头部从原文 mail_parser 解析）——spec 的意图（结构化信息）由 mail_parser 达成。

---

## File Structure

```
修改 crates/postium-core/src/engine/transport.rs     # +examine_folder/qresync_select/uid_search/changed_since_flags
修改 crates/postium-core/src/infrastructure/protocols/imap/mod.rs  # +底层方法（examine/qresync/changedsince）
修改 crates/postium-core/src/infrastructure/storage/repository/sync_repo.rs  # +highest_modseq 列与读写
修改 crates/postium-core/src/infrastructure/storage/database.rs      # ALTER TABLE migration
新建 crates/postium-core/src/engine/sync.rs           # SyncPlan 决策（纯函数）+ SyncEngine 执行
修改 crates/postium-core/src/engine/actor.rs          # SyncNow 实装（调 SyncEngine）
修改 crates/postium-core/src/service/sync_service.rs  # sync_account_with_range/sync_all 走引擎
删除 crates/postium-core/src/domain/sync/folder_sync_dispatcher.rs
删除 crates/postium-core/src/domain/sync/folder_sync_full.rs
删除 crates/postium-core/src/domain/sync/folder_sync_increment.rs
修改 crates/postium-core/src/domain/sync/mod.rs       # 移除删除模块的声明；保留 SyncProgress/Scheduler 引用
修改 crates/postium-core/src/domain/sync/scheduler.rs # 触发入口改走引擎（保留定时逻辑）
测试：engine/sync.rs #[cfg(test)]（决策纯函数矩阵）+ 新 integration tests/engine_sync.rs（mock session 全流程）
```

---

### Task 1: transport 扩展——EXAMINE/QRESYNC/CHANGEDSINCE/UID SEARCH

**Files:**
- Modify: `crates/postium-core/src/infrastructure/protocols/imap/mod.rs`（底层）
- Modify: `crates/postium-core/src/engine/transport.rs`（trait + RealSession）
- Modify: `crates/postium-core/src/engine/pool.rs`（MockSession 补方法）

- [ ] **Step 1.1: 底层 ImapClient 三方法**

`imap/mod.rs` 的 `impl ImapClient` 追加（async-imap 0.11 API；`Session::examine` 存在；QRESYNC SELECT 参数用 `run_command_and_check_ok` 裸命令 + 手动解析 untagged 响应——参考 `select_folder` 现有实现如何从响应提取 MailboxInfo，照同一模式）：

```rust
    /// EXAMINE（只读）选择文件夹，返回状态（含 UIDVALIDITY/UIDNEXT/HIGHESTMODSEQ）。
    pub async fn examine_folder(&mut self, folder: &str) -> Result<MailboxInfo, MailError> {
        tracing::debug!(folder, "IMAP: EXAMINE 文件夹");
        let mailbox = self
            .session
            .examine(folder)
            .await
            .map_err(|e| MailError::FolderNotFound(format!("EXAMINE '{folder}' 失败: {e}")))?;
        Ok(MailboxInfo {
            exists: mailbox.exists as u32,
            recent: mailbox.recent as u32,
            unseen: mailbox.unseen,
            uid_validity: mailbox.uid_validity,
            uid_next: mailbox.uid_next,
        })
    }
```

**MailboxInfo 需要加 `highest_modseq: Option<u64>` 字段**——`select_folder` 与 `examine_folder` 都从 mailbox（async-imap `Mailbox` 类型有 `highest_modseq` 属性，`session` 的 untagged 数据里可取）填充；`select_folder` 现有实现处同步补。若 async-imap 的 Mailbox 不直接暴露 HIGHESTMODSEQ，改从 `session` 的响应流过滤 `HIGHESTMODSEQ (n)`（参考 fetch.rs 处理 untagged 的模式）。以编译与实跑为准。

QRESYNC SELECT（带参数的 SELECT 在 async-imap 无直接方法，用裸命令）：

```rust
    /// 带 QRESYNC 参数的 SELECT：服务器一次性返回 flag 变更与 vanished。
    /// 响应由调用方经 `read_qresync_responses` 解析。
    pub async fn select_qresync(
        &mut self,
        folder: &str,
        uidvalidity: u32,
        modseq: u64,
    ) -> Result<(), MailError> {
        let cmd = format!("SELECT {folder} (QRESYNC (uidvalidity {modseq} 1,1))");
        // 注意：folder 含空格等需 IMAP 字符串引号——用现有 validate/quote 惯例
        self.run_command_and_check_ok(&cmd).await
    }

    /// 读取当前连接上累积的 untagged 响应流（QRESYNC/CHANGEDSINCE 后调用），
    /// 直到 tagged OK。返回原始响应文本供上层解析。
    pub async fn drain_untagged(&mut self) -> Result<Vec<String>, MailError> {
        let mut out = Vec::new();
        while let Some(res) = self.session.read_response().await.map_err(|e| {
            MailError::ImapConnectionFailed(format!("读取响应失败: {e}"))
        })? {
            out.push(format!("{res:?}"));
            // tagged OK 结束本轮（协议层识别由解析函数处理）
        }
        Ok(out)
    }
```

**实现提示（重要）**：上面 `drain_untagged` 的"直到 tagged OK"判断需要访问响应解析——
如果 `read_response` + `format!("{res:?}")` 过于粗糙，改为照抄 `imap/parser.rs` 或
`fetch.rs` 处理 untagged FETCH 的既有模式（用 `imap_proto::Response` 匹配
`Response::Done` 判结束、`Response::Fetch`/`Response::Vanished`/`Response::Ids`
取数据）。**以能正确区分数据与 tagged 结束为准**，这是本任务的核心难点。

CHANGEDSINCE flag 拉取：

```rust
    /// FETCH uid_set (FLAGS UID) (CHANGEDSINCE modseq)——CONDSTORE 档用。
    /// 返回 (uid, flags) 列表。
    pub async fn fetch_flags_changed_since(
        &mut self,
        folder: &str,
        uid_set: &str,
        modseq: u64,
    ) -> Result<Vec<(u32, Vec<String>)>, MailError> {
        self.select_folder(folder).await?;
        let cmd = format!("UID FETCH {uid_set} (UID FLAGS) (CHANGEDSINCE {modseq})");
        self.run_command_and_check_ok(&cmd).await?;
        // 解析：照 fetch.rs 的 fetch_uids / fetch_flags 的 untagged 处理模式，
        // 从 read_response 流取 UID+FLAGS 对，直到 tagged OK。
    }
```

解析部分照抄 `fetch.rs` 里 `fetch_flags`（429 行文件内）的响应处理模式——先读它。

- [ ] **Step 1.2: trait ImapSession 扩展**

`transport.rs` 的 trait 追加（RealSession 转发；MockSession 同步补）：

```rust
    /// EXAMINE（只读）文件夹，返回状态。
    async fn examine_folder(&mut self, folder: &str) -> Result<MailboxInfo, MailError>;
    /// QRESYNC 参数化 SELECT（随后用 drain/解析方法取变更）。
    async fn select_qresync(&mut self, folder: &str, uidvalidity: u32, modseq: u64) -> Result<(), MailError>;
    /// CHANGEDSINCE flag 拉取（CONDSTORE 档）。
    async fn fetch_flags_changed_since(&mut self, folder: &str, uid_set: &str, modseq: u64) -> Result<Vec<(u32, Vec<String>)>, MailError>;
    /// UID SEARCH（全量 UID 列表，vanished 对比与 UID 增量档用）。
    async fn uid_search_all(&mut self, folder: &str) -> Result<Vec<u32>, MailError>;
```

`uid_search_all` 底层：现有 `list_all_uids`（fetch/search.rs 有）转发。
MockSession 全部返回空/默认值（engine/sync.rs 的测试用自己的 mock 扩展，见 Task 3）。

- [ ] **Step 1.3: 编译验证**

```bash
cargo build -p postium-core
```

预期：Finished（底层解析实现以实跑为准——需要真实服务器验证的部分在集成测试标记，CI 用 mock 路径）。

- [ ] **Step 1.4: 提交**

```bash
git add crates/postium-core/src/engine/transport.rs crates/postium-core/src/infrastructure/protocols/imap/mod.rs crates/postium-core/src/engine/pool.rs
git commit -m "feat(engine): transport 扩展——EXAMINE/QRESYNC/CHANGEDSINCE/UID SEARCH"
```

---

### Task 2: sync_state 表加 highest_modseq

**Files:**
- Modify: `crates/postium-core/src/infrastructure/storage/database.rs`（migration）
- Modify: `crates/postium-core/src/infrastructure/storage/repository/sync_repo.rs`（读写）
- Modify: `crates/postium-core/src/infrastructure/storage/models/sync_state.rs`（Model 字段）

- [ ] **Step 2.1: migration**

`database.rs` 的 migration 区域（找到现有 ALTER/CREATE 的 migration 惯例，照同一模式）：

```rust
        // IMAP 引擎：sync_state 增加 highest_modseq（CONDSTORE/QRESYNC 增量状态）
        conn.execute_batch(
            "ALTER TABLE sync_state ADD COLUMN highest_modseq INTEGER;",
        ).map_err(|e| {
            // 列已存在时忽略（duplicate column），其余上抛
            let _ = e; // duplicate column name: highest_modseq → 幂等
        }).ok();
```

（若现有 migration 有版本化机制，照该机制；否则幂等 ALTER + 忽略重复列错误，
与仓库现有模式一致——先读 database.rs 确认。）

- [ ] **Step 2.2: Model + repo 读写**

`sync_state.rs` Model 加 `pub highest_modseq: Option<u64>`；`sync_repo.rs` 的
`map_sync_state`/`upsert_sync_state`/`get_sync_state`/`update_sync_state` 同步
加列读写（照现有列的写法）。

- [ ] **Step 2.3: 验证 + 提交**

```bash
cargo test -p postium-core sync_repo
cargo build -p postium-core
git add -A crates/postium-core/src/infrastructure
git commit -m "feat(engine): sync_state 表增加 highest_modseq 列"
```

预期：schema 测试（tests/schema.rs 若断言列集合需同步更新）与既有测试通过。

---

### Task 3: SyncPlan 决策纯函数（矩阵单测）

**Files:**
- Create: `crates/postium-core/src/engine/sync.rs`

决策输入输出（纯函数，无 IO）：

```rust
//! 同步引擎：策略决策（纯函数）+ 执行。

/// 每文件夹的同步策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncPlan {
    /// UIDVALIDITY 变化（或无本地状态且文件夹非空）：清空本地 + 全量重同步。
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
/// - 本地有 highest_modseq 且服务器也报了 → QRESYNC 可用则 QresyncResync，
///   否则 CondstoreResync
/// - 服务器无 modseq（老服务器）→ 本地有 last_seen_uid 则 UidIncremental，
///   否则 FullResync
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
    let local_modseq = local.highest_modseq;
    let server_modseq = server.highest_modseq;
    if caps_qresync && local_modseq.is_some() && server_modseq.is_some() {
        return SyncPlan::QresyncResync;
    }
    if caps_condstore && local_modseq.is_some() && server_modseq.is_some() {
        return SyncPlan::CondstoreResync;
    }
    if local.last_seen_uid.is_some() {
        SyncPlan::UidIncremental
    } else {
        SyncPlan::FullResync
    }
}
```

- [ ] **Step 3.1: 写决策矩阵测试（先于实现）**

测试矩阵（全放 `engine/sync.rs` 的 `#[cfg(test)] mod tests`）：

```rust
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
```

- [ ] **Step 3.2: 实现决策函数使测试通过**

```bash
cargo test -p postium-core engine::sync
```

预期：7 个决策测试 PASS。

- [ ] **Step 3.3: 提交**

```bash
git add crates/postium-core/src/engine/sync.rs
git commit -m "feat(engine): SyncPlan 决策纯函数——三档策略矩阵"
```

---

### Task 4: SyncEngine 执行——三档策略落地

**Files:**
- Modify: `crates/postium-core/src/engine/sync.rs`（执行函数）
- Modify: `crates/postium-core/src/engine/transport.rs`（如 Task 1 方法签名需调）

- [ ] **Step 4.1: 执行入口**

```rust
/// 单文件夹同步执行结果。
#[derive(Debug, Default, Clone)]
pub struct FolderSyncOutcome {
    pub new_emails: usize,
    pub flag_updates: usize,
    pub deleted_emails: usize,
}

/// 执行同步计划（面向 ImapSession + DB）。
///
/// `fetch_new_emails`：按 UID 集合批量拉取完整邮件并入库（内部调用现有
/// transport 的 batch 抓取 + `save_batch_emails` + `whole_emails_to_contacts`），
/// 由调用方闭包注入以保持 engine 与入库细节解耦。
pub async fn execute_plan(
    session: &mut dyn ImapSession,
    db: crate::infrastructure::storage::DbConn,
    account_id: i32,
    folder: &str,
    plan: &SyncPlan,
    local: &LocalSyncState,
    fetch_new_emails: &dyn Fn(&mut dyn ImapSession, &str, &[u32]) -> std::future::Future<Output = Result<Vec<crate::infrastructure::protocols::imap::WholeEmailDto>, MailError>> + Send,
) -> Result<FolderSyncOutcome, MailError> {
    match plan {
        SyncPlan::FullResync => execute_full(session, db, account_id, folder, fetch_new_emails).await,
        SyncPlan::QresyncResync => execute_qresync(session, db, account_id, folder, local, fetch_new_emails).await,
        SyncPlan::CondstoreResync => execute_condstore(session, db, account_id, folder, local, fetch_new_emails).await,
        SyncPlan::UidIncremental => execute_uid_incremental(session, db, account_id, folder, local, fetch_new_emails).await,
    }
}
```

四个 execute_* 的语义：

- **execute_full**：SELECT → `uid_search_all` → `fetch_new_emails(全部 UID)` →
  `email_repo::delete_by_account_folder_tx` 等价的清空（参照
  `folder_sync_full.rs` 的 `delete_account_folder_emails` 用法）→ 入库 →
  状态回写（uidvalidity/last_seen_uid=最大 UID，modseq 若服务器报了也写）。
- **execute_qresync**：`select_qresync` → 解析响应（flag 变更：`FETCH uid (FLAGS)`；
  vanished：UID 列表）→ flag 更新走现有 `email_repo` 的 flag 更新函数（找到
  `email_repo` 中按 message_id/uid 更新 is_read/is_starred 的既有函数并复用）→
  vanished 删除走 `email_repo::delete_emails_by_uids`（若无此函数，加一个
  DELETE FROM emails WHERE account_id=? AND folder=? AND uid IN (...)）→ 新邮件
  （QRESYNC 不返回新邮件，需要再跑一次 UID 增量补 `last_seen_uid` 之后的部分）
  → 状态回写。
- **execute_condstore**：`fetch_flags_changed_since` → flag 更新；UID 集合对比
  vanished（`uid_search_all` vs 本地 UID 集——本地 UID 集从 emails 表查：
  `email_repo` 加 `list_uids_by_folder(account_id, folder)` 若无）→ UID 增量补新。
- **execute_uid_incremental**：逻辑照抄 `folder_sync_increment.rs`（list_uids_since_uid
  → batch fetch → 入库 → 联系人 → upsert 状态），但走 trait session。

**实现要点**：flag 更新与删除的 DB 函数若 `email_repo` 缺失就补；`whole_emails_to_contacts`
保持调用；每档完成都回写 `sync_repo::upsert_sync_state`（含 highest_modseq=服务器
当前值，UidIncremental 档写 None）。

- [ ] **Step 4.2: 集成测试（mock session 全流程）**

`tests/engine_sync.rs`（新集成测试）：

```rust
//! SyncEngine 执行集成测试：mock session 三档策略。
use postium_core::engine::sync::{decide_plan, execute_plan, LocalSyncState, ServerMailboxMeta, SyncPlan};
// MockSession 扩展版：可控 UID 列表/flags/vanished 响应
// 断言：入库邮件数、flag 更新数、删除数、状态回写值
```

（Mock 需要访问 DB 与入库闭包——测试用真实 tempdir DB + MockSession 的组合；
mock 的 `fetch_new_emails` 闭包返回固定 WholeEmailDto。每档一个测试：
FullResync 清空重灌、UidIncremental 只补新增、CondstoreResync flag 更新+删除、
QresyncResync 因响应解析依赖真实服务器格式在集成层用 Condstore 等价路径覆盖，
真实 QRESYNC 响应留给 163/Outlook 真机回归。）

- [ ] **Step 4.3: 运行**

```bash
cargo test -p postium-core engine_sync
```

预期：各档测试 PASS。

- [ ] **Step 4.4: 提交**

```bash
git add crates/postium-core/src/engine/sync.rs crates/postium-core/tests/engine_sync.rs crates/postium-core/src/infrastructure/storage/repository/email_repo.rs
git commit -m "feat(engine): SyncEngine 执行——三档策略落地"
```

---

### Task 5: AccountEngine::SyncNow 实装 + 替换旧 sync 层

**Files:**
- Modify: `crates/postium-core/src/engine/actor.rs`（SyncNow 实装：列出文件夹 → 逐文件夹 decide_plan + execute_plan，发 Progress 事件）
- Modify: `crates/postium-core/src/engine/actor.rs`（AccountEngine 增加 db/auth 字段——构造由 Registry 传入）
- Modify: `crates/postium-core/src/service/sync_service.rs`（`sync_account_with_range`/`sync_all_accounts` 内部改：ensure_engine → 发 SyncNow 命令 → 等回执；公共签名不变）
- Delete: `crates/postium-core/src/domain/sync/folder_sync_dispatcher.rs`
- Delete: `crates/postium-core/src/domain/sync/folder_sync_full.rs`
- Delete: `crates/postium-core/src/domain/sync/folder_sync_increment.rs`
- Modify: `crates/postium-core/src/domain/sync/mod.rs`（移除模块声明；保留 SyncProgress/Scheduler/history）
- Modify: `crates/postium-core/src/domain/sync/scheduler.rs`（定时触发入口改走引擎 SyncNow）

- [ ] **Step 5.1: SyncNow 实装**

`AccountEngine` 增加字段 `db: crate::infrastructure::storage::DbConn`（Registry 的
`ensure_engine` 增加 `db` 参数，CoreApi::imap_engine 传 `self.db.clone()`）。
`SyncNow` 处理逻辑：

```rust
EngineCommand::SyncNow { folder, reply } => {
    let result = self.sync_folders(folder.as_deref()).await;
    let _ = reply.send(result);
}
```

`sync_folders(&mut self, folder: Option<&str>) -> Result<(), MailError>`：
1. `with_control` → `list_folders`（或指定 folder 单独处理）
2. 对每个文件夹：EXAMINE → `decide_plan` → `execute_plan`（fetch_new_emails
   闭包内复用现有 `batch_fetch_emails_by_uids` + `save_batch_emails` +
   `whole_emails_to_contacts` 链）
3. 每文件夹发 Progress 事件（`SyncProgress { account_id, stage: SyncingEmails, .. }`
   经 `_events` 出口——Task 5 骨架的 `_events` 改回 `events` 启用）

- [ ] **Step 5.2: sync_service 改走引擎**

`sync_service.rs` 的 `sync_account_with_range(event_tx, account_id, range)`：
内部改为 ensure_engine（auth/db 已在 self）→ WatchFolder/获取账号模型 → 发
SyncNow { folder: None, reply } → await。`range` 参数保留签名（首同步窗口在
引擎的 FullResync 路径应用——把 window 传给 SyncNow 变体或暂忽略并在注释
说明：窗口逻辑由 sync_state 首次状态为空时的 FullResync + 现有
`window_for_initial_range` 在 full 路径继续生效）。**保持公共签名不变**。

`sync_all_accounts` 循环各账号发 SyncNow。

`sync_service` 的测试（`sync_all_accounts_with_progress_*` 等带 mock runner
的测试）：runner 模式被引擎替代后这些测试需重写为引擎 mock 版或删除——
按最小破坏原则：保留函数但 runner 注入点改为注入 SyncNow 回执成功/失败的
mock 引擎（测试改写允许）。

- [ ] **Step 5.3: 删除旧层 + scheduler 改造**

删除三个文件；`mod.rs` 移除声明；`scheduler.rs` 的定时触发回调改为调用
引擎 SyncNow（scheduler 持有 registry/db 引用——照其现有构造调整）；
`history.rs` 若仅被删除模块引用则一并删除，否则保留。

- [ ] **Step 5.4: 编译 + 全量测试 + 提交**

```bash
cargo build -p postium-core -p postium-ui
cargo test --workspace 2>&1 | grep -cE "FAILED"   # 预期 0
git add -A
git commit -m "feat(engine): SyncNow 实装——三档策略接入 AccountEngine，旧同步层退役"
```

（旧 sync 层的集成测试 `tests/engine_sync.rs` 之外，`tests/sync_history.rs`/
`tests/all_accounts_sync.rs` 若测的是被删模块，改写为引擎版或删除——
以"测试的是行为而非实现"为准改写。）

---

### Task 6: 全量验证与真机回归指引

- [ ] **Step 6.1: 全 workspace 测试 + clippy**

```bash
cargo test --workspace 2>&1 | grep -cE "FAILED"   # 0
cargo clippy -p postium-core -p postium-ui --all-targets 2>&1 | grep -cE "^error"  # 0
```

- [ ] **Step 6.2: 真机回归（手动，列出清单）**

163（ID 命令/授权码/无 modseq → UID 增量档）、Outlook（OAuth2/QRESYNC 视
服务器支持）、Gmail（QRESYNC 全特性）——验证：首次全量、二次增量秒级、
标已读双向同步、删除同步、断网重连恢复。

- [ ] **Step 6.3: 提交推送**

```bash
git push -u origin feat/imap-engine
```

阶段 3（IDLE + 文件夹 registry + UI 事件接线）另立计划。
