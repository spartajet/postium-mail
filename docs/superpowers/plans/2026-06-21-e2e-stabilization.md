# E2E 稳定化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将现有 WebdriverIO + tauri-driver E2E 从冒烟测试脚手架升级为 Linux CI 可稳定执行的回归测试平台。

**Architecture:** Rust 启动层新增 E2E 模式和独立数据目录解析，E2E 模式下自动 seed 2 个账号和至少 48 封固定邮件。前端只添加稳定 `data-testid` 锚点和账号切换后的列表刷新逻辑，WDIO 负责临时数据目录、driver 生命周期、失败产物和 Linux CI 运行。

**Tech Stack:** Tauri 2、Rust 2024、SeaORM 2.0 RC、Svelte 5、WebdriverIO 9、tauri-driver、Bun、GitHub Actions、SQLite。

## Global Constraints

- 所有新增或修改的文档使用中文。
- 默认 E2E 不连接真实 IMAP、SMTP、OAuth 或远程邮件服务商。
- 默认 E2E 保持真实内部链路：Tauri app、Svelte UI、Tauri commands、Rust services、SQLite、前端 stores 和路由。
- E2E 模式由 `POSTIUM_E2E=1` 启用。
- E2E 模式必须使用 `POSTIUM_DATA_DIR`，缺失时启动失败并给出明确错误。
- Seed 至少创建 2 个账号和 48 封邮件。
- 主账号至少有 24 封 inbox 邮件。
- E2E 主选择器使用 `data-testid`，不依赖 Tailwind class、DOM 位置或翻译文案。
- Linux E2E 是本阶段唯一必过 E2E 门禁。
- Windows E2E 本阶段不是必过项。
- 不把 `browser.pause()` 作为主要同步机制。
- 不通过 `if (element)` 静默跳过必需操作。
- 每个任务完成后提交一次，除非该任务没有文件改动。

---

## 文件结构总览

新增文件：

- `src-tauri/src/infrastructure/testing/mod.rs`：声明测试/运行时辅助模块。
- `src-tauri/src/infrastructure/testing/e2e_seed.rs`：E2E seed 数据插入逻辑，包含账号和邮件 fixtures。
- `e2e/helpers/selectors.js`：`data-testid` 选择器 helper。
- `e2e/helpers/app.js`：应用级等待、账号切换、列表等待等共享 E2E helper。
- `e2e/test/specs/smoke.e2e.js`：应用启动和 seed 可见性冒烟测试。
- `e2e/test/specs/account-switching.e2e.js`：账号切换测试。
- `e2e/test/specs/email-list.e2e.js`：搜索和滚动测试。
- `docs/testing/e2e.md`：中文 E2E 本地运行与排障文档。

修改文件：

- `src-tauri/src/infrastructure/mod.rs`：导出 `testing` 模块。
- `src-tauri/src/lib.rs`：新增 `resolve_data_dir()`，E2E 模式调用 seed。
- `src-tauri/src/domain/providers/pool.rs`：注册默认 `CustomProvider`，保证 seed 账号 `provider = "custom"` 可用于 category 查询。
- `src/lib/components/layout/Sidebar.svelte`：增加 sidebar/account/folder/compose/settings 测试锚点；账号切换后刷新当前文件夹并取消选中邮件。
- `src/lib/components/email/EmailList.svelte`：增加列表、搜索、空状态、刷新按钮、邮件项测试锚点。
- `src/lib/components/email/EmailDetail.svelte`：增加详情、空状态、主题、发件人、正文、星标、删除测试锚点。
- `src/lib/components/email/ComposeModal.svelte`：增加 compose modal 和输入/按钮测试锚点。
- `src/routes/settings/+page.svelte`：增加设置页和主题按钮测试锚点。
- `e2e/pageobjects/*.js`：改为使用 `data-testid` 和显式等待。
- `e2e/wdio.conf.js`：管理临时数据目录、E2E 环境变量、driver ready、截图和报告目录。
- `.github/workflows/test.yml`：Linux E2E 作为必过 job，上传 artifacts；移除 Windows 必过 matrix。

---

### Task 1: Rust E2E 数据目录解析

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `fn resolve_data_dir() -> std::path::PathBuf`
- Consumes: `std::env::var`, `dirs::home_dir`

- [ ] **Step 1: 在 `src-tauri/src/lib.rs` 添加路径导入**

在现有 `use std::sync::Arc;` 附近改为：

```rust
use std::{path::PathBuf, sync::Arc};
```

- [ ] **Step 2: 在 `create_specta_builder()` 前添加 `resolve_data_dir()`**

添加：

```rust
fn resolve_data_dir() -> PathBuf {
    let e2e_enabled = std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1");

    if e2e_enabled {
        let data_dir = std::env::var("POSTIUM_DATA_DIR")
            .expect("POSTIUM_E2E=1 时必须设置 POSTIUM_DATA_DIR");
        return PathBuf::from(data_dir);
    }

    dirs::home_dir()
        .expect("无法获取数据目录")
        .join(".postium")
}
```

- [ ] **Step 3: 替换 `run()` 中硬编码的数据目录**

将：

```rust
let data_dir = dirs::home_dir().expect("无法获取数据目录").join(".postium");
```

替换为：

```rust
let data_dir = resolve_data_dir();
```

- [ ] **Step 4: 运行 Rust 编译检查**

Run: `cd src-tauri && cargo check`

Expected: 编译成功。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/lib.rs
git commit -m "test: add e2e data directory resolution"
```

---

### Task 2: 注册 CustomProvider

**Files:**
- Modify: `src-tauri/src/domain/providers/pool.rs`

**Interfaces:**
- Consumes: `super::enterprise::custom::CustomProvider::new(imap_host, imap_port, smtp_host, smtp_port)`
- Produces: `ProviderPool::default()` 中可通过 `get("custom")` 获取 provider

- [ ] **Step 1: 在 `ProviderPool::default()` 注册 custom provider**

在企业 provider 注册之后、`pool` 返回之前添加：

```rust
pool.register(Arc::new(super::enterprise::custom::CustomProvider::new(
    "imap.postium.test",
    993,
    "smtp.postium.test",
    465,
)));
```

完整位置应在：

```rust
pool.register(Arc::new(
    super::enterprise::microsoft_365::Microsoft365Provider::new(),
));
pool.register(Arc::new(super::enterprise::custom::CustomProvider::new(
    "imap.postium.test",
    993,
    "smtp.postium.test",
    465,
)));
pool
```

- [ ] **Step 2: 运行 Rust 编译检查**

Run: `cd src-tauri && cargo check`

Expected: 编译成功。

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/domain/providers/pool.rs
git commit -m "test: register custom provider for e2e fixtures"
```

