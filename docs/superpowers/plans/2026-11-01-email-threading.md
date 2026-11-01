# 邮件会话线程化（Email Threading）实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Gmail 式会话线程化——混合算法归组（头链优先 + 主题/参与者兜底）、物化 thread_id、会话列表 + 详情时间线、回复写侧带线程头、平铺/会话可切换。

**Architecture:** `postium-core` 新增纯函数归组引擎（`domain/threading/`）+ threads 表与 `thread_repo`（归属/回填/会话聚合查询），同步落库事务内归属；`email_service` 暴露会话 API；`postium-ui` 的 EmailsStore 双行模型（会话/平铺），email_list 会话行 + 切换，email_detail 时间线容器（折叠卡 + 展开卡复用现有单封渲染）。

**Tech Stack:** Rust 2024 / rusqlite（幂等迁移，`database.rs` 模式）/ mail-parser（头提取）/ lettre（写侧头）/ GPUI（UI）。

**Spec:** `docs/superpowers/specs/2026-11-01-email-threading-design.md`（四决策：全局归组 / 混合算法 / 全会话 UI / 物化 thread_id + 可切平铺）。

## Global Constraints

- 分支 `email-threading`（自当前 `html-renderer` HEAD 切出，kickoff 时创建）。
- 迁移幂等：先 `table_has_column`/`table_exists` 检查再执行（`database.rs` 既有模式）。
- i18n 键集守卫：新增 UI 文案必须 zh/en 双语同补（`i18n_tables.rs`）。
- `#[core::prelude::v1::test]` 遮蔽问题仅存在于 gpui-kit glob 场景；postium-core 内用裸 `#[test]`。
- e2e 环境变量：`force_e2e_data_dir()`（既有），窗口尺寸敏感断言用 `open_window`。
- 邮件表列变更必须同步更新 `INSERT_EMAIL_SQL`/`INSERT_EMAIL_HEADER_SQL`/`UPSERT_EMAIL_SQL` 三段 SQL 与 `emails::Model`、`EmailWrite`。
- 提交信息沿用 conventional 前缀（`feat(threading):` / `test(threading):`）。

---

### Task 1: 归组引擎纯函数（domain/threading）

**Files:**
- Create: `crates/postium-core/src/domain/threading/mod.rs`
- Create: `crates/postium-core/src/domain/threading/normalize.rs`
- Modify: `crates/postium-core/src/domain/mod.rs`（加 `pub mod threading;`）

**Interfaces（Produces，后续任务依赖）:**
```rust
pub struct ThreadingInput<'a> {
    pub message_id: Option<&'a str>,      // 本封 Message-ID（不含尖括号语义也可，统一小写）
    pub subject: Option<&'a str>,
    pub chain_ids: Vec<String>,           // In-Reply-To + References 的 Message-ID 并集（小写、含尖括号剥离）
    pub participants: std::collections::BTreeSet<String>, // 本封 发+收+抄 邮箱小写
}
pub enum ThreadResolution { Existing(i64), New }
pub fn normalize_subject(subject: &str) -> String;
pub fn parse_message_ids(raw_header_value: &str) -> Vec<String>;
pub fn resolve_thread<'a>(
    input: &ThreadingInput<'a>,
    lookup: &mut dyn ThreadLookup,
) -> ThreadResolution;
pub trait ThreadLookup {
    /// 头链：任一 id 命中存量邮件的 message_id → 其 thread_id
    fn thread_of_message_id(&mut self, ids: &[String]) -> Option<i64>;
    /// 主题兜底：subject_key 相同的候选线程（updated_at DESC）中参与者有交集者
    fn thread_by_subject_and_participant(
        &mut self,
        subject_key: &str,
        participants: &std::collections::BTreeSet<String>,
    ) -> Option<i64>;
}
```

- [ ] **Step 1: 写失败测试**（`normalize.rs` 内 `#[cfg(test)]` + `mod.rs` 决策测试）

