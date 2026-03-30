# Postium Mail 全面架构重构 —实现计划

> 基于设计文档：`docs/superpowers/specs/2026-03-30-architecture-refactor-design.md`

## 阶段 1：后端基础层重构

### Task 1.1：为 MailError 添加 specta 序列化支持

**目标：** 让现有 `MailError` 枚举能通过 Tauri Specta 自动导出为 TypeScript 类型。

**文件：**
- 修改：`src-tauri/src/error/types.rs`
- 修改：`src-tauri/Cargo.toml`

**具体操作：**

1. 在 `Cargo.toml` 的 `[dependencies]` 中添加：
   ```toml
   specta = { version = "2" }
   specta-typescript = { version = "0.0.21" }
   tauri-specta = { version = "2.0.0-rc" }
   ```

2. 在 `error/types.rs` 中，为 `MailError` 枚举及其子错误枚举（`ConnectionError`, `AuthError` 等）添加 specta 派生宏：
   ```rust
   #[derive(Debug, thiserror::Error, specta::Type, serde::Serialize)]
   #[serde(tag = "type", content = "message")]
   pub enum MailError {
       #[error("账号未找到: {0}")]
       AccountNotFound(i32),
       // ... 其余变体保持不变，但添加 specta::Type 和 Serialize
   }
   ```

3. 为所有子错误枚举（`ConnectionError`, `AuthError`, `SyncError` 等）添加相同的 derive 宴象

4. 添加 `From<MailError> for String` 实现，使用 `serde_json::to_string` 序列化为结构化 JSON（错误码 + 消息）

**验证：** `cargo check` 编译通过，`cargo test --lib`

---

### Task 1.2：引入 tauri-specta Builder

**目标：** 在 `lib.rs` 中配置 Tauri Specta， 曽 generate_handler 曘 `为 `collect_commands!` + Builder 模式。

**文件：**
- 修改：`src-tauri/src/lib.rs`

**具体操作：**

1. 在 `lib.rs` 顶部添加 imports:
   ```rust
   use tauri_specta::{collect_commands, Builder};
   use specta_typescript::Typescript;
   ```

2. 替换当前的 `tauri::Builder::default()` 面中的 `.invoke_handler(tauri::generate_handler![...])` 为:
   ```rust
   let builder = Builder::<tauri::Wry>::new()
       .commands(collect_commands![
           command::list_accounts,
           command::get_account,
           command::delete_account,
           command::detect_provider,
           command::list_providers,
           command::start_auth_command,
           command::list_emails,
           command::get_email,
           command::search_emails_fts,
           command::mark_as_read,
           command::toggle_star,
           command::delete_emails,
           command::move_email_to_folder,
           command::get_folder_stats,
           command::sync_account,
           command::sync_account_with_progress,
           command::send_email,
       ]);

   #[cfg(debug_assertions)]
   builder
       .export(Typescript::default(), "../src/lib/bindings.ts")
       .expect("Failed to export typescript bindings");

   tauri::Builder::default()
       // ... plugins 保持不变
       .invoke_handler(builder.invoke_handler())
       .setup(move |app| {
           builder.mount_events(app);
           // ... setup 逻辑保持不变
           Ok(())
       })
   ```

3. 在 `lib.rs` 中将 `pub mod command;` 改为 `mod command;`（command 模块不需要公开）

**验证：** `cargo build` 编译通过。在 debug 模式下运行时确认 `src/lib/bindings.ts` 已生成

---

### Task 1.3：为所有 Command 添加 #[specta::specta] 注解

**目标：** 为每个 `#[tauri::command]` 函数添加 `#[specta::specta]` 注解。

**文件：**
- 修改: `src-tauri/src/command/account.rs`
- 修改: `src-tauri/src/command/email.rs`
- 修改: `src-tauri/src/command/sync.rs`
- 修改: `src-tauri/src/command/auth.rs`
- 修改: `src-tauri/src/command/provider.rs`

**具体操作：**

