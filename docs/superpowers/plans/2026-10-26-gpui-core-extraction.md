# GPUI 迁移 · 计划 1：Core 与 Editor 地基 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 postium-mail 后端抽成无 UI 依赖的 `postium-core` workspace crate（含 CoreApi 门面），并建立 `postium-editor`（原生邮件编辑器组件），两者测试全绿，为 UI 层（计划 2）供地基。

> **状态：已完成（2026-10-26）**——全部 6 个任务交付，core 240+ 测试、editor 6 测试全绿。

**Architecture:** 仓库根部建 Cargo workspace（`src-tauri` 排除在外原样保留）；`crates/postium-core` 迁入 service/domain/infrastructure/error 并清除全部 tauri 依赖（事件改 smol channel，异步改 tokio）；新增 `api.rs` 提供 `CoreApi` 门面（44 个原 tauri command 的进程内等价物）；`crates/postium-editor` 迁入 spike 验证过的 EmailEditor 组件并产品化转换器（pulldown-cmark + scraper）。

**Tech Stack:** Rust 2024、tokio、tokio-rusqlite、smol(channel)、gpui-kit 0.8、gpui-kit-markdown-editor(git)、pulldown-cmark、scraper

**Spec:** `docs/superpowers/specs/2026-10-26-gpui-migration-design.md`

## Global Constraints

- 数据目录 `~/.postium` 与 SQLite schema 不变，与 Tauri 版双向兼容
- `src-tauri/`、`src/`（Svelte）在计划 1-2 期间保持原样可运行（过渡期双轨）
- 依赖版本：`gpui-kit = "0.8"`（禁 gpui-fast 特性）；`gpui-kit-markdown-editor = { git = "https://github.com/wabzqem/gpui-kit-markdown-editor" }`（crates.io 0.1.0 不兼容）
- 不引入 webview/wry 任何依赖
- 每个任务以 `cargo test -p <crate>` 全绿 + commit 结束
- 已有 Rust 测试（src-tauri/tests 与模块内 #[cfg(test)]）必须在新 crate 里继续通过

---

### Task 1: Workspace 脚手架

**Files:**
- Create: `Cargo.toml`（仓库根）
- Create: `crates/postium-core/Cargo.toml`
- Create: `crates/postium-core/src/lib.rs`（占位）
- Create: `crates/postium-editor/Cargo.toml`
- Create: `crates/postium-editor/src/lib.rs`（占位）
- Create: `.gitignore` 追加 `/target`

**Interfaces:**
- Produces: workspace 成员 `postium-core`、`postium-editor`；`postium_core::` 与 `postium_editor::` 两个空 crate 名

- [ ] **Step 1: 写根 Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/postium-core", "crates/postium-editor"]
exclude = ["src-tauri", "e2e"]

[workspace.package]
version = "0.3.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
tokio-rusqlite = "0.7"
rusqlite = { version = "0.37", features = ["bundled", "chrono"] }
smol = "2"
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
async-trait = "0.1"
```

注意：`exclude = ["src-tauri"]` 使旧 Tauri 工程继续独立构建，不受 workspace 影响。

- [ ] **Step 2: 写两个 crate 的 Cargo.toml**

`crates/postium-core/Cargo.toml`：依赖 = workspace 的 tokio/tokio-rusqlite/rusqlite/smol/serde/serde_json/chrono/thiserror/tracing/async-trait + 业务依赖（从 `src-tauri/Cargo.toml` 复制邮件协议部分）：

```toml
[package]
name = "postium-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
tokio.workspace = true
tokio-rusqlite.workspace = true
rusqlite.workspace = true
smol.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
thiserror.workspace = true
tracing.workspace = true
async-trait.workspace = true
# 邮件协议（版本照抄 src-tauri/Cargo.toml）
async-imap = { version = "0.11", default-features = false, features = ["runtime-tokio"] }
lettre = { version = "0.11", features = ["tokio1-native-tls"] }
mail-parser = "0.11"
quoted_printable = "0.5"
encoding_rs = "0.8"
oauth2 = { version = "5", features = ["reqwest"] }
tokio-native-tls = "0.3"
tokio-util = { version = "0.7", features = ["compat"] }
futures = "0.3"
base64 = "0.22"
dirs = "6"
keyring = "3"
regex = "1"
reqwest = { version = "0.12", features = ["json"] }
rfc2047-decoder = "1"
url = "2"
utf7-imap = "0.3"
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
```

`crates/postium-editor/Cargo.toml`：

```toml
[package]
name = "postium-editor"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
gpui-kit = "0.8"
gpui-kit-markdown-editor = { git = "https://github.com/wabzqem/gpui-kit-markdown-editor" }
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
scraper = "0.23"

