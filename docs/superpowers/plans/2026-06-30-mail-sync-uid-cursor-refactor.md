# 邮件同步 UID 游标重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将历史回填收敛为固定数量 UID 游标分页，并修正普通增量同步的 UID 范围和统计结果。

**Architecture:** 新邮件同步只使用 `last_sync_uid` 高水位游标，旧邮件回填只使用 `history_before_uid` 低水位游标。历史回填每次最多拉取 `HISTORY_UID_BATCH_SIZE = 50` 封更早邮件，`history_synced_since` 仅作为展示/诊断字段，不参与控制流程。前端文案改为“加载更早邮件”，普通刷新入口统一经过 `SyncState`。

**Tech Stack:** Rust 2024, Tauri v2, rusqlite/tokio-rusqlite, async-imap, Specta/Tauri Specta bindings, Svelte 5 runes, Vitest, Bun.

## Global Constraints

- 文档和用户可见说明用中文书写。
- 没有明确指令，不提交代码；本计划中的任务不执行 `git commit`。
- Shell 命令使用 `rtk` 前缀。
- 历史回填批量大小固定为常量 `HISTORY_UID_BATCH_SIZE = 50`，方便将来修改。
- 本轮不做复杂同步队列、不做多账号/多文件夹调度重构、不删除 `history_synced_since` 字段。
- 历史回填不得调用 Full Sync，不得删除本地文件夹内容。
- Svelte 文件变更后运行 Svelte autofixer 或至少运行 `bun run check`。
- Tauri 新增或变更 command 后必须确认 `src-tauri/src/lib.rs` 的 Specta command 收集仍正确。

---

## File Structure

后端文件：

- Modify: `src-tauri/src/domain/sync/mod.rs`
  - 导出 `HISTORY_UID_BATCH_SIZE` 常量，并新增纯函数辅助历史游标判断。
- Modify: `src-tauri/src/infrastructure/protocols/imap/search.rs`
  - 把增量 UID 查询从固定 `+100` 改为 `UID start:*`。
  - 保持历史回填 `UID 1:(before_uid - 1)` 并按常量截取最后 N 个 UID。
- Modify: `src-tauri/src/domain/sync/folder_sync_increment.rs`
  - 使用完整新增 UID 列表，按批次精确 fetch，返回真实 `SyncResult`。
- Modify: `src-tauri/src/domain/sync/folder_sync_full.rs`
  - 首次同步完成后初始化 `history_before_uid`。
  - 确认历史回填不依赖 Full Sync。
- Modify: `src-tauri/src/service/sync_service.rs`
  - 历史回填只用 `history_before_uid` 推进。
  - `history_synced_since` 只更新展示值。
  - 修正耗尽判断和错误推进规则。
- Modify: `src-tauri/src/infrastructure/storage/repository/email_repo.rs`
  - 如需要，补充按 UID 精确保存/统计所需函数。
- Test: `src-tauri/tests/sync_history.rs`
  - 增加历史游标状态测试。
- Test: `src-tauri/tests/email_repository.rs` 或 `src-tauri/tests/sync_history.rs`
  - 增加保存邮件头幂等统计测试。

前端文件：

- Modify: `src/lib/i18n/zh-CN.ts`
  - 文案改为“加载更早邮件”“正在加载更早邮件...”。
- Modify: `src/lib/i18n/en-US.ts`
  - 文案改为 `Load older mail`、`Loading older mail...`。
- Modify: `src/lib/components/email/EmailList.svelte`
  - 使用新文案，保持按钮显示规则。
  - 普通刷新通过 `SyncState` 执行。
- Modify: `src/lib/stores/email.svelte.ts`
  - 移除或改造直接调用 `commands.syncAccount` 的刷新入口。
- Modify: `src/lib/stores/sync.svelte.ts`
  - 确认历史状态缓存和回填加载状态符合新语义。
- Test: `src/lib/__tests__/components/EmailList.test.ts`
  - 更新按钮文案和点击行为断言。
- Test: `src/lib/__tests__/stores/email.test.ts`
  - 更新刷新逻辑测试。