为以下每个函数添加 `#[specta::specta]`（紧跟在 `#[tauri::command]` 后面））：
- `account.rs`: `list_accounts`, `get_account`, `delete_account`
- `email.rs`: `list_emails`, `get_email`, `search_emails_fts`, `mark_as_read`, `toggle_star`, `delete_emails`, `move_email_to_folder`, `get_folder_stats`
- `sync.rs`: `sync_account_with_progress`, `sync_account`, `send_email`
- `auth.rs`: `start_auth_command`
- `provider.rs`: `detect_provider`, `list_providers`

**验证：** `cargo build` 编译通过。检查 `bindings.ts` 中是否包含所有命令的类型定义

---

### Task 1.4：将 Command 返回类型改为 `Result<T, MailError>`

**目标：** 将所有 Command 的错误返回从 `Result<T, String>` 改为 `Result<T, MailError>`，利用 Task 1.1 添加的 `From<MailError> for String` 自动转换。

**文件：**
- 修改所有 5 个 command 文件

**具体操作：**

将每个 command 函数的返回类型中的 `String` 改为 `MailError`，例如：
```rust
// 之前
pub async fn list_accounts(state: tauri::State<'_, DatabaseState>) -> Result<Vec<storage::AccountDto>, String> {

// 之后
pub async fn list_accounts(state: tauri::State<'_, DatabaseState>) -> Result<Vec<storage::AccountDto>, MailError> {
```

同时将函数体内所有 `.map_err(|e| e.to_string())` 删除（因为 `From<MailError>for String` 会自动转换）。

**验证：** `cargo build` 编译通过。`cargo test --lib` 全部通过。

---

### Task 1.5：确保所有 specta 导出的类型正确

**目标：** 验证 `bindings.ts` 的内容完整且正确。

**具体操作：**

1. 运行 `cargo build` 生成 bindings
2. 检查 `src/lib/bindings.ts`（注意：这是前端 src 目录，重构后会变）
3. 确认包含：
   - 所有 command 函数（如 `listAccounts`, `createAccount` 等）
   - 所有 DTO 类型（`AccountDto`, `EmailListResponse`, `MailError` 等）
   - 所有事件类型（如果有）

**验证：** bindings.ts 中没有 `any` 类型，所有类型都是具体的。

---

## 阶段 2：引入 Service 层

### Task 2.1：创建 service 模块骨架

**目标：** 创建 `src-tauri/src/service/` 目录和基础结构。

**新建文件：**
- `src-tauri/src/service/mod.rs`

**具体操作：**

```rust
// service/mod.rs
mod account_service;
mod email_service;
mod sync_service;

pub use account_service::AccountService;
pub use email_service::EmailService;
pub use sync_service::SyncService;
```

在 `lib.rs` 中添加 `pub mod service;`

**验证：** `cargo check` 编译通过

---

### Task 2.2：实现 AccountService

**目标：** 将账号相关业务逻辑从 Command/Repository 提取到 AccountService。

**新建文件：** `src-tauri/src/service/account_service.rs`

**业务逻辑来源（从以下位置提取）：**
- `command/auth.rs:61-129` — 密码认证后创建账号的流程（构建 `CreateAccountRequest`，调用 `AccountRepository::create`）
- `command/provider.rs:53-109` — 服务商检测逻辑（获取 provider、构建 DTO）
- `storage/service/account.rs:226-366` — `AccountRepository::create` 中的服务商检测、配置获取

**AccountService 结构体：**
```rust
pub struct AccountService {
    db: Arc<DbConn>,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    app_handle: AppHandle,
}

impl AccountService {
    pub fn new(...) -> Self { ... }

    /// 列出所有账号
    pub async fn list_accounts(&self) -> Result<Vec<AccountDto>> { ... }

    /// 获取单个账号
    pub async fn get_account(&self, id: i32) -> Result<Option<AccountDto>>{ ... }

    /// 创建账号（密码认证后）
    pub async fn create_account(&self, auth_response: PasswordAuthSuccess) -> Result<AccountDto> { ... }

    /// 删除账号
    pub async fn delete_account(&self, id: i32) -> Result<()> { ... }

    /// 检测服务商
    pub async fn detect_provider(&self, email: &str) -> Result<ProviderDetectionResult> { ... }

    /// 列出服务商
    pub fn list_providers(&self) -> Vec<ProviderInfoDto> { ... }
}
```