---

### Task 3: Rust E2E Seed 模块

**Files:**
- Create: `src-tauri/src/infrastructure/testing/mod.rs`
- Create: `src-tauri/src/infrastructure/testing/e2e_seed.rs`
- Modify: `src-tauri/src/infrastructure/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `DbConn`
- Produces: `pub async fn seed_e2e_data(db: &DbConn) -> Result<(), MailError>`
- Produces: seed 账号 `primary.e2e@postium.test`、`secondary.e2e@postium.test`
- Produces: seed 邮件 subject 包含 `Primary Inbox Message 01`、`Primary Inbox Message 24`、`Secondary Inbox Message 01`、`Quarterly Planning Alpha`

- [ ] **Step 1: 创建 `src-tauri/src/infrastructure/testing/mod.rs`**

```rust
pub mod e2e_seed;
```

- [ ] **Step 2: 创建 `src-tauri/src/infrastructure/testing/e2e_seed.rs`**

```rust
use crate::error::MailError;
use crate::infrastructure::storage::DbConn;
use crate::infrastructure::storage::entities::{accounts, emails};
use crate::infrastructure::storage::repository::account_repo;
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, EntityTrait, Set};

const PRIMARY_EMAIL: &str = "primary.e2e@postium.test";
const SECONDARY_EMAIL: &str = "secondary.e2e@postium.test";
const BASE_TS: i64 = 1_779_936_000;