表驱动覆盖：`Re:`/`RE:`/`Fwd:`/`FW:`/`回复：`/`答复：`/`转发：`（全/半角冒号）、嵌套 `Re: 回复： Re: x`、前后空白、连续空白折叠、ASCII 小写化（`Hello` → `hello`）、CJK 保留（`项目评审` 不变）、空串/纯空白 → 空串。`parse_message_ids`：`<a@b> <c@d>`、无尖括号裸 id、多空白/换行分隔、去重小写、空输入 → `[]`。`resolve_thread`：链命中 → Existing 且不再调主题查找（用 MockLookup 计数断言短路）；链未中 + 主题键空串 → New（恒不主题匹配）；主题命中无交集 → New；主题 + 交集 → Existing；多候选由 lookup 决定（mock 返回最新）。

```rust
#[test]
fn normalize_strips_stacked_prefixes() {
    assert_eq!(normalize_subject("Re: 回复： FWD: 项目评审"), "项目评审");
    assert_eq!(normalize_subject("  Hello   World "), "hello world");
    assert_eq!(normalize_subject(""), "");
}
#[test]
fn resolve_prefers_chain_and_short_circuits() {
    let mut lk = MockLookup::default(); // 记录调用
    lk.chain_hit = Some(7);
    let res = resolve_thread(&input_with_chain(vec!["<a@b>"]), &mut lk);
    assert!(matches!(res, ThreadResolution::Existing(7)));
    assert_eq!(lk.subject_calls, 0, "链命中后不得再走主题查找");
}
```

- [ ] **Step 2: 跑测试确认失败**（模块不存在编译错）
- [ ] **Step 3: 实现**——`normalize_subject` 循环剥前缀（前缀表 `["re:", "fwd:", "fw:", "回复：", "回复:", "答复：", "答复:", "转发：", "转发:"]`，输入先小写 ASCII 段再循环 trim_start_matches），空白折叠 `split_whitespace().join(" ")`；`parse_message_ids` 按 `<…>` 提取 + 空白分割兜底，去重小写；`resolve_thread` 按短路序调 lookup。
- [ ] **Step 4: 测试转绿** Run: `cargo test -p postium-core --lib domain::threading`
- [ ] **Step 5: Commit** `feat(threading): 归组引擎纯函数（规范化/链解析/三分支决策）`

### Task 2: threads 表 + emails 加列 + 回填迁移

**Files:**
- Modify: `sql/schema.sql`（threads 表 + 索引；emails 建表语句加两列——新库直接含）
- Modify: `crates/postium-core/src/infrastructure/storage/database.rs`（幂等迁移 `migrate_threading` + 调用于 initialize_schema）
- Create: `crates/postium-core/src/infrastructure/storage/repository/thread_repo.rs`
- Modify: `crates/postium-core/src/infrastructure/storage/repository/mod.rs`（注册）
- Modify: `crates/postium-core/src/infrastructure/storage/models/emails.rs`（Model 加 `thread_id: Option<i64>`、`in_reply_to: Option<String>`）

**Interfaces（Produces）:**
```rust
// thread_repo.rs
pub fn backfill_threading(conn: &rusqlite::Connection) -> rusqlite::Result<BackfillStats>;
pub struct BackfillStats { pub threads_created: usize, pub emails_assigned: usize }
```

schema.sql 追加：

```sql
CREATE TABLE IF NOT EXISTS threads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_key TEXT NOT NULL,
    participants TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_threads_subject_key ON threads(subject_key);
CREATE INDEX IF NOT EXISTS idx_emails_thread_id ON emails(thread_id);
```

`migrate_threading(conn)`：`table_exists("threads")` 不存在则建表；`table_has_column("emails","thread_id")`/`("in_reply_to")` 缺则 `ALTER TABLE emails ADD COLUMN`（允许 NULL）；随后调用 `backfill_threading`（仅在本次新建了 threads 表或 emails 存在无 thread_id 行时执行——幂等判据：`SELECT 1 FROM emails WHERE thread_id IS NULL AND is_draft=0 LIMIT 1`）。回填实现：`SELECT id, message_id, subject, sender_email, recipient_emails, cc_emails, received_at FROM emails WHERE is_draft=0 ORDER BY received_at ASC` 逐行构造 `ThreadingInput`（存量无头 → `chain_ids` 空集走主题路径），`ThreadLookup` 的 rusqlite 实现（`thread_of_message_id`：`SELECT thread_id FROM emails WHERE message_id=? AND thread_id IS NOT NULL LIMIT 1`；`thread_by_subject_and_participant`：候选线程 `LIKE` 匹配逐个内存求交，participants 逗号集合 ≤64 上限追加），`Existing(tid)` 时 UPDATE emails.thread_id + 合并 participants + 刷新 threads.updated_at，`New` 时 INSERT threads + UPDATE。全程单事务。

