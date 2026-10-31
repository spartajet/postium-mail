# IMAP 引擎 · 阶段 3（IDLE 推送 + 文件夹 registry + UI 联通）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 收信从 5 分钟轮询升级为 IDLE 秒级推送（收件箱恒盯 + 浏览过的文件夹 LRU），文件夹角色从"名字猜"升级为 SPECIAL-USE 三层发现 + DB 落地，新邮件事件贯通到 UI 列表。

**Architecture:** `IdleManager`（engine/idle.rs）每账号管理 ≤3 条专属 IDLE 连接（watch set = INBOX + LRU，每条独立连接独立任务）；EXISTS → 发 `SyncNow` 命令给 AccountEngine（复用三档同步）；IDLE 不支持/连续失败 → 90s 轮询降级。文件夹角色：`list_folders` 的 SPECIAL-USE 标注 → `folders` 表新列 → 侧栏消费；回退链走既有 `domain/folders/registry.rs` 启发式。

**Tech Stack:** async-imap 0.11 `Session::idle() -> Handle`（`wait_with_timeout` / StopSource）、现有 engine 全部产物。

**设计文档:** `docs/superpowers/specs/2026-10-29-imap-engine-design.md` §5/§6。

**关键 API 事实（已核实）**：
- `ImapClient::idle(self) -> Handle<T>` **消费** Session——IDLE 连接是专属连接，不进池、不跑命令循环。
- `Handle::init()` 启动 IDLE；`wait_with_timeout(dur)` 返回 `(Future<Result<IdleResponse>>, StopSource)`；`IdleResponse::{NewData(ResponseData), Timeout}`；超时被任意响应重置（含服务器 keepalive）。默认 `wait()` = 29 分钟。
- 结束 IDLE：drop StopSource 触发 DONE（读 idle.rs 确认 drop 语义）。
- `domain/folders/registry.rs` 已有 `FolderRegistry::classify`/`remote_folders`/`keyword_candidate`（SPECIAL-USE/关键字启发式雏形）。

---

## File Structure

```
修改 crates/postium-core/src/engine/transport.rs    # +into_idle 桥（ImapClient → idle Handle 包装）
新建 crates/postium-core/src/engine/idle.rs         # IdleManager：watch 决策纯函数 + 每账号 idle 任务循环
修改 crates/postium-core/src/engine/actor.rs        # WatchFolder 命令 + 引擎启动时拉起 idle 任务 + NewMail→SyncNow
修改 crates/postium-core/src/engine/mod.rs
修改 crates/postium-core/src/infrastructure/storage/database.rs        # folders 表加 special_use 列（幂等）
修改 crates/postium-core/src/infrastructure/storage/models/folders.rs  # Model 字段
修改 crates/postium-core/src/infrastructure/storage/repository/folders_repo.rs（若无此文件则在 sync_repo 或新文件）
修改 crates/postium-core/src/infrastructure/protocols/imap/mod.rs      # list_folders 的 FolderInfo 已有 special_use（核实）
修改 crates/postium-core/src/domain/folders/registry.rs                # 接 SPECIAL-USE 标注（若缺）
测试：engine/idle.rs 纯函数单测 + tests/engine_idle.rs（可选集成）
```

---

### Task 1: 文件夹角色落地——folders 表 special_use 列 + LIST 标注接线

**Files:** database.rs（幂等 migration，照 `migrate_sync_state_highest_modseq_column` 模式）、models/folders.rs（若 Model 存在；folders 的 DB 模型位置先 `rg "struct.*Folder" crates/postium-core/src/infrastructure/storage` 确认——可能 folders 不落 DB 而是实时 LIST。**以实际为准**：若 folders 不落 DB，则角色存 `folder_categories_by_account` 既有机制（sync_repo::upsert_folder_category 已存在）并在侧栏读取处消费，不加列。）