pub async fn seed_e2e_data(db: &DbConn) -> Result<(), MailError> {
    if account_repo::get_by_email(db, PRIMARY_EMAIL).await?.is_some() {
        return Ok(());
    }

    let now = BASE_TS;
    let primary = accounts::ActiveModel {
        id: NotSet,
        name: Set("Primary E2E".to_string()),
        email: Set(PRIMARY_EMAIL.to_string()),
        display_name: Set(Some("Primary E2E".to_string())),
        provider: Set("custom".to_string()),
        imap_host: Set(Some("imap.postium.test".to_string())),
        imap_port: Set(Some(993)),
        imap_ssl: Set(Some(true)),
        imap_ssl_mode: Set(Some("ssl".to_string())),
        smtp_host: Set(Some("smtp.postium.test".to_string())),
        smtp_port: Set(Some(465)),
        smtp_ssl: Set(Some(true)),
        smtp_ssl_mode: Set(Some("ssl".to_string())),
        color: Set(Some("#2563eb".to_string())),
        sync_enabled: Set(Some(false)),
        last_sync_at: Set(None),
        auth_type: Set(Some("Password".to_string())),
        account_type: Set("personal".to_string()),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(db)
    .await?;

    let secondary = accounts::ActiveModel {
        id: NotSet,
        name: Set("Secondary E2E".to_string()),
        email: Set(SECONDARY_EMAIL.to_string()),
        display_name: Set(Some("Secondary E2E".to_string())),
        provider: Set("custom".to_string()),
        imap_host: Set(Some("imap.postium.test".to_string())),
        imap_port: Set(Some(993)),
        imap_ssl: Set(Some(true)),
        imap_ssl_mode: Set(Some("ssl".to_string())),
        smtp_host: Set(Some("smtp.postium.test".to_string())),
        smtp_port: Set(Some(465)),
        smtp_ssl: Set(Some(true)),
        smtp_ssl_mode: Set(Some("ssl".to_string())),
        color: Set(Some("#16a34a".to_string())),
        sync_enabled: Set(Some(false)),
        last_sync_at: Set(None),
        auth_type: Set(Some("Password".to_string())),
        account_type: Set("personal".to_string()),
        created_at: Set(now + 1),
        updated_at: Set(now + 1),
    }
    .insert(db)
    .await?;

    let mut fixtures = Vec::new();
    fixtures.extend(primary_emails(primary.id));
    fixtures.extend(secondary_emails(secondary.id));

    emails::Entity::insert_many(fixtures).exec(db).await?;
    Ok(())
}

fn primary_emails(account_id: i32) -> Vec<emails::ActiveModel> {
    let mut items = Vec::new();

    let special_subjects = [
        (5, "Quarterly Planning Alpha"),
        (8, "Quarterly Planning Beta"),
        (16, "Quarterly Planning Archive"),
        (24, "Primary Inbox Message 24"),
    ];

    for idx in 1..=24 {
        let subject = special_subjects
            .iter()
            .find_map(|(special_idx, subject)| (*special_idx == idx).then_some(*subject))
            .unwrap_or_else(|| {
                if idx == 1 {
                    "Primary Inbox Message 01"
                } else {
                    "Primary Inbox Message"
                }
            });
        let subject = if subject == "Primary Inbox Message" {
            format!("Primary Inbox Message {idx:02}")
        } else {
            subject.to_string()
        };

        items.push(email_model(
            account_id,
            "INBOX",
            idx,
            &subject,
            idx <= 16,
            matches!(idx, 3 | 6 | 9 | 12),
            BASE_TS - i64::from(idx),
        ));
    }

    for idx in 1..=5 {
        let subject = if idx == 1 {
            "Sent Confirmation Message".to_string()
        } else {
            format!("Primary Sent Message {idx:02}")
        };
        items.push(email_model(
            account_id,
            "Sent",
            100 + idx,
            &subject,
            true,
            false,
            BASE_TS - 100 - i64::from(idx),
        ));
    }

    for idx in 1..=4 {
        let subject = if idx == 1 {
            "Starred Reference Message".to_string()
        } else {
            format!("Primary Starred Message {idx:02}")
        };
        items.push(email_model(
            account_id,
            "Archive",
            200 + idx,
            &subject,
            true,
            true,
            BASE_TS - 200 - i64::from(idx),
        ));
    }

    for idx in 1..=2 {
        let subject = if idx == 1 {
            "Draft Proposal Outline".to_string()
        } else {
            "Primary Draft Message 02".to_string()
        };
        items.push(email_model(
            account_id,
            "Drafts",
            300 + idx,
            &subject,
            true,
            false,
            BASE_TS - 300 - i64::from(idx),
        ));
    }

    items.push(email_model(
        account_id,
        "Trash",
        401,
        "Trash Cleanup Notice",
        true,
        false,
        BASE_TS - 401,
    ));

    items
}

fn secondary_emails(account_id: i32) -> Vec<emails::ActiveModel> {
    let mut items = Vec::new();

    for idx in 1..=8 {
        items.push(email_model(
            account_id,
            "INBOX",
            idx,
            &format!("Secondary Inbox Message {idx:02}"),
            idx <= 4,
            idx == 2,
            BASE_TS - 1_000 - i64::from(idx),
        ));
    }

    for idx in 1..=2 {
        items.push(email_model(
            account_id,
            "Sent",
            100 + idx,
            &format!("Secondary Sent Message {idx:02}"),
            true,
            false,
            BASE_TS - 1_100 - i64::from(idx),
        ));
    }

    items.push(email_model(
        account_id,
        "Archive",
        201,
        "Secondary Starred Message 01",
        true,
        true,
        BASE_TS - 1_201,
    ));

    items.push(email_model(
        account_id,
        "Drafts",
        301,
        "Secondary Draft Message 01",
        true,
        false,
        BASE_TS - 1_301,
    ));

    items
}

fn email_model(
    account_id: i32,
    folder: &str,
    uid: u32,
    subject: &str,
    is_read: bool,
    is_starred: bool,
    sent_at: i64,
) -> emails::ActiveModel {
    let is_secondary = subject.starts_with("Secondary");
    let sender_name = if is_secondary {
        "Secondary Sender"
    } else {
        "Primary Sender"
    };
    let sender_email = if is_secondary {
        "sender.secondary@postium.test"
    } else {
        "sender.primary@postium.test"
    };
    let body = format!("This is deterministic E2E body content for {subject}.");

    emails::ActiveModel {
        id: NotSet,
        account_id: Set(account_id),
        folder: Set(folder.to_string()),
        uid: Set(uid),
        message_id: Set(Some(format!(
            "<e2e-{account_id}-{folder}-{uid}@postium.test>"
        ))),
        subject: Set(Some(subject.to_string())),
        sender_name: Set(Some(sender_name.to_string())),
        sender_email: Set(sender_email.to_string()),
        recipient_emails: Set("e2e.user@postium.test".to_string()),
        cc_emails: Set(None),
        bcc_emails: Set(None),
        preview: Set(Some(format!("Preview for {subject}"))),
        body_text: Set(Some(body.clone())),
        body_html: Set(Some(format!("<p>{body}</p>"))),
        is_read: Set(Some(is_read)),
        is_starred: Set(Some(is_starred)),
        is_draft: Set(Some(folder == "Drafts")),
        is_answered: Set(Some(false)),
        is_deleted: Set(Some(folder == "Trash")),
        sent_at: Set(sent_at),
        received_at: Set(sent_at),
        created_at: Set(sent_at),
        updated_at: Set(sent_at),
    }
}
```

- [ ] **Step 3: 导出 `testing` 模块**

在 `src-tauri/src/infrastructure/mod.rs` 末尾添加：

```rust
/// 测试和 E2E 运行时辅助模块。
pub mod testing;
```

- [ ] **Step 4: 在 `run()` 初始化数据库后调用 seed**

在 `src-tauri/src/lib.rs` 的数据库初始化 async block 中，将：

```rust
infrastructure::storage::database::init_database(&data_dir)
    .await
    .expect("数据库初始化失败")
```

替换为：

```rust
let db = infrastructure::storage::database::init_database(&data_dir)
    .await
    .expect("数据库初始化失败");

if std::env::var("POSTIUM_E2E").ok().as_deref() == Some("1") {
    infrastructure::testing::e2e_seed::seed_e2e_data(&db)
        .await
        .expect("E2E seed 数据初始化失败");
}

db
```

- [ ] **Step 5: 运行 Rust 测试**

Run: `cd src-tauri && cargo test`

Expected: 全部测试通过。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/infrastructure/mod.rs src-tauri/src/infrastructure/testing src-tauri/src/lib.rs
git commit -m "test: seed deterministic e2e data"
```

---

### Task 4: 前端稳定测试锚点和账号切换刷新

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte`
- Modify: `src/lib/components/email/EmailList.svelte`
- Modify: `src/lib/components/email/EmailDetail.svelte`
- Modify: `src/lib/components/email/ComposeModal.svelte`
- Modify: `src/routes/settings/+page.svelte`

**Interfaces:**
- Produces: 设计文档中列出的 `data-testid`
- Produces: `handleAccountSwitch(accountId: number): void`
- Consumes: `emailStore.loadEmailsByCategory(accountId, emailStore.currentFolder)`
- Consumes: `emailStore.deselectEmail()`

- [ ] **Step 1: 修改 `Sidebar.svelte` 的根元素和账号切换逻辑**

将根 `<aside ...>` 改为包含：

```svelte
<aside
    data-testid="sidebar"
    class="flex h-full w-65 shrink-0 flex-col border-r border-border bg-glass backdrop-blur-md"
>
```

在 `<script>` 中添加函数：

```ts
function handleAccountSwitch(accountId: number) {
    accountStore.setActive(accountId);
    emailStore.deselectEmail();
    emailStore.loadEmailsByCategory(accountId, emailStore.currentFolder);
    showAccountDropdown = false;
}
```

将账号触发按钮添加：

```svelte
data-testid="account-switcher"
```

将显示当前账号的 `<span class="flex-1 truncate text-muted-foreground">` 改为：

```svelte
<span
    data-testid="active-account-label"
    class="flex-1 truncate text-muted-foreground"
>
```

在账号列表按钮添加动态 test id：

```svelte
data-testid={account.email === "primary.e2e@postium.test"
    ? "account-option-primary"
    : account.email === "secondary.e2e@postium.test"
      ? "account-option-secondary"
      : undefined}
```

并将 onclick 改为：

```svelte
onclick={() => handleAccountSwitch(account.id)}
```

- [ ] **Step 2: 给 Sidebar 按钮添加锚点**

写邮件按钮添加：

```svelte
data-testid="compose-button"
```

文件夹按钮添加：

```svelte
data-testid={`folder-${folder.id}`}
```

设置按钮添加：

```svelte
data-testid="settings-nav"
```

- [ ] **Step 3: 给 `EmailList.svelte` 添加锚点**

根列表 panel 改为：

```svelte
<div
    data-testid="email-list"
    class="list-panel flex h-full w-95 shrink-0 flex-col border-r border-border bg-elevated/80"
>
```

搜索 input 添加：

```svelte
data-testid="email-search-input"
```

同步/刷新按钮添加：

```svelte
data-testid="email-refresh-button"
```

搜索无结果空状态和普通空状态容器都添加：

```svelte
data-testid="email-empty-state"
```

搜索结果邮件按钮和普通邮件按钮都添加：

```svelte
data-testid="email-item"
data-subject={result.subject || "(No Subject)"}
```

普通列表使用：

```svelte
data-testid="email-item"
data-subject={email.subject || "(No Subject)"}
```

- [ ] **Step 4: 给 `EmailDetail.svelte` 添加锚点**

根容器改为：

```svelte
<div
    data-testid={emailState.selectedEmail ? "email-detail" : "email-detail-empty"}
    class="detail-panel flex h-full flex-1 flex-col bg-background"
>
```

主题 `<h2>` 添加：

```svelte
data-testid="email-subject"
```

发件人显示名称 `<span class="text-sm font-medium text-foreground">` 添加：

```svelte
data-testid="email-sender"
```

正文外层 `<div class="px-5 py-4">` 添加：

```svelte
data-testid="email-body"
```

星标按钮添加：

```svelte
data-testid="email-star-button"
```

删除按钮添加：

```svelte
data-testid="email-delete-button"
```

- [ ] **Step 5: 给 `ComposeModal.svelte` 添加锚点**

模态框主体 `<div class="flex h-[70vh]...">` 添加：

```svelte
data-testid="compose-modal"
```

关闭按钮添加：

```svelte
data-testid="compose-close-button"
```

收件人 input 添加：

```svelte
data-testid="compose-to-input"
```

抄送 input 添加：

```svelte
data-testid="compose-cc-input"
```

主题 input 添加：

```svelte
data-testid="compose-subject-input"
```

编辑器外层 `<div class="flex-1 overflow-hidden">` 添加：

```svelte
data-testid="compose-body-editor"
```

发送按钮添加：

```svelte
data-testid="compose-send-button"
```

- [ ] **Step 6: 给 `settings/+page.svelte` 添加锚点**

根容器改为：

```svelte
<div
    data-testid="settings-page"
    class="flex h-full flex-col overflow-y-auto bg-background"
>
```

浅色按钮添加：

```svelte
data-testid="theme-light"
```

深色按钮添加：

```svelte
data-testid="theme-dark"
```

系统按钮添加：

```svelte
data-testid="theme-system"
```

- [ ] **Step 7: 运行 Svelte 检查**

Run: `bun run check`

Expected: 检查通过。

- [ ] **Step 8: 提交**

```bash
git add src/lib/components/layout/Sidebar.svelte src/lib/components/email/EmailList.svelte src/lib/components/email/EmailDetail.svelte src/lib/components/email/ComposeModal.svelte src/routes/settings/+page.svelte
git commit -m "test: add stable e2e selectors"
```

---

### Task 5: WDIO 生命周期、临时数据目录和失败产物

**Files:**
- Modify: `e2e/package.json`
- Modify: `e2e/wdio.conf.js`

**Interfaces:**
- Produces: `.e2e-data/run-*`
- Produces: `e2e/artifacts/screenshots`
- Produces: `e2e/artifacts/logs`
- Produces: `e2e/artifacts/reports`
- Produces: `process.env.POSTIUM_E2E = "1"` 和 `POSTIUM_DATA_DIR`

- [ ] **Step 1: 增加 WDIO reporter 依赖**

修改 `e2e/package.json` 的 `devDependencies`，增加：

```json
"@wdio/junit-reporter": "^9.19.0"
```

- [ ] **Step 2: 重写 `e2e/wdio.conf.js` 顶部常量和 helper**

保留现有 import，并添加：

```js
import fs from 'fs';
import net from 'net';
```

在常量区添加：

```js
const rootDir = path.resolve(__dirname, '..');
const wdioPort = Number(process.env.WDIO_PORT || 4444);
const e2eDataRoot = path.resolve(rootDir, '.e2e-data');
const runId = `run-${Date.now()}-${process.pid}`;
const runDataDir = path.join(e2eDataRoot, runId);
const artifactsDir = path.resolve(__dirname, 'artifacts');
const screenshotsDir = path.join(artifactsDir, 'screenshots');
const reportsDir = path.join(artifactsDir, 'reports');
const logsDir = path.join(artifactsDir, 'logs');
let hasFailure = false;
```

添加 helper：

```js
function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function waitForPort(port, host = '127.0.0.1', timeoutMs = 15000) {
  const startedAt = Date.now();
  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const socket = net.createConnection({ port, host });
      socket.once('connect', () => {
        socket.destroy();
        resolve();
      });
      socket.once('error', () => {
        socket.destroy();
        if (Date.now() - startedAt > timeoutMs) {
          reject(new Error(`等待 tauri-driver 端口 ${host}:${port} 超时`));
          return;
        }
        setTimeout(tryConnect, 250);
      });
    };
    tryConnect();
  });
}