- Test: `src/lib/__tests__/stores/sync-history.test.ts`
  - 保持历史状态和回填命令调用测试。

---

### Task 1: 后端常量与 UID 游标纯函数

**Files:**
- Modify: `src-tauri/src/domain/sync/mod.rs`
- Test: `src-tauri/src/domain/sync/mod.rs`

**Interfaces:**
- Produces: `pub const HISTORY_UID_BATCH_SIZE: usize = 50`
- Produces: `pub fn history_exhausted_before_uid(before_uid: u32) -> bool`
- Produces: `pub fn next_history_before_uid(batch: &[u32], previous_before_uid: u32) -> u32`

- [ ] **Step 1: 添加失败测试**

在 `src-tauri/src/domain/sync/mod.rs` 的测试模块中加入：

```rust
#[test]
fn history_batch_size_should_default_to_fifty() {
    assert_eq!(HISTORY_UID_BATCH_SIZE, 50);
}

#[test]
fn history_exhausted_before_uid_should_be_true_for_first_uid() {
    assert!(history_exhausted_before_uid(1));
    assert!(history_exhausted_before_uid(0));
    assert!(!history_exhausted_before_uid(2));
}

#[test]
fn next_history_before_uid_should_move_to_minimum_batch_uid() {
    assert_eq!(next_history_before_uid(&[41, 39, 40], 50), 39);
}

#[test]
fn next_history_before_uid_should_keep_previous_when_batch_empty() {
    assert_eq!(next_history_before_uid(&[], 50), 50);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml history_`

Expected: FAIL，提示常量或函数未定义。

- [ ] **Step 3: 实现最小代码**

在 `src-tauri/src/domain/sync/mod.rs` 的类型定义附近添加：

```rust
pub const HISTORY_UID_BATCH_SIZE: usize = 50;

pub fn history_exhausted_before_uid(before_uid: u32) -> bool {
    before_uid <= 1
}

pub fn next_history_before_uid(batch: &[u32], previous_before_uid: u32) -> u32 {
    batch.iter().copied().min().unwrap_or(previous_before_uid)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml history_`

Expected: PASS。

---

### Task 2: IMAP UID 搜索命令收敛

**Files:**
- Modify: `src-tauri/src/infrastructure/protocols/imap/search.rs`

**Interfaces:**
- Consumes: `HISTORY_UID_BATCH_SIZE`
- Produces: `ImapClient::list_uids_since_uid(folder, uid_since) -> Result<Vec<u32>, MailError>` 查询 `UID uid_since:*`
- Produces: `ImapClient::list_uids_before_uid(folder, before_uid, limit) -> Result<Vec<u32>, MailError>` 保持按最后 N 个 UID 返回

- [ ] **Step 1: 更新搜索命令测试**

在 `src-tauri/src/infrastructure/protocols/imap/search.rs` 的测试模块中加入或替换：

```rust
#[test]
fn build_uid_since_search_command_should_request_all_newer_uids() {
    assert_eq!(
        build_uid_since_search_command(42).as_deref(),
        Some("UID 42:*")
    );
}

#[test]
fn build_uid_since_search_command_should_return_none_for_zero() {
    assert_eq!(build_uid_since_search_command(0), None);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml build_uid_since_search_command`

Expected: FAIL，提示函数不存在。

- [ ] **Step 3: 实现命令构造函数并替换增量查询**

在 `search.rs` 增加：

```rust
fn build_uid_since_search_command(uid_since: u32) -> Option<String> {
    (uid_since >= 1).then(|| format!("UID {uid_since}:*"))
}
```

将 `list_uids_since_uid` 中：

```rust
let search_cmd = format!("UID {}:{}", uid_since, uid_since + 100);
```

替换为：

```rust
let Some(search_cmd) = build_uid_since_search_command(uid_since) else {
    return Ok(Vec::new());
};
```

并调用：

```rust
.uid_search(&search_cmd)
```

- [ ] **Step 4: 运行 IMAP 搜索测试**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml build_uid_since_search_command`

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml build_uid_before_search_command_should_request_uids_below_cursor`