**Steps:**
- [ ] 盘点：`rg -n "upsert_folder_category|folder_categor" crates/postium-core/src` + 侧栏读取路径（`crates/postium-ui/src/views/sidebar.rs` 的 groups/labels 来源）
- [ ] 实现：LIST 结果的 SPECIAL-USE 标注（`FolderInfo.special_use` 字段已存在——核实）经 registry.classify → 写入角色存储；侧栏按角色排序/图标（收件箱置顶、\Sent/\Drafts/\Trash/\Junk 各自图标）
- [ ] 测试：registry.classify 对 SPECIAL-USE 标注的映射单测（已有雏形补齐）
- [ ] `cargo test -p postium-core folders` + build；提交 `feat(engine): 文件夹角色 SPECIAL-USE 落地`

### Task 2: IDLE 传输桥

**Files:** transport.rs + pool.rs（MockSession 不需要 idle——IDLE 不走 trait/池）

- [ ] `ImapClient` 加 `pub fn into_idle(self) -> async_imap::extensions::idle::Handle<...>`（转发 `session.idle()`——注意 ImapClient::idle(self) 消费 self，直接转发）
- [ ] `engine/transport.rs` 加 `pub struct IdleBridge { handle: Handle<...> }`（包装 init/wait_with_timeout/stop）——或直接暴露 async-imap Handle 类型（engine 内部用，不抽象）
- [ ] 编译 + 提交 `feat(engine): IDLE 传输桥`

### Task 3: IdleManager

**Files:** 新建 `crates/postium-core/src/engine/idle.rs`；修改 actor.rs（引擎启动时拉起 idle 任务）

- [ ] **watch 决策纯函数**（TDD）：

```rust
/// 当前 watch 集合 → 需要开始/停止的文件夹（LRU 上限 max）。
pub fn watch_changes(
    inbox: &str,
    requested: &[String],       // 用户浏览过的文件夹（按最近使用序）
    watching: &[String],        // 当前在盯的
    max: usize,                 // 上限（含 INBOX）
) -> (Vec<String>, Vec<String>)  // (start, stop)
```

规则：INBOX 恒 watch；requested 倒序（最近优先）取 max-1 个非 INBOX 文件夹；已在 watch 但不在新集合 → stop。测试矩阵 ≥6 用例（INBOX 恒在/LRU 替换/超出上限/去重）。

- [ ] **idle 任务循环**（每 watched 文件夹一个 tokio task）：

```
loop {
  连接（factory.connect）→ into_idle → init → loop {
     wait_with_timeout(25min) => NewData(EXISTS) → stop → 触发 SyncNow(folder) → break 重进 IDLE
                              => Timeout → break 重进 IDLE（重发）
  }
  连续失败 2 次 → 降级标记（该文件夹 90s 轮询：sleep 90 → SyncNow）→ 成功后恢复 IDLE
}
```

- [ ] **降级决策纯函数**：`should_degrade(consecutive_failures: u32) -> bool`（≥2）+ 恢复逻辑单测
- [ ] **AccountEngine 集成**：`WatchFolder { folder }` 命令（UI 打开文件夹时发）更新 requested；引擎 run 启动时 spawn idle 任务（INBOX）+ 90s 轮询兜底任务（降级文件夹）；EXISTS → 自发 SyncNow 命令
- [ ] `cargo test -p postium-core idle`（决策/降级单测）+ build；提交 `feat(engine): IdleManager——watch LRU + 25min 重发 + 降级轮询`

### Task 4: UI 联通收尾

**Files:** actor.rs（NewMail/同步完成 → 既有 Progress 事件链已通——核实 IDLE 触发的 SyncNow 走 CoreApi.sync_tx 出口可达状态栏/节流刷新）；registry/folders 角色变化 → `FolderListChanged` 事件 → 侧栏刷新（accounts store reload 复用）

- [ ] 核实：IDLE EXISTS → SyncNow → execute_plan 发 Progress → SyncEvent::Progress → AppShell 节流刷新（阶段 2 已接）——**端到端已通**，缺的只是 idle 任务触发 SyncNow 的联通（Task 3 已做）
- [ ] 侧栏角色消费：sidebar.rs 按 special_use 排序/图标（核实 groups 数据来源后最小改动）
- [ ] `cargo test --workspace` + 手动验证清单（真机：163 IDLE 支持存疑→观察降级轮询日志；Outlook IDLE 应推送）
- [ ] 提交 `feat(engine): IDLE 推送接线——收信秒级体验收尾`