**验证：** `cargo check` 编译通过

---

### Task 2.3：实现 EmailService

**目标：** 将邮件相关业务逻辑从 Command 提取到 EmailService。

**新建文件：** `src-tauri/src/service/email_service.rs`

**业务逻辑来源：**
- `command/email.rs:50-123` — 获取账号 → 获取 provider → 获取 folder_mapping → 查询邮件
- `command/email.rs:145-194` — 获取邮件详情（同样的 provider 获取逻辑）
- `command/email.rs:393-433` — 获取文件夹统计

**EmailService 结构体：**
```rust
pub struct EmailService {
    db: Arc<DbConn>,
    provider_pool: Arc<ProviderPool>,
}

impl EmailService {
    pub fn new(...) -> Self { ... }

    /// 邮件列表
    pub async fn list_emails(&self, account_id: i32, folder: &str, page: u64, limit: u64) -> Result<EmailListResponse> { ... }

    /// 邮件详情
    pub async fn get_email(&self, id: i32) -> Result<EmailDetail> { ... }

    /// 搜索邮件
    pub async fn search_emails(&self, query: &str, account_id: Option<i32>, limit: Option<u64>) -> Result<Vec<SearchResult>> { ... }

    /// 标记已读
    pub async fn mark_as_read(&self, email_id: i32, is_read: bool) -> Result<()> { ... }

    /// 切换星标
    pub async fn toggle_star(&self, email_id: i32) -> Result<bool> { ... }

    /// 批量删除
    pub async fn batch_delete(&self, email_ids: Vec<i32>) -> Result<usize> { ... }

    /// 移动到文件夹
    pub async fn move_to_folder(&self, email_id: i32, folder: &str) -> Result<()> { ... }

    /// 文件夹统计
    pub async fn get_folder_stats(&self, account_id: i32) -> Result<Vec<FolderStat>> { ... }

    /// 发送邮件
    pub async fn send_email(&self, request: SendEmailRequest) -> Result<String> { ... }
}
```

**验证：** `cargo check` 编译通过

---

### Task 2.4：实现 SyncService

**目标：** 将同步逻辑封装到 SyncService。

**新建文件：** `src-tauri/src/service/sync_service.rs`

**业务逻辑来源：**
- `command/sync.rs:86-115` — 同步账号（创建 SyncManager、执行同步）
- `command/sync.rs:228-298` — 发送邮件的 SMTP 逻辑（移到 EmailService）

**SyncService 结构体：**
```rust
pub struct SyncService {
    db: Arc<DbConn>,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    app_handle: AppHandle,
}

impl SyncService {
    pub fn new(...) -> Self { ... }

    /// 同步账号（带进度）
    pub async fn sync_account(&self, account_id: i32) -> Result<SyncResult> { ... }
}
```

**注意：** SyncService 内部持有 `SyncManager`（或未来的 `SyncOrchestrator`）作为单例，不再每次创建。

**验证：** `cargo check` 编译通过

---

### Task 2.5：重构 Command 层为薄包装

**目标：** 所有 Command 函数只做一行委托调用 Service。

**修改文件：**
- `src-tauri/src/command/account.rs` — 委托 AccountService
- `src-tauri/src/command/email.rs` — 委托 EmailService
- `src-tauri/src/command/sync.rs` — 委托 SyncService
- `src-tauri/src/command/auth.rs` — 委托 AccountService + AuthManager
- `src-tauri/src/command/provider.rs` — 委托 AccountService