- [ ] **Step 1: 失败测试**（thread_repo `#[cfg(test)]`，`rusqlite::Connection::open_in_memory` + 手工建最小表）——三封同主题邮件（其中一封 Re: 前缀）回填后同 thread_id；同主题不同参与者 → 两线程；草稿不参与；重复跑回填幂等（stats 归零）。
- [ ] **Step 2: 确认失败** Run: `cargo test -p postium-core --lib thread_repo`
- [ ] **Step 3: 实现 schema/迁移/回填**；注意 `initialize_schema` 调用点放 `SCHEMA_SQL` 之后（建表语句已 IF NOT EXISTS，迁移只负责旧库加列 + 回填）。
- [ ] **Step 4: 转绿** + 既有 database 迁移测试全跑 `cargo test -p postium-core --lib infrastructure::storage`
- [ ] **Step 5: Commit** `feat(threading): threads 表/emails 加列/存量回填迁移`

### Task 3: thread_repo::assign（单封归属落库）

**Files:**
- Modify: `crates/postium-core/src/infrastructure/storage/repository/thread_repo.rs`

**Interfaces（Produces）:**
```rust
/// 单封邮件归属（在调用方事务内执行）：三分支决策 + 建线程/并线程。
/// 返回 thread_id 供邮件行写入。草稿由调用方跳过（不调本函数）。
pub fn assign(conn: &rusqlite::Connection, input: &ThreadingInput<'_>, received_at: i64)
    -> rusqlite::Result<i64>;
```

实现：Task 1 `resolve_thread`（Task 2 的 rusqlite `ThreadLookup` 实现）；`New` → INSERT threads(subject_key=normalize, participants=join(≤64), created_at/updated_at=received_at) 返回 last_insert_rowid；`Existing` → UPDATE participants 追加合并（读出 split/insert/去重/截 64/join）+ `updated_at = MAX(updated_at, received_at)`。

- [ ] **Step 1: 失败测试**——链路径（预插带 message_id 的邮件后 assign 其子邮件）、主题+交集路径、无交集新建、participants 合并去重与 64 上限、updated_at 只增不减。
- [ ] **Step 2: 确认失败** → **Step 3: 实现** → **Step 4: 转绿**（`cargo test -p postium-core --lib thread_repo`）
- [ ] **Step 5: Commit** `feat(threading): thread_repo::assign 单封归属`

### Task 4: 头提取 + 同步落库接线

**Files:**
- Modify: `crates/postium-core/src/infrastructure/protocols/imap/parser.rs`（`ParsedHeaders` 加 `in_reply_to: Option<String>`；`extract_headers_from_message_with_raw` 提取 `msg.in_reply_to_value()`/`msg.references()` 合并——mail-parser API：`msg.in_reply_to()` 返回 `Option<&MessageId>`，取 `.value()` 字串；合并两头的 id 集（`parse_message_ids` 去重）join 逗号，截 2048）
- Modify: `crates/postium-core/src/infrastructure/protocols/types.rs`（`WholeEmailDto` 加 `in_reply_to: Option<String>`）
- Modify: `crates/postium-core/src/infrastructure/protocols/imap/fetch.rs`（组装处透传 ~611 行）
- Modify: `crates/postium-core/src/infrastructure/storage/repository/email_repo.rs`（`EmailWrite` 加 `in_reply_to: Option<String>`；三段 SQL 列表加 `in_reply_to`；`save_batch_emails` 事务内：构造 `ThreadingInput`（message_id/subject/chain=parse_message_ids(in_reply_to)/participants=sender+to+cc 小写集），`is_draft=false` 时 `thread_assign` 后把 `thread_id` 写入 INSERT/UPSERT）
- Modify: `email_repo.rs` 其他 INSERT 构造点（`insert_sent_email`、`insert_draft_email*`、email_service.rs:988、mail_draft.rs:180、e2e_seed.rs 的 EmailWrite 字面量补 `in_reply_to: None`；sent 路径同样走 assign）