Expected: PASS。

---

### Task 3: 增量同步真实统计与完整 UID 范围

**Files:**
- Modify: `src-tauri/src/domain/sync/folder_sync_increment.rs`
- Modify: `src-tauri/src/infrastructure/protocols/imap/fetch.rs`
- Test: `src-tauri/src/domain/sync/folder_sync_increment.rs`

**Interfaces:**
- Consumes: `ImapClient::list_uids_since_uid(... UID start:*)`
- Produces: `SyncResult { new_emails, updated_emails, deleted_emails: 0, duration_ms: 0 }` 使用真实计数
- Produces: `ImapClient::batch_fetch_emails_by_uids(folder, uids) -> Result<Vec<WholeEmailDto>, MailError>`

- [ ] **Step 1: 为精确 UID fetch 添加接口测试目标**

在 `fetch.rs` 里新增精确 UID 集合构造函数测试：

```rust
#[test]
fn build_uid_set_should_join_exact_uids() {
    assert_eq!(build_uid_set(&[9, 3, 7]), Some("9,3,7".to_string()));
}

#[test]
fn build_uid_set_should_return_none_for_empty_input() {
    assert_eq!(build_uid_set(&[]), None);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml build_uid_set`

Expected: FAIL，提示 `build_uid_set` 未定义。

- [ ] **Step 3: 添加精确 UID fetch 方法**

在 `fetch.rs` 增加：

```rust
fn build_uid_set(uids: &[u32]) -> Option<String> {
    if uids.is_empty() {
        return None;
    }
    Some(uids.iter().map(u32::to_string).collect::<Vec<_>>().join(","))
}
```

执行一次机械抽取：

1. 将现有 `pub async fn batch_fetch_emails(&mut self, folder: &str, start_uid: u32, end_uid: u32)` 重命名为 `async fn fetch_whole_emails_by_uid_set(&mut self, folder: &str, uid_set: &str)`。
2. 删除新私有函数开头的 `let uid_range = format!("{}:{}", start_uid, end_uid);`。
3. 把 tracing 字段里的 `uid_range` 改为 `uid_set`。
4. 把 `.uid_fetch(&uid_range, ...)` 改为 `.uid_fetch(uid_set, ...)`。
5. 在私有函数后重新添加公共范围方法：

```rust
pub async fn batch_fetch_emails(
    &mut self,
    folder: &str,
    start_uid: u32,
    end_uid: u32,
) -> Result<Vec<WholeEmailDto>, MailError> {
    let uid_range = format!("{start_uid}:{end_uid}");
    self.fetch_whole_emails_by_uid_set(folder, &uid_range).await
}
```

6. 添加精确 UID 集合公共方法：

```rust
pub async fn batch_fetch_emails_by_uids(
    &mut self,
    folder: &str,
    uids: &[u32],
) -> Result<Vec<WholeEmailDto>, MailError> {
    let Some(uid_set) = build_uid_set(uids) else {
        return Ok(Vec::new());
    };
    self.fetch_whole_emails_by_uid_set(folder, &uid_set).await
}
```

- [ ] **Step 4: 修改增量同步使用精确 UID 集合**

在 `sync_folder_incremental` 中将批量 fetch 改为按 `new_uids.chunks(50)` 精确拉取：

```rust
let mut inserted = 0usize;
let mut fetched = 0usize;

for group in new_uids.chunks(crate::domain::sync::HISTORY_UID_BATCH_SIZE) {
    let new_emails = imap_client.batch_fetch_emails_by_uids(folder, group).await?;
    fetched += new_emails.len();
    inserted += save_batch_emails(&db, account_id, folder, &new_emails).await?;
}
```

返回：

```rust
Ok(SyncResult {
    new_emails: inserted,
    updated_emails: fetched,
    deleted_emails: 0,
    duration_ms: 0,
})
```

- [ ] **Step 5: 运行后端测试**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml build_uid_set`

Expected: PASS。

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml sync_history`

Expected: PASS。

---

### Task 4: 历史回填只按 `history_before_uid` 推进