**示例（重构后的 command/email.rs）：**
```rust
#[tauri::command]
#[specta::specta]
pub async fn list_emails(
    service: tauri::State<'_, EmailService>,
    account_id: i32,
    folder: String,
    page: usize,
    limit: usize,
) -> Result<storage::EmailListResponse, MailError> {
    service.list_emails(account_id, &folder, page as u64, limit as u64).await
}
```

**修改 `lib.rs` setup 逻辑：**

将 Tauri managed state 从当前的 `DatabaseState`/`KeyringState`/`AuthManagerState`/`ProviderPoolState`/`OAuthSessionManagerState` 替换为 `AccountService`/`EmailService`/`SyncService`（同时保留 `AuthManagerState` 因为 auth 命令仍然需要直接访问 AuthManager）。

**验证：** `cargo build` 编译通过。所有现有测试通过。

---

### Task 2.6：重构 SyncManager 拆分为 SyncOrchestrator + SyncProgressEmitter

**目标：** 将 `SyncManager` 拆分为更小的组件。

**修改文件：**
- `src-tauri/src/sync/sync_manager.rs` → 重命名为 `orchestrator.rs`
- 新建: `src-tauri/src/sync/progress.rs`
- 修改: `src-tauri/src/sync/mod.rs`

**SyncOrchestrator（从 SyncManager 提取）：**
```rust
pub struct SyncOrchestrator {
    db: Arc<DbConn>,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    folder_sync: Arc<FolderSyncDispatcher>,
    mail_processor: Arc<MailProcessor>,
    change_detector: Arc<ChangeDetector>,
    progress: Arc<SyncProgressEmitter>,
}
```

**SyncProgressEmitter（从 SyncManager 提取）：**
```rust
pub struct SyncProgressEmitter {
    app_handle: AppHandle,
}

impl SyncProgressEmitter {
    pub fn emit(&self, account_id: i32, stage: &str, ...) { ... }
}
```

**验证：** `cargo test --lib` 全部通过

---

## 阶段 3：前端重建（Svelte 5）

### Task 3.1：清理 Vue 前端

**目标：** 删除现有 Vue 前端目录和配置。

**具体操作：**

1. 删除 `src/` 目录（整个 Vue 前端）
2. 删除 `public/` 目录
3. 删除 `dist/` 目录
4. 删除以下文件（如果存在）：
   - `package.json`
   - `package-lock.json`
   - `yarn.lock`
   - `vite.config.ts`
   - `tsconfig.json`
   - `tsconfig.node.json`
   - `svelte.config.js`（旧）
   - `index.html`

**不要删除：**
- `node_modules/`（后面用新包管理器重新安装）
- `.gitignore`

**验证：** 确认 `src/` 已删除。后端仍然可以编译（`cargo build`）

---

### Task 3.2：初始化 Svelte 5 项目

**目标：** 在 `src/` 目录创建全新的 Svelte 5 + Vite + TypeScript 项目。

**具体操作：**

1. 创建新的 `package.json`:
   ```json
   {
     "name": "postium-mail",
     "private": true,
     "version": "0.1.0",
     "type": "module",
     "scripts": {
       "dev": "vite dev",
       "build": "vite build",
       "preview": "vite preview"
     },
     "dependencies": {
       "@tauri-apps/api": "^2",
       "@tauri-apps/plugin-keyring": "^2"
       "@tauri-apps/plugin-opener": "^2"
       "@tauri-apps/plugin-positioner": "^2"
       "@tauri-apps/plugin-single-instance": "^2"
       "svelte": "^5"
       "clsx": "^2"
       "tailwind-merge": "^3"
       "tailwind-variants": "^1"
       "mode-watcher": "^1"
       "bits-ui": "^2"
       "lucide-svelte": "^1"
       "formsnap": "^2"
       "date-fns": "^4"
       "tw-animate-css": "^1"
     },
     "devDependencies": {
       "@sveltejs/vite-plugin-svelte": "^5",
       "@sveltejs/vite-plugin-svelte-inspector": "^5",
       "@tailwindcss/vite": "^4",
       "@tailwindcss/language-server": "^4",
       "tailwindcss": "^4",
       "typescript": "^5.6",
       "vite": "^6",
       "vite-plugin-tauri": "^2",
       "@svitejs/adapter-static": "^3"
     }
   }
   ```