**Interfaces（Consumes）:** Task 1 `ThreadingInput`/`parse_message_ids`、Task 3 `assign`。

- [ ] **Step 1: 失败测试**（email_repo 测试模块）——`save_batch_emails` 两封 Re: 同主题邮件（第二封带 `in_reply_to: Some("<id1>")`）→ 同 thread_id 且 threads.updated_at 为第二封时间；无头同主题+交集 → 同线；草稿（is_draft=true）→ thread_id NULL。
- [ ] **Step 2: 确认失败** → **Step 3: 实现全部修改点** → **Step 4: 转绿**（`cargo test -p postium-core --lib` 全量——EmailWrite 字面量补齐后无编译错）
- [ ] **Step 5: Commit** `feat(threading): 头提取 + 同步落库事务内归属`

### Task 5: list_conversations 会话聚合查询

**Files:**
- Modify: `crates/postium-core/src/infrastructure/storage/repository/thread_repo.rs`
- Create: `ConversationSummary` 于 `crates/postium-core/src/service/email_service.rs`（core 出参类型，UI 消费）

**Interfaces（Produces）:**
```rust
pub struct ConversationSummary {
    pub thread_id: i64,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub preview: Option<String>,
    pub received_at: i64,     // 线程最新邮件时间
    pub total: u64,
    pub unread: u64,
    pub starred_any: bool,
}
pub struct ConversationFilter<'a> {
    pub account_id: Option<i32>,      // None = 全账号
    pub category: Option<EmailCategory>, // 虚拟分类（Inbox/Starred/Sent…）
    pub folder: Option<&'a str>,      // 单账号文件夹（与 category 二选一）
    pub unread_only: bool,
}
pub async fn list_conversations(db: &DbConn, filter: &ConversationFilter<'_>, page: usize, limit: usize)
    -> Result<(Vec<ConversationSummary>, u64 /*total*/), MailError>;
pub async fn conversation_emails(db: &DbConn, thread_id: i64, filter: &ConversationFilter<'_>)
    -> Result<Vec<emails::Model>, MailError>; // received_at ASC，同可见性过滤（is_deleted=0）
```

SQL 核心（单条聚合查询，过滤下推为 WHERE 子句拼接——形态对齐 email_repo 现有 list* 的条件构造方式）：

```sql
SELECT t.id, e_last.subject, e_last.sender_name, e_last.sender_email,
       e_last.preview, t.updated_at AS received_at,
       COUNT(e.id) AS total,
       SUM(CASE WHEN e.is_read=0 THEN 1 ELSE 0 END) AS unread,
       MAX(e.is_starred) AS starred_any
FROM threads t
JOIN emails e ON e.thread_id = t.id AND e.is_draft = 0 AND e.is_deleted = 0
  {可见性过滤：account/category/folder/unread_only}
JOIN emails e_last ON e_last.id = (
    SELECT id FROM emails WHERE thread_id = t.id AND is_draft=0 AND is_deleted=0
    {同可见性过滤} ORDER BY received_at DESC LIMIT 1
)
GROUP BY t.id
ORDER BY t.updated_at DESC
LIMIT ? OFFSET ?;
-- 总数：同 FROM/WHERE 的 COUNT(DISTINCT t.id)
```

- [ ] **Step 1: 失败测试**（thread_repo 集成测）——多线程排序（updated_at DESC）、未读/星标/封数聚合、分类过滤（Inbox 只含 INBOX、Starred 只含星标线程）、unread_only 过滤、分页、会话内邮件时间升序（conversation_emails）、已删除邮件不进聚合。
- [ ] **Step 2: 确认失败** → **Step 3: 实现** → **Step 4: 转绿**
- [ ] **Step 5: Commit** `feat(threading): list_conversations/conversation_emails 聚合查询`