**Files:**
- Modify: `src-tauri/src/service/sync_service.rs`
- Modify: `src-tauri/src/domain/sync/folder_sync_full.rs`
- Modify: `src-tauri/tests/sync_history.rs`

**Interfaces:**
- Consumes: `HISTORY_UID_BATCH_SIZE`, `history_exhausted_before_uid`, `next_history_before_uid`
- Produces: `history_before_uid_for_folder(db, account_id, folder, state) -> Result<u32, MailError>`
- Produces: 历史回填成功后更新 `history_before_uid = min(uid_batch)`

- [ ] **Step 1: 增加历史状态测试**

在 `src-tauri/tests/sync_history.rs` 增加：

```rust
#[tokio::test]
async fn history_state_should_use_existing_history_before_uid() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    db.call(|conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, uidvalidity, uidnext, last_sync_uid,
                history_before_uid, history_exhausted, synced_at, created_at, updated_at
            ) VALUES (1, 'INBOX', 1, 200, 199, 80, 0, 1, 1, 1)",
            [],
        )?;
        Ok(())
    }).await.unwrap();

    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service.get_history_state(1, EmailCategory::Inbox).await.unwrap();

    assert_eq!(result.history_before_uid, Some(80));
    assert!(!result.history_exhausted);
}

#[tokio::test]
async fn history_state_should_report_exhausted_when_state_is_exhausted() {
    init_provider_pool();
    let db = DbConn::open_in_memory_for_test().await.unwrap();
    seed_account(&db).await;
    db.call(|conn| {
        conn.execute(
            "INSERT INTO sync_state (
                account_id, folder, uidvalidity, uidnext, last_sync_uid,
                history_before_uid, history_exhausted, synced_at, created_at, updated_at
            ) VALUES (1, 'INBOX', 1, 2, 1, 1, 1, 1, 1, 1)",
            [],
        )?;
        Ok(())
    }).await.unwrap();

    let service = SyncService::new(
        db,
        Arc::new(postium_mail_lib::domain::auth::AuthManager::default()),
    );

    let result = service.get_history_state(1, EmailCategory::Inbox).await.unwrap();

    assert_eq!(result.history_before_uid, Some(1));
    assert!(result.history_exhausted);
}
```

- [ ] **Step 2: 运行测试**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml history_state_should_`

Expected: PASS 或暴露当前状态聚合问题。

- [ ] **Step 3: 修改历史回填服务**

在 `sync_service.rs` 中：

```rust
use crate::domain::sync::{
    history_exhausted_before_uid, next_history_before_uid, HISTORY_UID_BATCH_SIZE,
};
```

把局部 `const HISTORY_UID_BATCH_SIZE` 删除，使用公共常量。

在每个文件夹循环中，在远端查询前加入：

```rust
if history_exhausted_before_uid(before_uid) {
    sync_repo::update_history_state(
        &self.db,
        account_id,
        folder,
        state.as_ref().and_then(|s| s.history_synced_since),
        Some(before_uid),
        true,
    ).await?;
    folder_exhausted_states.push(true);
    continue;
}
```

把：

```rust
let next_before_uid = uids.iter().min().copied().unwrap_or(before_uid);
```

替换为：

```rust
let next_before_uid = next_history_before_uid(&uids, before_uid);
```

保留 `folder_history_exhausted = history_exhausted_before_uid(next_before_uid)`。

- [ ] **Step 4: 初始化首次同步后的 `history_before_uid`**

在 `folder_sync_full.rs` 中，全量同步有 UID 时更新历史状态应传入：

```rust
let history_before_uid = uids.first().copied();
```

范围同步：

```rust
sync_repo::update_history_state(
    &db,
    account_id,
    folder,
    Some(start),
    history_before_uid,
    false,
).await?;
```

全部同步：

```rust
sync_repo::update_history_state(
    &db,
    account_id,
    folder,
    None,
    history_before_uid,
    true,
).await?;
```

空 UID 且 `uidnext` 存在时保留当前 `Some(last_sync_uid)` 或 `Some(uidnext)` 的行为，但确保 `history_exhausted` 和初始范围语义一致。

- [ ] **Step 5: 运行后端相关测试**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml sync_history`