2. 创建 `vite.config.ts`:
   ```typescript
import { sveltekit } from '@sveltejs/kit/vite';
   import { defineConfig } from 'vite';

   export default defineConfig({
     plugins: [sveltekit()],
     clearScreen: false,
     server: {
       port: 1420,
       strictPort: true,
     },
   });
   ```

   注意：由于 Tauri 不使用 SvelteKit 的路由和服务器功能，考虑直接使用 `@sveltejs/vite-plugin-svelte` 而非 SvelteKit。需要根据 Tauri 的集成方式决定。


   实际上，对于 Tauri 应用，推荐使用纯 Svelte（非 SvelteKit）模式：
   ```typescript
   // vite.config.ts（Tauri 纯 Svelte 模式）
   import { svelte } from '@sveltejs/vite-plugin-svelte';
   import { defineConfig } from 'vite';

   export default defineConfig({
     plugins: [svelte()],
     clearScreen: false,
     server: {
       port: 1420,
       strictPort: true,
     },
   });
   ```

3. 创建 `tsconfig.json`
4. 创建 `index.html`（Vite 入口）
5. 创建 `src/main.ts`（Svelte 入口）
6. 创建 `src/App.svelte`（根组件，最小化）

**验证：** `npm install && npm run dev` 启动成功

---

### Task 3.3：安装配置 shadcn-svelte + Tailwind CSS v4

**目标：** 集成 shadcn-svelte 和 Tailwind CSS v4。

**具体操作:**

1. 运行 `npx shadcn-svelte@latest init`（选择 Svelte 5, Tailwind v4 等）
2. 这会创建：
   - `src/app.css`（Tailwind + shadcn CSS 变量）
   - `components.json`（shadcn 配置）
   - `src/lib/utils.ts`（cn 工具函数）
3. 手动安装需要的 shadcn 组件：
   ```bash
   npx shadcn-svelte@latest add button card dialog dropdown input label separator sheet tooltip scroll-area avatar badge
   ```

**验证：** 启动应用能看到 shadcn 组件样式

---

### Task 3.4：配置 bindings.ts 集成

**目标：** 将 Tauri Specta 生成的 bindings 集成到前端。

**具体操作:**

1. 修改 `src-tauri/src/lib.rs` 中 bindings 导出路径为 `../src/lib/bindings.ts`
2. 确保 `src/lib/bindings.ts` 在 `.gitignore` 中（因为是自动生成的）
3. 在需要调用后端的 Store 中 import:
   ```typescript
   import { commands } from '$lib/bindings';
   ```

**验证：** 调用 `commands.listAccounts()` 能返回数据

---

### Task 3.5：实现布局组件

**目标：** 实现应用的核心布局。

**新建文件：**
- `src/lib/components/layout/AppShell.svelte` — 三栏布局（sidebar + list + detail）
- `src/lib/components/layout/TitleBar.svelte` — 自定义标题栏 + 窗口控制
- `src/lib/components/layout/Sidebar.svelte` — 文件夹导航
- `src/lib/components/layout/WindowControls.svelte` — 最小化/最大化/关闭按钮

**AppShell 布局：**
```
┌──────────────────────────────────────────────┐
│ TitleBar (drag region + window controls)      │
├──────┬───────────────┬───────────────────────┤
│      │               │                       │
│ Side │  EmailList    │    EmailDetail         │
│ bar  │               │                       │
│      │               │                       │
│      │               │                       │
└──────┴───────────────┴───────────────────────┘
```

**验证：** 窗口可以拖动、最小化、最大化、关闭

---

### Task 3.6：实现 Store 层