### Task 6: email_service 会话 API + EmailDto 透出

**Files:**
- Modify: `crates/postium-core/src/service/email_service.rs`
- Modify: `crates/postium-core/src/infrastructure/storage/models/emails.rs` 已加列的 SELECT 透出（email_repo 各 list 的 query_map 补两列）

**Interfaces（Produces）:**
```rust
impl EmailService {
    pub async fn list_conversations(&self, account_id: i32, folder: &str, page: usize, limit: usize)
        -> Result<ConversationListResponse, MailError>;
    pub async fn list_conversations_by_category(&self, account_id: i32, category: EmailCategory, page: usize, limit: usize)
        -> Result<ConversationListResponse, MailError>;
    pub async fn list_conversations_by_category_for_all_accounts(&self, category: EmailCategory, page: usize, limit: usize)
        -> Result<ConversationListResponse, MailError>;
    pub async fn conversation_emails(&self, thread_id: i64) -> Result<Vec<EmailDto>, MailError>;
    pub async fn mark_conversation_read(&self, thread_id: i64) -> Result<(), MailError>;
    pub async fn toggle_conversation_star(&self, thread_id: i64) -> Result<bool, MailError>; // 作用于最新一封，返回其新状态
}
pub struct ConversationListResponse { pub conversations: Vec<ConversationSummary>, pub total: u64, pub page: usize, pub limit: usize }
```

`EmailDto` 加 `pub message_id: Option<String>` + `pub in_reply_to: Option<String>`（convert_models 透出——写侧 Task 7 与 compose 需要）。`mark_conversation_read`：`UPDATE emails SET is_read=1, updated_at=? WHERE thread_id=? AND is_read=0`；`toggle_conversation_star`：取线程内 received_at 最新一封 id 调既有 toggle_starred 路径。

- [ ] **Step 1: 失败测试**（服务层，临时库）——三 list 变体过滤正确、conversation_emails 时间序、mark 全读后 unread=0、toggle 作用于最新封。
- [ ] **Step 2: 确认失败** → **Step 3: 实现** → **Step 4: 转绿**（`cargo test -p postium-core` 全量）
- [ ] **Step 5: Commit** `feat(threading): email_service 会话 API`

### Task 7: 写侧——回复带线程头

**Files:**
- Modify: `crates/postium-core/src/service/mail_draft.rs`（`SaveDraftRequest` 加 `#[serde(default)] pub in_reply_to: Option<String>` + `#[serde(default)] pub references: Option<String>`；`build_draft_email`：非空时 `builder.header(HeaderType::InReplyTo(...))` —— lettre 0.11 无内置 In-Reply-To/References 头类型时，实现自定义 `RawHeader { name, value }` 实现 `lettre::message::Header` trait（`name()`→`HeaderName::new_from_ascii_str`、`display()`→value、`parse()`→构造）；BuiltDraft 记录已带输出）
- Modify: `crates/postium-ui/src/views/compose.rs`（`ComposeMode::Reply/ReplyAll` 增加 `message_id: Option<String>` + `references: Option<String>` 字段；保存时透传进 SaveDraftRequest；主题已有 Re: 前缀逻辑处确认/补齐——无前缀则 prepend `Re: `）
- Modify: `crates/postium-ui/src/views/email_detail.rs`（reply/reply_all 构造 ComposeMode 处从 `EmailDto.message_id`/`in_reply_to` 填充：`in_reply_to_for_reply = 原.in_reply_to 链 + 原.message_id` 去重截 2048）

- [ ] **Step 1: 失败测试**（mail_draft 单测）——请求带 in_reply_to/references → `build_draft_email` 产出的 `builder` 邮件头含两者（BuiltDraft 增加 headers 断言或在现有发送测试上断言 raw 内容包含 `In-Reply-To:`）；空值不带头。
- [ ] **Step 2: 确认失败** → **Step 3: 实现** → **Step 4: 转绿**；`cargo check --workspace`（ComposeMode 字面量构造点补齐：email_detail 两处 + e2e 若有）
- [ ] **Step 5: Commit** `feat(threading): 回复草稿带 In-Reply-To/References/Re:`