function cleanupRunData() {
  if (!hasFailure && fs.existsSync(runDataDir)) {
    fs.rmSync(runDataDir, { recursive: true, force: true });
  }
}
```

- [ ] **Step 3: 更新 WDIO config 基础字段**

将：

```js
port: 4444,
```

改为：

```js
port: wdioPort,
```

将 reporters 改为：

```js
reporters: [
  'spec',
  ['junit', {
    outputDir: reportsDir,
    outputFileFormat: (options) => `wdio-${options.cid}.xml`,
  }],
],
```

- [ ] **Step 4: 更新 `onPrepare`**

将 `onPrepare` 改为 async：

```js
onPrepare: async () => {
  ensureDir(runDataDir);
  ensureDir(screenshotsDir);
  ensureDir(reportsDir);
  ensureDir(logsDir);

  const build = spawnSync(
    'bun',
    ['run', 'tauri', 'build', '--debug', '--no-bundle'],
    {
      cwd: rootDir,
      stdio: 'inherit',
      shell: true,
      env: {
        ...process.env,
        POSTIUM_E2E: '1',
        POSTIUM_DATA_DIR: runDataDir,
      },
    }
  );

  if (build.status !== 0) {
    throw new Error(`Tauri debug build failed with status ${build.status}`);
  }

  const appPath = path.resolve(__dirname, '..', 'src-tauri', 'target', 'debug', appBinary);
  if (!fs.existsSync(appPath)) {
    throw new Error(`Tauri app binary not found: ${appPath}`);
  }
},
```

- [ ] **Step 5: 更新 `beforeSession` 启动 driver**

将 `beforeSession` 改为 async，并传入端口：

```js
beforeSession: async () => {
  tauriDriver = spawn(
    path.resolve(cargoBinDir, tauriDriverBinary),
    ['--port', String(wdioPort)],
    {
      stdio: [null, process.stdout, process.stderr],
      env: {
        ...process.env,
        POSTIUM_E2E: '1',
        POSTIUM_DATA_DIR: runDataDir,
      },
    }
  );

  tauriDriver.on('error', (error) => {
    console.error('tauri-driver error:', error);
    process.exit(1);
  });

  tauriDriver.on('exit', (code) => {
    if (!exit) {
      console.error('tauri-driver exited with code:', code);
      process.exit(1);
    }
  });

  await waitForPort(wdioPort);
},
```

- [ ] **Step 6: 增加失败截图和 completion 清理**

在 config 中添加：

```js
afterTest: async (test, context, { error }) => {
  if (!error) return;
  hasFailure = true;
  const safeTitle = test.title.replace(/[^a-z0-9-_]+/gi, '-').toLowerCase();
  await browser.saveScreenshot(path.join(screenshotsDir, `${safeTitle}.png`));
},