Expected: PASS。

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml last_sync_uid_for_full_sync_should_use_uidnext_high_watermark_when_window_is_empty`

Expected: PASS。

---

### Task 5: 前端文案与刷新入口统一

**Files:**
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Modify: `src/lib/components/email/EmailList.svelte`
- Modify: `src/lib/stores/email.svelte.ts`
- Test: `src/lib/__tests__/components/EmailList.test.ts`
- Test: `src/lib/__tests__/stores/email.test.ts`

**Interfaces:**
- Consumes: `syncStore.syncAccount(accountId) -> Promise<void>`
- Consumes: `emailState.loadEmailsByCategory(accountId, category, page?) -> Promise<void>`
- Produces: UI 文案 `加载更早邮件` / `Load older mail`

- [ ] **Step 1: 更新组件测试文案**

在 `src/lib/__tests__/components/EmailList.test.ts` 中，把 mock i18n 文案改为：

```ts
loadMore: "加载更多",
syncOlder: "加载更早邮件",
syncingOlder: "正在加载更早邮件...",
```

把断言中的“同步更久邮件”替换为“加载更早邮件”。

- [ ] **Step 2: 运行前端测试确认失败**

Run: `rtk bun test src/lib/__tests__/components/EmailList.test.ts`

Expected: FAIL，实际 UI 仍显示旧文案。

- [ ] **Step 3: 修改 i18n 文案**

在 `src/lib/i18n/zh-CN.ts`：

```ts
syncOlder: "加载更早邮件",
syncingOlder: "正在加载更早邮件...",
```

在 `src/lib/i18n/en-US.ts`：

```ts
syncOlder: "Load older mail",
syncingOlder: "Loading older mail...",
```

- [ ] **Step 4: 修改刷新入口**

在 `EmailList.svelte` 的 `handleRefresh` 中，改为：

```ts
async function handleRefresh() {
    if (accountStore.activeAccountId) {
        await syncStore.syncAccount(accountStore.activeAccountId);
        await emailState.loadEmailsByCategory(
            accountStore.activeAccountId,
            emailState.currentFolder,
            emailState.page,
        );
    }
}
```

在 `src/lib/stores/email.svelte.ts` 中删除 `refreshCurrentCategory`，或保留但不再直接调用 `commands.syncAccount`。如果保留，改为只刷新列表：

```ts
async refreshCurrentCategory(accountId: number) {
  await this.loadEmailsByCategory(accountId, this.currentFolder, this.page);
}
```

- [ ] **Step 5: 运行 Svelte/前端测试**

Run: `rtk bun test src/lib/__tests__/components/EmailList.test.ts src/lib/__tests__/stores/email.test.ts src/lib/__tests__/stores/sync-history.test.ts`

Expected: PASS。

Run: `rtk bun run check`

Expected: PASS。

---

### Task 6: 全量验证与绑定检查

**Files:**
- Verify only unless Specta bindings changed.
- Modify if generated: `src/lib/bindings.ts`

**Interfaces:**
- Verifies: Rust backend tests pass.
- Verifies: Frontend tests and type checks pass.

- [ ] **Step 1: 运行 Rust 测试**

Run: `rtk cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS。

- [ ] **Step 2: 运行前端测试**

Run: `rtk bun test`

Expected: PASS。

- [ ] **Step 3: 运行前端检查**

Run: `rtk bun run check`

Expected: PASS。

- [ ] **Step 4: 检查绑定是否需要更新**

如果 Rust command 类型没有新增字段或 command，不需要更新 `src/lib/bindings.ts`。

如果实现中移动了 `HISTORY_UID_BATCH_SIZE` 常量但没有改变 Specta 类型，仍不需要更新绑定。

如果 DTO 字段发生变化，运行项目现有绑定生成命令，并确认 `src/lib/bindings.ts` 只包含预期类型变化。

- [ ] **Step 5: 汇总变更，不提交**

Run: `rtk git status --short`

Expected: 只出现本轮相关文件。不要执行 `git commit`。