### Task 8: ui_state 视图档 + EmailsStore 双行模型

**Files:**
- Modify: `crates/postium-ui/src/ui_state.rs`（`UiState` 加 `pub email_view_mode: Option<String>`（"threaded"|"flat"）；`load/save` 自动随 serde）
- Modify: `crates/postium-ui/src/stores/emails.rs`

**Interfaces（Produces）:**
```rust
#[derive(Clone, Copy, PartialEq)]
pub enum EmailViewMode { Threaded, Flat }
// EmailsStore 新字段：
pub view_mode: EmailViewMode,
pub conversations: Vec<postium_core::service::email_service::ConversationSummary>,
pub conversation_total: u64,
pub selected_thread: Option<i64>,
pub selected_thread_version: u32,   // 宿主据此装载时间线
// 事件枚举加：
pub enum EmailsEvent { ..., ThreadSelectionChanged(i64) }
// 方法：
impl EmailsStore {
    pub fn set_view_mode(&mut self, mode: EmailViewMode, api: &Arc<CoreApi>, cx: &mut Context<Self>); // 持久化 ui-state + reset_and_load
    pub fn select_thread(&mut self, thread_id: i64, cx: &mut Context<Self>);
}
```

`reset_and_load`/`load_more`/`refresh_now` 内按 `view_mode` 分派到会话 API 或现有平铺 API（导航/搜索/未读过滤全路径保持；搜索在会话态 v1 降级为切平铺执行搜索——会话级 FTS 聚合非本期，注释备案）。

- [ ] **Step 1:** 实现 store 改造 + `cargo check -p postium-ui`（store 无独立单测基建，正确性由 T12 e2e 锁）
- [ ] **Step 2: Commit** `feat(threading): EmailsStore 会话/平铺双行模型`

### Task 9: email_list 会话行 + 切换按钮

**Files:**
- Modify: `crates/postium-ui/src/views/email_list.rs`
- Modify: `crates/postium-ui/src/i18n_tables.rs`（`email.threadedView` zh「会话视图」/ en "Conversation view"；`email.flatView` zh「平铺视图」/ en "Flat view"）

会话行渲染（新 `struct ConversationRow`，对齐现有 EmailRow 的样式约定）：`主题(bold if unread>0) ｜ 参与者摘要（last.sender_name/邮箱；unread>0 加粗主题）｜ preview ｜ received_at ｜ 未读点 ｜ total>1 时封数角标「N」｜ starred_any 星图标`。点击 → `store.select_thread(thread_id)`。列表头部（现有工具条区域）加二态图标切换（MessageSquare/List 图标族）→ `set_view_mode`。行数据源：`view_mode==Threaded` 用 `store.conversations`，否则现状 `store.emails`。

- [ ] **Step 1:** 实现行组件 + 切换 + i18n 双语键 → `cargo check -p postium-ui`
- [ ] **Step 2: Commit** `feat(threading): 会话列表行 + 平铺切换`

### Task 10: email_detail 时间线容器

**Files:**
- Modify: `crates/postium-ui/src/views/email_detail.rs`
- Modify: `crates/postium-ui/src/app.rs`（SelectionChanged 与 ThreadSelectionChanged 双入口：会话选中 → detail_view 重建为时间线形态）

形态：`EmailDetailView` 加 `timeline: Option<Vec<EmailDto>>` + `expanded: Option<i32>`（展开邮件 id）。装载：`store.force_load` 现路径不动；新增会话装载——`api.conversation_emails(thread_id)` 填 timeline，默认展开最后一封（`DetailStore::load(expanded_id)` 复用现有单封详情全部渲染）。UI：现有单封详情上方插入折叠卡列（时间升序、除展开封外每封一行卡：发件人 + 日期 + preview 首行；点击 → `expanded = id` + `store.load(id)`，单展开态）。进入会话即调 `mark_conversation_read(thread_id)`（列表未读点随 EmailsEvent::UnreadChanged 刷新）。