**目标：** 用 Svelte 5 runes 实现状态管理。

**新建文件：**
- `src/lib/stores/account.svelte.ts` — 账号状态
- `src/lib/stores/email.svelte.ts` — 邮件状态
- `src/lib/stores/sync.svelte.ts` — 同步状态
- `src/lib/stores/ui.svelte.ts` — UI 状态

**account.svelte.ts 核心结构：**
```typescript
import { commands } from '$lib/bindings';

let accounts = $state<AccountDto[]>([]);
let loading = $state(false);
let selectedAccountId = $state<number | null>(null);
let selectedFolder = $state('inbox');

let selectedAccount = $derived(
    accounts.find(a => a.id === selectedAccountId) ?? null
);

async function loadAccounts() { ... }
async function deleteAccount(id: number) { ... }
function selectAccount(id: number) { ... }
function selectFolder(folder: string) { ... }

export function getAccountStore() {
    return { /* getters + methods */ };
}
```

**sync.svelte.ts 核心结构：**
```typescript
import { events } from '$lib/bindings';

let syncingAccounts = $state<Set<number>>(new Set());
let syncProgresses = $state<Map<number, SyncProgress>>(new Map());

// 监听同步进度事件
$effect(() => {
    // 使用 events.syncProgress.listen()
});

export function getSyncStore() { ... }
```

**验证：** Store 状态可以在组件间正确共享

---

### Task 3.7：实现账号管理 UI

**目标：** 实现添加账号、账号列表、删除账号。

**新建文件：**
- `src/lib/components/account/AccountList.svelte` — 侧边栏中的账号列表
- `src/lib/components/account/AddAccountModal.svelte` — 添加账号的模态框

**AddAccountModal 流程：**
1. 输入邮箱地址
2. 调用 `commands.detectProvider({ email })` 检测服务商
3. 根据推荐认证类型显示表单：
   - OAuth: 显示 OAuth 登录按钮 → 调用 `commands.startAuthCommand`
   - 密码: 显示密码输入框 + IMAP/SMTP 配置 → 调用 `commands.startAuthCommand`
4. 成功后关闭模态框并刷新账号列表

**验证：** 可以添加和删除账号

---

### Task 3.8：实现邮件列表和详情

**目标：** 实现邮件浏览的核心功能。

**新建文件：**
- `src/lib/components/email/EmailList.svelte` — 邮件列表（分页）
- `src/lib/components/email/EmailListItem.svelte` — 单封邮件项
- `src/lib/components/email/EmailDetail.svelte` — 邮件详情（HTML 渲染）

**EmailList 功能：**
- 分页加载（20封/页）
- 显示发件人、主题、日期、已读/星标状态
- 点击选中，右侧显示详情
- 支持标星、已读标记

**EmailDetail 功能：**
- 渲染 HTML 邮件内容（使用 iframe sandbox 或 DOMPurify）
- 显示发件人、收件人、日期
- 操作按钮：回复、转发、删除、归档

**验证：** 可以浏览邮件列表和查看邮件详情

---

### Task 3.9：实现同步进度和邮件操作

**目标：** 实现同步触发和进度显示，以及邮件操作。

**新建文件：**
- `src/lib/components/common/SyncProgress.svelte` — 同步进度条

**功能：**
- 手动触发同步：调用 `commands.syncAccountWithProgress`
- 监听 `sync-progress` 事件更新进度
- 显示同步阶段（连接中/同步文件夹/同步邮件/完成）

**邮件操作：**
- 标记已读/未读：`commands.markAsRead`
- 切换星标：`commands.toggleStar`
- 删除：`commands.deleteEmails`
- 移动到文件夹：`commands.moveEmailToFolder`

**验证：** 同步能触发并显示进度，邮件操作正常

---

### Task 3.10：实现发送邮件

**目标：** 实现简单的邮件发送功能。

**新建文件：**
- `src/lib/components/email/ComposeModal.svelte` — 写邮件模态框