onComplete: () => {
  closeTauriDriver();
  cleanupRunData();
},
```

并修改 `onShutdown` 中调用：

```js
onShutdown(() => {
  closeTauriDriver();
  cleanupRunData();
});
```

- [ ] **Step 7: 安装依赖并更新 lockfile**

Run: `cd e2e && bun install`

Expected: `e2e/bun.lock` 更新成功。

- [ ] **Step 8: 运行 WDIO 配置语法检查**

Run: `cd e2e && bunx wdio run wdio.conf.js --help`

Expected: 输出 WDIO 帮助或配置加载成功，不出现 JS 语法错误。

- [ ] **Step 9: 提交**

```bash
git add e2e/package.json e2e/bun.lock e2e/wdio.conf.js
git commit -m "test: harden wdio e2e lifecycle"
```

---

### Task 6: E2E Helper 和 Page Objects 重写

**Files:**
- Create: `e2e/helpers/selectors.js`
- Create: `e2e/helpers/app.js`
- Modify: `e2e/pageobjects/sidebar.page.js`
- Modify: `e2e/pageobjects/email.page.js`
- Modify: `e2e/pageobjects/compose.page.js`
- Modify: `e2e/pageobjects/settings.page.js`

**Interfaces:**
- Produces: `byTestId(id: string): ChainablePromiseElement`
- Produces: `allByTestId(id: string): ChainablePromiseArray`
- Produces: `waitForText(text: string, timeout?: number): Promise<void>`
- Produces: page methods `waitForReady()`, `switchToPrimary()`, `switchToSecondary()`, `openCompose()`, `search(query)`, `scrollToSubject(subject)`

- [ ] **Step 1: 创建 `e2e/helpers/selectors.js`**

```js
export const byTestId = (id) => $(`[data-testid="${id}"]`);
export const allByTestId = (id) => $$(`[data-testid="${id}"]`);

export async function waitForText(text, timeout = 10000) {
  await browser.waitUntil(
    async () => (await $('body').getText()).includes(text),
    {
      timeout,
      timeoutMsg: `页面未在 ${timeout}ms 内出现文本: ${text}`,
    }
  );
}

export async function waitForTextGone(text, timeout = 10000) {
  await browser.waitUntil(
    async () => !(await $('body').getText()).includes(text),
    {
      timeout,
      timeoutMsg: `页面未在 ${timeout}ms 内移除文本: ${text}`,
    }
  );
}
```

- [ ] **Step 2: 创建 `e2e/helpers/app.js`**

```js
import sidebarPage from '../pageobjects/sidebar.page.js';
import emailPage from '../pageobjects/email.page.js';
import { waitForText } from './selectors.js';

export async function waitForAppReady() {
  await sidebarPage.waitForReady();
  await emailPage.waitForReady();
  await waitForText('Primary Inbox Message 01');
}
```

- [ ] **Step 3: 重写 `sidebar.page.js`**

```js
import { byTestId } from '../helpers/selectors.js';

class SidebarPage {
  get root() { return byTestId('sidebar'); }
  get composeButton() { return byTestId('compose-button'); }
  get inboxFolder() { return byTestId('folder-inbox'); }
  get sentFolder() { return byTestId('folder-sent'); }
  get starredFolder() { return byTestId('folder-starred'); }
  get settingsButton() { return byTestId('settings-nav'); }
  get accountSwitcher() { return byTestId('account-switcher'); }
  get primaryAccountOption() { return byTestId('account-option-primary'); }
  get secondaryAccountOption() { return byTestId('account-option-secondary'); }
  get activeAccountLabel() { return byTestId('active-account-label'); }