- [ ] **Step 1:** 实现时间线装载 + 折叠卡 + 展开切换 → `cargo check -p postium-ui`
- [ ] **Step 2: Commit** `feat(threading): 详情时间线（折叠卡 + 展开复用单封渲染）`

### Task 11: 会话级操作（星标/删除）

**Files:**
- Modify: `crates/postium-ui/src/views/email_list.rs`（会话行星标点击 → `api.toggle_conversation_star`；会话行删除（现有删除交互形态）→ 对 `conversation_emails` 逐封走既有删除流）
- Modify: `crates/postium-ui/src/stores/emails.rs`（操作后 refresh_now 保持在会话态）

- [ ] **Step 1:** 实现两操作接线 → `cargo check -p postium-ui`
- [ ] **Step 2: Commit** `feat(threading): 会话级星标/删除`

### Task 12: e2e_seed 线程场景 + e2e 全链路

**Files:**
- Modify: `crates/postium-core/src/infrastructure/testing/e2e_seed.rs`（seed 增加一组收件箱线程场景：`邮件A（主题「线程化探针」，message_id=<t1@seed>，sender=a@x → b@me）` + `邮件B（主题「Re: 线程化探针」，in_reply_to=<t1@seed>，sender=b@me? 否——由对方 c@x 发回，参与者含 b@me 保交集）`，非草稿、一封未读）
- Modify: `crates/postium-ui/src/e2e_tests.rs`

e2e（`#[gpui_pre::test] fn email_threading_conversation_view(cx)`，boot + seed 后）：
1. 默认会话态：探针「线程化探针」聚合为**一行**（debug_selector `conv-row-线程化探针`），角标「2」可见，未读点在；
2. 点击会话行 → 时间线两卡（`thread-card-a@x`/`thread-card-c@x` 折叠、最新展开：正文探针文本可见），未读点消失（mark_conversation_read 生效——重查列表探针无未读样式）；
3. 点折叠卡 → 展开切换（原展开卡折叠）；
4. 切平铺 → 同主题两行独立；切回会话 → 记忆保持（ui-state 持久化断言：重开视图仍会话态）。

- [ ] **Step 1:** seed 场景 + e2e 测试写全 → 跑 `cargo test -p postium-ui email_threading`（先失败再实现探针补齐）
- [ ] **Step 2: 全量回归** `cargo test --workspace`（含既有 e2e 不回归——列表默认行为变为会话态会影响现有列表 e2e！需要审视：既有 e2e 断言平铺行 → 首次启动默认 Threaded 会破坏它们。**决策：默认档位 Flat，用户手动切会话并记忆**——比 spec 的"默认会话"更稳（既有 e2e/用户习惯零破坏），spec §5 措辞在实现时同步订正为「默认平铺、可切会话并持久化」。）
- [ ] **Step 3: Commit** `test(threading): e2e 全链路 + seed 线程场景`

### Task 13: 收尾回归 + 文档订正

**Files:**
- Modify: `docs/superpowers/specs/2026-11-01-email-threading-design.md`（§5 默认档位按 T12 决策订正）
- Modify: `.superpowers/sdd/` 记录（沿用账本格式，新建 `2026-11-01-email-threading/progress.md`）

- [ ] **Step 1:** `cargo test --workspace` 双跑全绿 + `cargo build --release -p postium-ui` 通过（体积档案回归）
- [ ] **Step 2: Commit** `chore(threading): spec 订正 + 收尾`

---

## Self-Review 记录

- 覆盖检查：spec §1→T2、§2→T1、§3.1→T4、§3.2→T3/T5、§3.3→T5/T6、§3.4→T11、§4→T7、§5→T8/T9、§6→T10、§7→T8、§8→各任务测试、§9→各任务+T12。无缺口。
- 类型一致：`ThreadingInput`/`ThreadResolution`/`assign`/`ConversationSummary`/`ConversationFilter`/`EmailViewMode`/`ThreadSelectionChanged` 跨任务签名一致（已逐一核对）。
- 关键风险预埋：T12 发现的默认档位问题已裁定为 Flat 默认并回写 spec。