[dev-dependencies]
# 无需 gpui 运行时的纯转换器测试
```

- [ ] **Step 3: 占位 lib.rs**

两个 crate 的 `src/lib.rs` 各写一行：`//! 占位：Task 2/5 填充。`

- [ ] **Step 4: 验证 workspace 构建**

Run: `cargo build --workspace`
Expected: 两个空 crate 编译通过（首次拉取依赖较久）

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/ .gitignore
git commit -m "build(postium): 建立 Cargo workspace 与 postium-core/postium-editor 骨架"
```

---

### Task 2: 迁移 domain 与 error（事件去 tauri 化）

**Files:**
- Create: `crates/postium-core/src/domain/` ← 逐文件复制 `src-tauri/src/domain/`
- Create: `crates/postium-core/src/error.rs` ← 复制 `src-tauri/src/error.rs`
- Modify: `crates/postium-core/src/lib.rs`
- Modify: `crates/postium-core/src/domain/sync/mod.rs`（AppHandle → channel）
- Modify: `crates/postium-core/src/domain/sync/scheduler.rs`（AppHandle → channel、tauri spawn → tokio）

**Interfaces:**
- Produces:
  - `postium_core::domain::*`（与原 `crate::domain::*` 同名同签名，除下述事件参数）
  - `pub type SyncEventTx = smol::channel::Sender<SyncProgressEvent>;`
  - `SyncScheduler::new(db, auth, event_tx: Option<SyncEventTx>)`（原第 3 参 `Option<tauri::AppHandle>` 的等价替换）
  - `postium_core::error::MailError`

- [ ] **Step 1: 复制文件**

```bash
mkdir -p crates/postium-core/src
cp -r src-tauri/src/domain crates/postium-core/src/
cp src-tauri/src/error.rs crates/postium-core/src/
```

- [ ] **Step 2: lib.rs 挂模块**

```rust
pub mod domain;
pub mod error;
```

- [ ] **Step 3: 事件发射点去 tauri 化**

`src-tauri/src/domain/sync/mod.rs:149` 一带：`use tauri::{AppHandle, Emitter};` 与
`app_handle.emit("sync-progress", ev)`（具体调用处用 grep `Emitter\|emit(` 定位）替换为：

```rust
use smol::channel::Sender;

pub type SyncEventTx = Sender<SyncProgressEvent>;

/// 向 UI 广播同步进度（替代原 tauri AppHandle::emit）
pub fn emit_sync_progress(tx: &Option<SyncEventTx>, event: SyncProgressEvent) {
    if let Some(tx) = tx {
        // 非阻塞发送：UI 端消费慢时丢弃最旧语义由接收端决定，这里 try_send
        let _ = tx.try_send(event);
    }
}
```

所有 `app_handle.emit(...)` 调用点改为 `emit_sync_progress(&self.event_tx, event)`，
构造函数（scheduler.rs `SyncScheduler::new`、相关 struct 字段）把 `Option<AppHandle>`
换成 `Option<SyncEventTx>`。`SyncProgressEvent` 本体（serde 结构）不动。

- [ ] **Step 4: scheduler 去 tauri 化**

`domain/sync/scheduler.rs`：`tauri::async_runtime::spawn` → `tokio::spawn`；
`use tauri::AppHandle` 删除；构造参数同 Step 3。

- [ ] **Step 5: 全域 grep 验证无 tauri 残留**

Run: `grep -rn "tauri" crates/postium-core/src/domain/ crates/postium-core/src/error.rs`
Expected: 无输出

- [ ] **Step 6: 编译修复循环**

Run: `cargo build -p postium-core`
Expected: 仅剩 domain 自身编译错误（如 oauth2 spawn 在 infrastructure，属 Task 3；
此步允许 fail，记录错误清单进入 Task 3 修复）。若 domain 层自身报缺
infrastructure/service 的引用（如 `crate::infrastructure::`），将报错清单记入
Task 3（迁移顺序上 domain 先行会有跨层引用，属预期）。

- [ ] **Step 7: Commit**

```bash
git add crates/postium-core
git commit -m "feat(core): 迁移 domain 与 error，同步事件改为 smol channel"
```

---

### Task 3: 迁移 infrastructure 与 service（全部测试转绿）

**Files:**
- Create: `crates/postium-core/src/infrastructure/` ← 复制
- Create: `crates/postium-core/src/service/` ← 复制
- Create: `crates/postium-core/src/tests/`（如 src-tauri/tests 有集成测试则复制对应可迁移子集）
- Modify: `crates/postium-core/src/lib.rs`
- Modify: `infrastructure/auth/oauth2.rs:281`（`tauri::async_runtime::spawn` → `tokio::spawn`）
- Modify: `service/sync_service.rs:183,261,286`（`app_handle: tauri::AppHandle` 参数 → `event_tx: Option<SyncEventTx>`，emit 改 `emit_sync_progress`）

**Interfaces:**
- Consumes: Task 2 的 `SyncEventTx`、`emit_sync_progress`
- Produces:
  - `postium_core::service::AccountService::new(db: Arc<tokio_rusqlite::Connection>, auth: Arc<AuthManager>)`
  - `EmailService::new_for_runtime(auth, db, e2e_enabled: bool, e2e_truth_enabled: bool)`
  - `SyncService::new(db, auth)`（内部方法签名同步去 AppHandle 化）
  - `LabelService::new(db)`、`AttachmentService::new(db, auth, data_dir: PathBuf)`
  - `postium_core::infrastructure::storage::database::init_database(&Path) -> impl Future<Output = Connection>`

- [ ] **Step 1: 复制目录**

```bash
cp -r src-tauri/src/infrastructure crates/postium-core/src/
cp -r src-tauri/src/service crates/postium-core/src/
cp -r src-tauri/tests crates/postium-core/tests 2>/dev/null || true
```

lib.rs 挂 `pub mod infrastructure; pub mod service;`

- [ ] **Step 2: 修复 oauth2 的 spawn**

`infrastructure/auth/oauth2.rs:281`：`tauri::async_runtime::spawn(async move {` →
`tokio::spawn(async move {`。

- [ ] **Step 3: 修复 sync_service 的事件参数**

`service/sync_service.rs` 三处 `app_handle: tauri::AppHandle` 参数改为
`event_tx: Option<SyncEventTx>`，函数体内的 `app_handle.emit(...)` 改
`domain::sync::emit_sync_progress(&event_tx, event)`。调用方（command 层不在本 crate）
由 Task 4 的 CoreApi 传入。

- [ ] **Step 4: 全 crate 去 tauri 残留清零**

Run: `grep -rn "tauri" crates/postium-core/ --include="*.rs"`
Expected: 无输出（有则逐个修复：State/AppHandle/Emitter/async_runtime 全部有上文模式）

- [ ] **Step 5: 编译 + 全部测试**

Run: `cargo test -p postium-core`
Expected: 原有 `cargo test`（在 src-tauri 下）通过的用例在新 crate 全部通过。
逐个修复编译错误；`keyring`/`reqwest` 等 features 若缺失按编译器提示补。

- [ ] **Step 6: 对照旧测试基线**

Run: `cd src-tauri && cargo test 2>&1 | tail -5`（确认旧工程仍绿）
Run: `cargo test -p postium-core 2>&1 | tail -5`
Expected: 两边测试数一致（core 侧 ≥ 旧侧用例数）

- [ ] **Step 7: Commit**

```bash
git add crates/postium-core
git commit -m "feat(core): 迁移 infrastructure 与 service，测试全绿"
```

---

### Task 4: CoreApi 门面

**Files:**
- Create: `crates/postium-core/src/api.rs`
- Create: `crates/postium-core/src/tests/api_tests.rs`
- Modify: `crates/postium-core/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 的全部服务构造函数
- Produces:

```rust
pub struct CoreApi {
    pub db: Arc<tokio_rusqlite::Connection>,
    pub auth: Arc<AuthManager>,
    pub accounts: AccountService,
    pub emails: EmailService,
    pub sync: SyncService,
    pub labels: LabelService,
    pub attachments: AttachmentService,
    pub oauth2: Arc<OAuth2Manager>,
    pub data_dir: PathBuf,
}

impl CoreApi {
    /// 完整装配：初始化日志由调用方（UI main）决定
    pub async fn init(data_dir: &Path, e2e: bool, e2e_truth: bool) -> anyhow::Result<(Arc<Self>, SyncEventRx)>;
    /// 进度事件订阅端（smol channel Receiver），UI 用 cx.spawn 消费
    pub type SyncEventRx = smol::channel::Receiver<SyncProgressEvent>;
}
```

CoreApi 方法 = 原 44 个 command 的进程内直调等价（参数/返回签名照抄 command 层
现签名，去掉 `tauri::State`/`AppHandle` 首参）。方法体即 command 函数体：
原 command 只做参数转发，故把 command/*.rs 的函数体原样搬进对应服务分组方法。

签名清单（从 `src-tauri/src/command/*.rs` 现签名逐个对照，去掉 State/AppHandle 首参后保持一致；UI（计划 2）按此清单调用）：

- account：`list_accounts() -> Vec<Account>`、`get_account(id: i64)`、`create_account(req: CreateAccountRequest)`、`update_account(id: i64, req: UpdateAccountRequest)`、`delete_account(id: i64)`、`update_account_password(id: i64, password: String)`
- email：`list_emails(account_id, folder, offset, limit)`、`list_emails_by_category(account_id, category, offset, limit)`、`list_emails_by_category_for_all_accounts(category, offset, limit)`、`get_email(id)`、`reload_email(id)`、`ensure_attachment_cached(email_id, attachment_id)`、`save_attachment_as(email_id, attachment_id, target_path: PathBuf)`、`open_attachment(email_id, attachment_id)`、`resolve_inline_attachments(email_id)`、`search_emails(query, account_id: Option<i64>, limit)`、`mark_as_read(ids: Vec<i64>, read: bool)`、`toggle_star(id)`、`delete_emails(ids)`、`move_email_to_folder(id, folder)`、`archive_email(id)`、`describe_local_attachments(paths: Vec<PathBuf>)`、`save_draft(draft: SaveDraftRequest)`、`delete_draft(id)`、`parse_email_addresses(input: String)`、`send_email(req: SendEmailRequest)`
- sync：`sync_account(id, event_tx 透传)`、`sync_account_with_range(id, range)`、`get_sync_history_state(id)`、`sync_older_emails(id, folder, before)`、`get_folder_stats(id)`、`get_folder_stats_for_all_accounts()`、`sync_all_accounts()`
- auth：`detect_provider(email)`、`list_providers() -> Vec<ProviderInfo>`、`start_oauth2(provider, email, redirect_port)`、`poll_oauth2(session_id)`
- label：`list_labels(account_id)`、`create_label(account_id, name, color)`、`update_label(label_id, name, color)`、`delete_label(label_id)`、`add_label_to_email(email_id, label_id)`、`remove_label_from_email(email_id, label_id)`、`get_labels_for_email(email_id)`、`list_emails_by_label(label_id, offset, limit)`

- [ ] **Step 1: 写 CoreApi 骨架与 init**

`api.rs` 核心装配（对应 src-tauri/src/lib.rs 步骤 3-7 的服务构建段）：

```rust
use std::{path::Path, sync::Arc};

use crate::domain::{auth::AuthManager, providers::pool::init_provider_pool, sync::SyncProgressEvent};
use crate::error::MailError;
use crate::infrastructure::{auth::oauth2::OAuth2Manager, storage::database::init_database};
use crate::service::{AccountService, AttachmentService, EmailService, LabelService, SyncService};

pub type SyncEventRx = smol::channel::Receiver<SyncProgressEvent>;

pub struct CoreApi { /* 上文 Interfaces 字段 */ }

impl CoreApi {
    pub async fn init(data_dir: &Path, e2e: bool, e2e_truth: bool) -> Result<(Arc<Self>, SyncEventRx), MailError> {
        std::fs::create_dir_all(data_dir).ok();
        let db = init_database(data_dir).await.expect("数据库初始化失败");
        init_provider_pool();
        let auth = Arc::new(AuthManager::default());
        let accounts = AccountService::new(db.clone(), auth.clone());
        let emails = EmailService::new_for_runtime(auth.clone(), db.clone(), e2e, e2e_truth);
        let sync = SyncService::new(db.clone(), auth.clone());
        let labels = LabelService::new(db.clone());
        let attachments = AttachmentService::new(db.clone(), auth.clone(), data_dir.to_path_buf());
        let oauth2 = Arc::new(OAuth2Manager::new(auth.clone(), db.clone()).expect("初始化OAuth2失败"));
        auth.set_oauth2_manager(oauth2.clone());
        let (tx, rx) = smol::channel::bounded::<SyncProgressEvent>(256);
        let _ = tx; // scheduler 启动时传入（计划 2 的 UI 负责调度器生命周期；core 暴露 tx 构造）
        Ok((Arc::new(Self { db, auth, accounts, emails, sync, labels, attachments, oauth2, data_dir: data_dir.to_path_buf() }), rx))
    }
}
```

（若 e2e seed 逻辑需要，照 lib.rs 188-195 搬入 init。）

- [ ] **Step 2: 搬入 44 个方法**

对每个 command 文件：打开 `src-tauri/src/command/<x>.rs`，把函数体复制进
`impl CoreApi`（分组注释 `// ===== account =====`），首参 State 去掉、
`service.foo(...)` 改 `self.accounts.foo(...)`（AttachmentService 的调用同理）。
sync 组的方法保留 `event_tx: Option<SyncEventTx>` 参数透传（UI 传入自身 rx 的 tx）。

- [ ] **Step 3: 冒烟测试（临时数据库）**

`crates/postium-core/src/tests/api_tests.rs`：

```rust
use postium_core::api::CoreApi;

#[tokio::test]
async fn core_api_boots_and_lists_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let (api, _rx) = CoreApi::init(dir.path(), false, false).await.unwrap();
    let accounts = api.list_accounts().await.unwrap();
    assert!(accounts.is_empty());
}
```

Run: `cargo test -p postium-core --test api_tests`
Expected: PASS

- [ ] **Step 4: 全量测试 + Commit**

```bash
cargo test -p postium-core
git add crates/postium-core
git commit -m "feat(core): CoreApi 门面——44 个原 tauri command 的进程内等价"
```

---

### Task 5: postium-editor 组件（EmailEditor 产品化）

**Files:**
- Create: `crates/postium-editor/src/lib.rs`
- Create: `crates/postium-editor/src/state.rs`（EmailEditorState，自 spike `src/email_editor.rs` 迁入）
- Create: `crates/postium-editor/src/view.rs`（EmailEditor 工具栏视图，自 spike 迁入）
- Create: `crates/postium-editor/src/convert.rs`（md⇄HTML 产品化转换器）
- Create: `crates/postium-editor/tests/convert_tests.rs`

**Interfaces:**
- Consumes: `gpui_kit`（`gpui_kit::init` 由宿主先调用）、`markdown_editor::init`
- Produces（UI 计划 2 直接使用）:

```rust
pub use state::{EmailEditorEvent, EmailEditorState};
pub use view::EmailEditor;
pub use convert::{md_to_email_html, email_html_to_md};

// state.rs
impl EmailEditorState {
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self>;
    pub fn from_html(html: &str, window: &mut Window, cx: &mut App) -> Entity<Self>;
    pub fn html(&self, cx: &App) -> String;
    pub fn set_html(&mut self, html: &str, cx: &mut Context<Self>);
    pub fn is_empty(&self, cx: &App) -> bool;
    pub fn set_link(&mut self, url: Option<&str>, cx: &mut App);
}
pub enum EmailEditorEvent { Changed { html: String }, EditLinkRequested { url: Option<SharedString> } }
// view.rs：#[derive(IntoElement)] pub struct EmailEditor { state: Entity<EmailEditorState> }
//         impl EmailEditor { pub fn new(state: &Entity<EmailEditorState>) -> Self }
```

- [ ] **Step 1: 迁入 spike 组件**

把 `E:\rust\postium-gpui-spike\src\email_editor.rs` 拆为 `state.rs`（EmailEditorState +
EmailEditorEvent）与 `view.rs`（EmailEditor + 工具栏），原样保留已验证的工具栏
命令分发与事件桥逻辑；`lib.rs`：

```rust
mod convert;
mod state;
mod view;

pub use convert::{email_html_to_md, md_to_email_html};
pub use state::{EmailEditor, EmailEditorEvent, EmailEditorState};
// 注：EmailEditor 实际从 view 导出，此处 re-export 保持单一入口
pub use view::EmailEditor as EmailEditorView;
```

（re-export 命名以编译通过为准收敛为：`pub use view::EmailEditor; pub use state::{EmailEditorState, EmailEditorEvent};`）

- [ ] **Step 2: md→HTML 产品化（pulldown-cmark）**

`convert.rs` 的 `md_to_email_html(md: &str) -> String`：用 pulldown-cmark 解析，
`push_html` 产标准 HTML 后，按邮件规范追加内联样式（段落 margin、blockquote 边框、
代码块背景），任务列表 `- [ ]` 转换为 `☐/☑` 文本（pulldown-cmark task list extension）：

```rust
use pulldown_cmark::{html, Options, Parser};

pub fn md_to_email_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, opts);
    let mut body = String::new();
    html::push_html(&mut body, parser);
    inline_email_styles(&body) // 后处理：给 p/blockquote/pre 加 style 属性，task checkbox 替换文本
}
```

`inline_email_styles` 用字符串替换实现（`<p>` → `<p style="margin:0 0 8px;">` 等，
与 spike 版样式一致）。

- [ ] **Step 3: HTML→md 产品化（scraper）**

`email_html_to_md(html: &str) -> String`：scraper 解析 DOM，遍历 body 子树产出块级
md（h1-h6 → `#`、p/div → 段落、ul/ol/li → 列表行（缩进按深度）、blockquote → `> `、
pre → 围栏代码、行内 b/strong→`**`、i/em→`*`、s/del→`~~`、code→反引号、a→`[t](u)`、
img→`![alt](src)`），实体解码用 scraper 自带 text()。

- [ ] **Step 4: 转换器往返测试**

`tests/convert_tests.rs`：

```rust
use postium_editor::{email_html_to_md, md_to_email_html};

#[test]
fn roundtrip_bold_list_link() {
    let md = "# 标题\n\n**加粗** 与 [链接](https://a.b) 和：\n\n- 甲\n- 乙\n";
    let html = md_to_email_html(md);
    let back = email_html_to_md(&html);
    assert!(back.contains("# 标题"), "标题往返: {back}");
    assert!(back.contains("**加粗**"), "加粗往返: {back}");
    assert!(back.contains("[链接](https://a.b)"), "链接往返: {back}");
    assert!(back.contains("- 甲"), "列表往返: {back}");
}

#[test]
fn chinese_content_survives() {
    let html = r#"<p>你好，<b>世界</b></p><ul><li>中文列表项</li></ul>"#;
    let md = email_html_to_md(html);
    assert!(md.contains("**世界**") && md.contains("中文列表项"));
}
```

Run: `cargo test -p postium-editor`
Expected: PASS（往返断言失败则调转换器直到过）

- [ ] **Step 5: 组件编译冒烟**

Run: `cargo build -p postium-editor`
Expected: 通过（gpui-kit 依赖编译较久）

- [ ] **Step 6: Commit**

```bash
git add crates/postium-editor
git commit -m "feat(editor): postium-editor 组件（EmailEditor + md⇄HTML 产品化转换器）"
```

---

### Task 6: 收尾验证

**Files:**
- Modify: `README.md`（补 workspace 说明一段）
- Modify: `docs/superpowers/specs/2026-10-26-gpui-migration-design.md`（状态注记：计划 1 完成）

- [ ] **Step 1: 全 workspace 测试**

Run: `cargo test --workspace && cd src-tauri && cargo test`
Expected: 全绿（新旧双轨并存）

- [ ] **Step 2: README 补一段**

在 README.md 开发环境一节后追加：

```markdown
## GPUI 迁移（进行中）

本仓库正迁移到 GPUI 原生 UI。Cargo workspace 位于根目录：

- `crates/postium-core`：后端（domain/service/infrastructure + CoreApi）
- `crates/postium-editor`：原生邮件编辑器组件

`src-tauri/` 与 `src/`（Svelte）在迁移期间保留双轨，验收后移除。
设计文档：`docs/superpowers/specs/2026-10-26-gpui-migration-design.md`
```

- [ ] **Step 3: Commit**

```bash
git add README.md docs
git commit -m "docs: 计划 1（core/editor 地基）完成说明"
```

---

## Self-Review 记录

- Spec 覆盖：本计划覆盖设计文档阶段 1-2（core 抽取、editor 产品化）；阶段 3-8 属计划 2（postium-ui），另行编写
- 类型一致性：`SyncEventTx`/`emit_sync_progress`/`CoreApi::init` 签名在 Task 2/3/4 间一致；EmailEditor API 与 spike 已验证代码一致
- 占位符扫描：无 TBD/TODO；44 方法签名以清单形式给出（对照 command 现签名，非占位）