  async waitForReady() {
    await this.root.waitForDisplayed({ timeout: 10000 });
    await this.accountSwitcher.waitForDisplayed({ timeout: 10000 });
    await this.inboxFolder.waitForDisplayed({ timeout: 10000 });
  }

  async openAccountSwitcher() {
    await this.accountSwitcher.waitForDisplayed({ timeout: 10000 });
    await this.accountSwitcher.click();
  }

  async switchToPrimary() {
    await this.openAccountSwitcher();
    await this.primaryAccountOption.waitForDisplayed({ timeout: 10000 });
    await this.primaryAccountOption.click();
  }

  async switchToSecondary() {
    await this.openAccountSwitcher();
    await this.secondaryAccountOption.waitForDisplayed({ timeout: 10000 });
    await this.secondaryAccountOption.click();
  }

  async clickInbox() {
    await this.inboxFolder.waitForDisplayed({ timeout: 10000 });
    await this.inboxFolder.click();
  }

  async clickSent() {
    await this.sentFolder.waitForDisplayed({ timeout: 10000 });
    await this.sentFolder.click();
  }

  async clickStarred() {
    await this.starredFolder.waitForDisplayed({ timeout: 10000 });
    await this.starredFolder.click();
  }

  async clickSettings() {
    await this.settingsButton.waitForDisplayed({ timeout: 10000 });
    await this.settingsButton.click();
  }
}

export default new SidebarPage();
```

- [ ] **Step 4: 重写 `email.page.js`**

```js
import { allByTestId, byTestId, waitForText } from '../helpers/selectors.js';

class EmailPage {
  get list() { return byTestId('email-list'); }
  get searchInput() { return byTestId('email-search-input'); }
  get emailItems() { return allByTestId('email-item'); }
  get detail() { return byTestId('email-detail'); }
  get detailEmpty() { return byTestId('email-detail-empty'); }
  get detailSubject() { return byTestId('email-subject'); }
  get detailSender() { return byTestId('email-sender'); }
  get detailBody() { return byTestId('email-body'); }

  async waitForReady() {
    await this.list.waitForDisplayed({ timeout: 10000 });
  }

  async search(query) {
    await this.searchInput.waitForDisplayed({ timeout: 10000 });
    await this.searchInput.setValue(query);
    await waitForText(query.split(' ')[0]);
  }

  async clearSearch() {
    await this.searchInput.waitForDisplayed({ timeout: 10000 });
    await this.searchInput.setValue('');
  }

  async clickEmailBySubject(subject) {
    await waitForText(subject);
    const item = await this.findEmailBySubject(subject);
    await item.click();
    await this.detail.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => (await this.detailSubject.getText()).includes(subject),
      {
        timeout: 10000,
        timeoutMsg: `邮件详情未显示主题: ${subject}`,
      }
    );
  }

  async findEmailBySubject(subject) {
    const items = await this.emailItems;
    for (const item of items) {
      const text = await item.getText();
      if (text.includes(subject)) return item;
    }
    throw new Error(`未找到邮件: ${subject}`);
  }

  async scrollToSubject(subject) {
    await this.list.waitForDisplayed({ timeout: 10000 });
    await browser.waitUntil(
      async () => {
        const bodyText = await $('body').getText();
        if (bodyText.includes(subject)) return true;
        await browser.execute(() => {
          const list = document.querySelector('[data-testid="email-list"] .overflow-y-auto');
          list?.scrollBy(0, 500);
        });
        return false;
      },
      {
        timeout: 10000,
        timeoutMsg: `滚动列表后仍未找到邮件: ${subject}`,
      }
    );
  }
}

export default new EmailPage();
```

- [ ] **Step 5: 重写 `compose.page.js`**

```js
import { byTestId } from '../helpers/selectors.js';

class ComposePage {
  get composeButton() { return byTestId('compose-button'); }
  get modal() { return byTestId('compose-modal'); }
  get closeButton() { return byTestId('compose-close-button'); }
  get toInput() { return byTestId('compose-to-input'); }
  get ccInput() { return byTestId('compose-cc-input'); }
  get subjectInput() { return byTestId('compose-subject-input'); }
  get bodyEditorWrap() { return byTestId('compose-body-editor'); }
  get sendButton() { return byTestId('compose-send-button'); }

  async openCompose() {
    await this.composeButton.waitForDisplayed({ timeout: 10000 });
    await this.composeButton.click();
    await this.modal.waitForDisplayed({ timeout: 10000 });
  }

  async closeCompose() {
    await this.closeButton.waitForDisplayed({ timeout: 10000 });
    await this.closeButton.click();
    await this.modal.waitForExist({ reverse: true, timeout: 10000 });
  }

  async fillEmail(to, subject) {
    await this.toInput.waitForDisplayed({ timeout: 10000 });
    await this.toInput.setValue(to);
    await this.subjectInput.setValue(subject);
  }

  async bodyEditor() {
    await this.bodyEditorWrap.waitForDisplayed({ timeout: 10000 });
    return this.bodyEditorWrap.$('.ProseMirror');
  }
}

export default new ComposePage();
```

- [ ] **Step 6: 重写 `settings.page.js`**

```js
import { byTestId } from '../helpers/selectors.js';

class SettingsPage {
  get page() { return byTestId('settings-page'); }
  get lightThemeButton() { return byTestId('theme-light'); }
  get darkThemeButton() { return byTestId('theme-dark'); }
  get systemThemeButton() { return byTestId('theme-system'); }

  async waitForReady() {
    await this.page.waitForDisplayed({ timeout: 10000 });
  }

  async isDarkMode() {
    const html = await $('html');
    const classes = await html.getAttribute('class');
    return classes ? classes.includes('dark') : false;
  }
}