**功能：**
- 收件人、主题、正文输入
- 纯文本编辑器（第一版不做富文本）
- 调用 `commands.sendEmail`
- 发送成功后关闭

**验证：** 可以发送邮件

---

### Task 3.11：实现国际化和主题切换

**目标：** 添加中英文切换和深色/浅色主题。

**新建文件：**
- `src/lib/utils/i18n.ts` — 国际化工具
- `src/lib/locales/zh-CN.ts` — 中文翻译
- `src/lib/locales/en-US.ts` — 英文翻译
- `src/lib/components/common/LanguageSelector.svelte`

**i18n 实现：**
```typescript
// src/lib/utils/i18n.ts
import zhCN from '$lib/locales/zh-CN';
import enUS from '$lib/locales/en-US';

const locales: Record<string, Record<string, string>> = {
    'zh-CN': zhCN,
    'en-US': enUS,
};

let currentLocale = $state('zh-CN');

export function t(key: string): string {
    return locales[currentLocale]?.[key] ?? key;
}

export function setLocale(locale: string) {
    currentLocale = locale;
}
```

**主题切换：** 使用 Tailwind CSS 的 `dark` class + `mode-watcher` 库自动检测系统主题

**验证：** 语言和主题切换正常工作

---

### Task 3.12：集成 Tauri 系统托盘

**目标：** 确保系统托盘在 Svelte 前端下正常工作。

**说明：** 系统托盘是 Rust 后端功能（`sys/tray.rs`），与前端框架无关，但需要确认：
1. 托盘菜单事件能正确触发
2. 前端可以通过 Tauri API 监听托盘事件

**验证：** 最小化到托盘、托盘菜单点击恢复正常

---

## 阶段 4：清理与优化

### Task 4.1：清理 Vue 依赖

**目标：** 清理所有 Vue 相关依赖和配置。

**具体操作：**

1. 检查 `package.json` 中是否还有 Vue 相关依赖（naive-ui, pinia, vue-router, vue-i18n 等），确保已移除
2. 删除 Vue 相关的配置文件（如果残留）
3. 清理 `node_modules/` 和 lock 文件，重新安装
4. 确认 `npm run build` 正常

**验证：** `npm run build` 成功，无 Vue 依赖残留

---

### Task 4.2：清理未使用的后端模块

**目标：** 移除明确不会使用的代码。

**具体操作：**

1. 评估 `engine/` 模块：
   - `FlowEngine` 未集成，但计划后续使用 — 保留
   - `NotificationManager` 未使用 — 保留（后续需要）
   - `ConflictResolver` 未使用 — 保留（后续需要）
2. 评估 `command/flow_engine.rs` — 清理（如果内容已迁移到 service 层）
3. 评估 `command/connection.rs` — 清理（如果未使用）

**注意：** 只删除明确不需要的代码，不进行「顺便重构」

**验证：** `cargo build` 编译通过

---

### Task 4.3：更新项目文档

**目标：** 更新项目文档反映新架构。

**修改文件：**
- `src-tauri/src/lib.rs` — 更新模块文档注释
- `docs/architecture-design.md` — 更新架构图和说明
- `docs/implementation-plan.md` — 更新进度

**验证：** 文档内容与实际代码一致

---

### Task 4.4：端到端验证

**目标：** 完整的功能验证。

**验证清单：**
- [ ] 应用启动正常
- [ ] 添加邮箱账号（OAuth + 密码）
- [ ] 账号列表显示正常
- [ ] 邮件同步触发，进度显示正常
- [ ] 邮件列表浏览（分页）
- [ ] 邮件详情查看（HTML 渲染）
- [ ] 邮件搜索
- [ ] 标星、已读标记
- [ ] 删除邮件
- [ ] 移动邮件
- [ ] 发送邮件
- [ ] 系统托盘
- [ ] 语言切换
- [ ] 主题切换
- [ ] 窗口拖动、最小化、最大化、关闭