export default new SettingsPage();
```

- [ ] **Step 7: 运行 ESLint 或语法检查**

Run: `cd e2e && bunx eslint .`

Expected: 如果项目当前 ESLint 覆盖 e2e，则通过；如果命令不存在，记录输出并继续下一步。

- [ ] **Step 8: 提交**

```bash
git add e2e/helpers e2e/pageobjects
git commit -m "test: rewrite e2e page objects"
```

---

### Task 7: 重写 E2E Specs

**Files:**
- Delete: `e2e/test/specs/account.e2e.js`
- Delete: `e2e/test/specs/email.e2e.js`
- Delete: `e2e/test/specs/navigation.e2e.js`
- Create: `e2e/test/specs/smoke.e2e.js`
- Create: `e2e/test/specs/navigation.e2e.js`
- Create: `e2e/test/specs/account-switching.e2e.js`
- Create: `e2e/test/specs/email-list.e2e.js`
- Modify: `e2e/test/specs/compose.e2e.js`
- Modify: `e2e/test/specs/theme.e2e.js`

**Interfaces:**
- Consumes: `waitForAppReady()`
- Consumes: page object methods from Task 6
- Produces: independent specs for smoke/navigation/account switching/email list/compose/theme

- [ ] **Step 1: 删除旧的弱断言 specs**

```bash
rm e2e/test/specs/account.e2e.js e2e/test/specs/email.e2e.js e2e/test/specs/navigation.e2e.js
```

- [ ] **Step 2: 创建 `smoke.e2e.js`**

```js
import { waitForAppReady } from '../../helpers/app.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';
import emailPage from '../../pageobjects/email.page.js';

describe('Smoke', () => {
  it('启动应用并显示主账号 inbox seed 数据', async () => {
    await waitForAppReady();
    await expect(sidebarPage.activeAccountLabel).toHaveText(
      expect.stringContaining('primary.e2e@postium.test')
    );
    await expect(emailPage.list).toBeDisplayed();
  });
});
```

- [ ] **Step 3: 创建 `navigation.e2e.js`**

```js
import { waitForText } from '../../helpers/selectors.js';
import { waitForAppReady } from '../../helpers/app.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Navigation', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  it('切换到 Sent 并显示已发送邮件', async () => {
    await sidebarPage.clickSent();
    await waitForText('Sent Confirmation Message');
  });

  it('切换到 Starred 并显示星标邮件', async () => {
    await sidebarPage.clickStarred();
    await waitForText('Starred Reference Message');
  });

  it('切回 Inbox 并显示主账号 inbox 邮件', async () => {
    await sidebarPage.clickInbox();
    await waitForText('Primary Inbox Message 01');
  });
});
```

- [ ] **Step 4: 创建 `account-switching.e2e.js`**

```js
import { waitForAppReady } from '../../helpers/app.js';
import { waitForText, waitForTextGone } from '../../helpers/selectors.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';

describe('Account switching', () => {
  it('在主账号和次账号之间切换并刷新邮件列表', async () => {
    await waitForAppReady();
    await sidebarPage.switchToSecondary();
    await waitForText('Secondary Inbox Message 01');
    await waitForTextGone('Primary Inbox Message 01');

    await sidebarPage.switchToPrimary();
    await waitForText('Primary Inbox Message 01');
  });
});
```

- [ ] **Step 5: 创建 `email-list.e2e.js`**

```js
import { waitForAppReady } from '../../helpers/app.js';
import { waitForText } from '../../helpers/selectors.js';
import emailPage from '../../pageobjects/email.page.js';

describe('Email list', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  it('搜索 Quarterly Planning 并显示固定结果', async () => {
    await emailPage.search('Quarterly Planning');
    await waitForText('Quarterly Planning Alpha');
    await waitForText('Quarterly Planning Beta');
    await waitForText('Quarterly Planning Archive');
  });

  it('滚动到 inbox 底部邮件并打开详情', async () => {
    await emailPage.scrollToSubject('Primary Inbox Message 24');
    await emailPage.clickEmailBySubject('Primary Inbox Message 24');
    await expect(emailPage.detailSubject).toHaveText(
      expect.stringContaining('Primary Inbox Message 24')
    );
  });
});
```

- [ ] **Step 6: 重写 `compose.e2e.js`**

```js
import { waitForAppReady } from '../../helpers/app.js';
import composePage from '../../pageobjects/compose.page.js';

describe('Compose Email', () => {
  beforeEach(async () => {
    await waitForAppReady();
  });

  it('打开写邮件弹窗并填写字段', async () => {
    await composePage.openCompose();
    await composePage.fillEmail('recipient.e2e@postium.test', 'E2E Compose Subject');

    await expect(composePage.toInput).toHaveValue('recipient.e2e@postium.test');
    await expect(composePage.subjectInput).toHaveValue('E2E Compose Subject');

    await composePage.closeCompose();
  });

  it('重新打开写邮件弹窗时字段为空', async () => {
    await composePage.openCompose();

    await expect(composePage.toInput).toHaveValue('');
    await expect(composePage.subjectInput).toHaveValue('');

    await composePage.closeCompose();
  });
});
```

- [ ] **Step 7: 重写 `theme.e2e.js`**

```js
import { waitForAppReady } from '../../helpers/app.js';
import sidebarPage from '../../pageobjects/sidebar.page.js';
import settingsPage from '../../pageobjects/settings.page.js';

describe('Theme Switching', () => {
  beforeEach(async () => {
    await waitForAppReady();
    await sidebarPage.clickSettings();
    await settingsPage.waitForReady();
  });

  it('切换到深色主题', async () => {
    await settingsPage.darkThemeButton.click();
    await browser.waitUntil(() => settingsPage.isDarkMode(), {
      timeout: 5000,
      timeoutMsg: '深色主题未生效',
    });
  });

  it('切换到浅色主题', async () => {
    await settingsPage.lightThemeButton.click();
    await browser.waitUntil(async () => !(await settingsPage.isDarkMode()), {
      timeout: 5000,
      timeoutMsg: '浅色主题未生效',
    });
  });

  it('系统主题按钮可点击', async () => {
    await settingsPage.systemThemeButton.click();
    await expect(settingsPage.systemThemeButton).toBeDisplayed();
  });
});
```

- [ ] **Step 8: 运行 E2E 测试**

Run: `bun run test:e2e`

Expected: E2E 通过；如果失败，查看 `e2e/artifacts/screenshots` 和 `.e2e-data/run-*`，修复后重跑。

- [ ] **Step 9: 提交**

```bash
git add e2e/test/specs
git commit -m "test: add deterministic e2e specs"
```

---

### Task 8: CI Linux E2E 门禁和 Artifacts

**Files:**
- Modify: `.github/workflows/test.yml`

**Interfaces:**
- Consumes: `e2e/artifacts/**`
- Consumes: `.e2e-data/**`
- Produces: Linux-only required E2E job

- [ ] **Step 1: 将 E2E job 改为 Linux-only**

将：

```yaml
  e2e-tests:
    name: E2E Tests (${{ matrix.platform }})
    runs-on: ${{ matrix.platform }}
    needs: [rust-tests, frontend-tests]
    strategy:
      fail-fast: false
      matrix:
        platform: [ubuntu-latest, windows-latest]
```

替换为：

```yaml
  e2e-tests:
    name: E2E Tests (Linux)
    runs-on: ubuntu-latest
    needs: [rust-tests, frontend-tests]
```

- [ ] **Step 2: 删除 Windows 专用步骤和条件**

删除：

```yaml
      # Windows: install matching Edge Driver
      - name: Install msedgedriver (Windows)
        if: matrix.platform == 'windows-latest'
        run: |
          cargo install --git https://github.com/chippers/msedgedriver-tool
          & "$HOME/.cargo/bin/msedgedriver-tool.exe"
          "$($PWD.Path)" >> $env:GITHUB_PATH
```

删除 Linux dependency 步骤中的 `if: matrix.platform == 'ubuntu-latest'`。

删除 Windows run step。

- [ ] **Step 3: 保留 Linux E2E 运行步骤**

保留并确保为：

```yaml
      - name: Run E2E tests
        working-directory: e2e
        run: xvfb-run bun run test
```

- [ ] **Step 4: 添加 artifacts 上传步骤**

在 E2E run step 后添加：

```yaml
      - name: Upload E2E artifacts
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-artifacts
          path: e2e/artifacts/**
          if-no-files-found: ignore

      - name: Upload failed E2E data
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-data
          path: .e2e-data/**
          if-no-files-found: ignore
```

- [ ] **Step 5: 校验 YAML**

Run: `git diff --check .github/workflows/test.yml`

Expected: 无空白错误。

- [ ] **Step 6: 提交**

```bash
git add .github/workflows/test.yml
git commit -m "ci: require linux e2e with artifacts"
```

---

### Task 9: 中文 E2E 文档

**Files:**
- Create: `docs/testing/e2e.md`
- Modify: `README.md`

**Interfaces:**
- Produces: 本地 E2E 运行说明
- Produces: README 测试文档入口

- [ ] **Step 1: 创建 `docs/testing/e2e.md`**

```markdown
# E2E 测试说明

Postium Mail 的 E2E 测试使用 WebdriverIO + tauri-driver 启动真实 Tauri 应用。

默认 E2E 不连接真实 IMAP、SMTP、OAuth 或远程邮件服务商。测试运行时会设置 `POSTIUM_E2E=1`，应用使用独立数据目录并自动插入固定 seed 数据。

## 本地依赖

需要安装：

- Rust stable
- Bun
- Tauri Linux 依赖或当前平台对应的 Tauri 依赖
- `tauri-driver`

安装 tauri-driver：

```bash
cargo install tauri-driver --locked
```

## 安装依赖

```bash
bun install
cd e2e
bun install
```

## 运行 E2E

在仓库根目录运行：

```bash
bun run test:e2e
```

该命令会进入 `e2e` 目录并执行 `wdio run wdio.conf.js`。

## E2E 数据目录

每次运行会创建：

```text
.e2e-data/run-<timestamp>-<pid>
```

成功时该目录会自动删除。失败时会保留，用于排查 SQLite 数据和应用状态。

E2E 模式使用的环境变量：

- `POSTIUM_E2E=1`
- `POSTIUM_DATA_DIR=<本次运行的数据目录>`
- `WDIO_PORT=<可选，默认 4444>`

不要手动让 E2E 使用 `~/.postium`。

## 失败产物

失败时查看：

```text
e2e/artifacts/screenshots/
e2e/artifacts/logs/
e2e/artifacts/reports/
.e2e-data/run-*/
```

CI 会上传 `e2e/artifacts/**`。如果 E2E 失败，也会上传 `.e2e-data/**`。

## CI 策略

本阶段 Linux E2E 是唯一必过 E2E 门禁。Windows E2E 后续单独稳定化。
```

- [ ] **Step 2: 在 README 添加测试文档入口**

在 `README.md` 末尾添加：

```markdown
## 测试

E2E 测试说明见 [docs/testing/e2e.md](docs/testing/e2e.md)。
```

- [ ] **Step 3: 提交**

```bash
git add README.md docs/testing/e2e.md
git commit -m "docs: add e2e testing guide"
```

---

### Task 10: 全量验证

**Files:**
- No planned file changes.

**Interfaces:**
- Consumes: previous tasks
- Produces: validated E2E stabilization branch

- [ ] **Step 1: 运行 Rust 测试**

Run: `bun run test:rust`

Expected: 全部通过。

- [ ] **Step 2: 运行前端测试**

Run: `bun run test:frontend`

Expected: 全部通过。

- [ ] **Step 3: 运行 Svelte 检查**

Run: `bun run check`

Expected: 通过。

- [ ] **Step 4: 运行 E2E 测试**

Run: `bun run test:e2e`

Expected: 通过。

- [ ] **Step 5: 验证工作区状态**

Run: `git status --short`

Expected: 无未提交源码改动。允许存在 ignored 的 `node_modules`、`target`、`.svelte-kit`、`.e2e-data` 失败目录。
