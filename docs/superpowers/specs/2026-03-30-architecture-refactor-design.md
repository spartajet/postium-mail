# Postium Mail 全面架构重构设计文档

> 日期：2026-03-30
> 状态：草案

## 1. 背景与动机

Postium Mail 当前架构存在以下核心问题：

1. **缺少 Service 层** — 业务逻辑散落在 Command handler 和 Repository 中，职责边界模糊
2. **SyncManager 是 God Object** — 同时负责连接管理、同步逻辑、邮件处理、进度通知
3. **MailProvider Trait 职责混乱** — 配置、认证、检测、工具函数全塞在一个 trait 里
4. **前端架构混乱** — Vue 3 的 Store 过于臃肿（account.ts 612 行），App.vue 承担了太多职责
5. **错误处理粗暴** — 所有错误转为字符串，前端无法区分错误类型
6. **类型安全缺失** — 前端手动调用 `invoke()`，无编译时类型检查

## 2. 重构目标

- **后端**：引入清晰的四层架构（Command → Service → Domain → Infrastructure）
- **前端**：用 Svelte 5 + shadcn-svelte + Tailwind CSS 完全重写
- **类型安全**：使用 Tauri Specta 自动生成 TypeScript 绑定
- **渐进式重构**：在现有项目上修改，保留 Git 历史，不新建项目

## 3. 技术选型

### 3.1 后端（保持 Rust + Tauri 2）

| 组件 | 版本 | 用途 |
|------|------|------|
| tauri | 2.x | 桌面应用框架 |
| tauri-specta | 2.0.0-rc.x | 类型安全命令绑定 |
| specta-typescript | latest | TypeScript 类型导出 |
| sea-orm | 2.0.0-rc.x | ORM（保持不变） |
| tokio | 1.x | 异步运行时 |

### 3.2 前端（全新）

| 组件 | 版本 | 用途 |
|------|------|------|
| Svelte | 5.x | 前端框架（runes 响应式） |
| shadcn-svelte | next | UI 组件库 |
| Tailwind CSS | v4 | 样式系统 |
| TypeScript | 5.x | 类型系统 |
| Vite | 6.x | 构建工具 |

## 4. 后端架构设计

### 4.1 四层架构

```
┌─────────────────────────────────────────────┐
│  Command 层 (src-tauri/src/command/)         │
│  - 薄 IPC 包装，零业务逻辑                    │
│  - #[specta::specta] 注解                    │
│  - 统一错误序列化                             │
├─────────────────────────────────────────────┤
│  Service 层 (src-tauri/src/service/)         │
│  - AccountService / EmailService / SyncService│
│  - 封装业务逻辑和事务边界                      │
│  - 协调 Domain 层多个模块                     │
├─────────────────────────────────────────────┤
│  Domain 层 (现有模块重组)                     │
│  - auth/  providers/  sync/  engine/         │
│  - 纯业务逻辑，不依赖 Tauri                   │
├─────────────────────────────────────────────┤
│  Infrastructure 层                           │
│  - storage/ (Repository 模式)                │
│  - protocols/ (IMAP/SMTP)                    │
│  - sys/ (日志、路径、托盘)                    │
└─────────────────────────────────────────────┘
```

### 4.2 模块重组

#### 4.2.1 新增 Service 层

```
src-tauri/src/service/
├── mod.rs                # 模块导出
├── account_service.rs    # 账号业务逻辑
├── email_service.rs      # 邮件操作业务逻辑
└── sync_service.rs       # 同步业务逻辑
```

**AccountService 职责：**
- 创建账号（服务商检测 → 配置获取 → 数据库保存 → Keyring 存储）
- 删除账号（数据库删除 → Keyring 清理 → 同步停止）
- 编辑账号配置
- 账号列表查询

**EmailService 职责：**
- 邮件列表查询（分页、过滤、排序）
- 邮件详情获取
- 邮件标记（已读/星标）
- 邮件删除/移动
- 邮件搜索（FTS5）
- 发送邮件（SMTP）

**SyncService 职责：**
- 触发账号同步
- 查询同步状态
- 同步进度事件发射
- 协调 SyncOrchestrator

#### 4.2.2 SyncManager 拆分

当前 `SyncManager` 拆分为：

```rust
// sync/orchestrator.rs - 同步流程编排
pub struct SyncOrchestrator {
    db: Arc<DbConn>,
    auth_manager: Arc<AuthManager>,
    provider_pool: Arc<ProviderPool>,
    folder_sync: Arc<FolderSyncDispatcher>,
    mail_processor: Arc<MailProcessor>,
    change_detector: Arc<ChangeDetector>,
}

// sync/progress.rs - 进度通知（独立）
pub struct SyncProgressEmitter {
    app_handle: AppHandle,
}

// sync/connection_pool.rs - IMAP 连接管理
pub struct ImapConnectionManager {
    // 管理连接生命周期、超时、重试
}
```

#### 4.2.3 MailProvider Trait 瘦身

```rust
// providers/traits.rs - 只保留配置方法
#[async_trait]
pub trait MailProvider: Send + Sync {
    fn info(&self) -> &ProviderInfo;
    fn imap_config(&self, email: &str) -> ImapServerConfig;
    fn smtp_config(&self, email: &str) -> SmtpServerConfig;
    fn capabilities(&self) -> ProviderCapabilities;
    fn folder_mapping(&self) -> StandardFolder;
}

// providers/detect.rs - 服务商检测（独立）
pub trait ProviderDetector: Send + Sync {
    fn domains(&self) -> &[&'static str];
    async fn detect(email: &str) -> bool;
}

// providers/oauth.rs - OAuth 支持（独立 trait）
pub trait OAuthProvider: MailProvider {
    fn oauth_config(&self) -> OAuthConfig;
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String;
    fn redirect_uri(&self, port: u16) -> String;
}
```

#### 4.2.4 统一错误处理

```rust
// error/types.rs
#[derive(Debug, thiserror::Error, specta::Type, serde::Serialize)]
#[serde(tag = "type", content = "message")]
pub enum MailError {
    #[error("账号未找到: {0}")]
    AccountNotFound(i32),

    #[error("认证失败: {0}")]
    AuthFailed(String),

    #[error("IMAP 连接失败: {0}")]
    ImapConnectionFailed(String),

    #[error("SMTP 发送失败: {0}")]
    SmtpSendFailed(String),

    #[error("同步失败: {0}")]
    SyncFailed(String),

    #[error("数据库错误: {0}")]
    DatabaseError(String),

    #[error("Keyring 错误: {0}")]
    KeyringError(String),

    #[error("服务商不支持: {0}")]
    ProviderNotSupported(String),

    #[error("参数无效: {0}")]
    InvalidParam(String),
}

// Command 层统一处理
impl From<MailError> for String {
    fn from(e: MailError) -> String {
        serde_json::to_string(&e).unwrap_or_else(|_| e.to_string())
    }
}
```

### 4.3 Tauri Specta 集成

```rust
// lib.rs
use tauri_specta::{collect_commands, Builder};
use specta_typescript::Typescript;

pub fn run() {
    let builder = Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            command::list_accounts,
            command::create_account,
            command::delete_account,
            command::start_auth,
            command::list_emails,
            command::get_email,
            command::search_emails,
            command::mark_as_read,
            command::toggle_star,
            command::delete_emails,
            command::move_email,
            command::sync_account,
            command::send_email,
        ]);

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/lib/bindings.ts")
        .expect("Failed to export typescript bindings");

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            // ... 初始化代码
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 4.4 Command 层示例（重构后）

```rust
// command/account.rs
#[tauri::command]
#[specta::specta]
pub async fn create_account(
    service: tauri::State<'_, AccountService>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    // 只有一行：委托给 Service
    service.create_account(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(
    service: tauri::State<'_, AccountService>,
) -> Result<Vec<AccountDto>, MailError> {
    service.list_accounts().await
}
```

### 4.5 Tauri 状态管理（重构后）

用 Service 实例替代当前的多个独立 State：

```rust
// setup 中
let account_service = Arc::new(AccountService::new(
    db_arc.clone(),
    auth_manager.clone(),
    provider_pool.clone(),
    app.handle().clone(),
));
let email_service = Arc::new(EmailService::new(db_arc.clone()));
let sync_service = Arc::new(SyncService::new(
    db_arc.clone(),
    auth_manager.clone(),
    provider_pool.clone(),
    app.handle().clone(),
));

app.manage(account_service);
app.manage(email_service);
app.manage(sync_service);
```

## 5. 前端架构设计（Svelte 5）

### 5.1 目录结构

```
src/
├── app.html                    # HTML 入口
├── app.css                     # Tailwind CSS v4 + shadcn 变量
├── lib/
│   ├── bindings.ts             # tauri-specta 自动生成，勿手动修改
│   ├── stores/                 # Svelte 5 响应式状态
│   │   ├── account.svelte.ts   # 账号状态
│   │   ├── email.svelte.ts     # 邮件状态
│   │   ├── sync.svelte.ts      # 同步状态
│   │   └── ui.svelte.ts        # UI 状态（主题、侧边栏等）
│   ├── components/
│   │   ├── ui/                 # shadcn-svelte 组件
│   │   ├── layout/
│   │   │   ├── AppShell.svelte     # 主布局
│   │   │   ├── Sidebar.svelte      # 侧边栏
│   │   │   ├── TitleBar.svelte     # 自定义标题栏
│   │   │   └── WindowControls.svelte
│   │   ├── email/
│   │   │   ├── EmailList.svelte    # 邮件列表
│   │   │   ├── EmailDetail.svelte  # 邮件详情
│   │   │   ├── EmailListItem.svelte
│   │   │   └── ComposeModal.svelte
│   │   ├── account/
│   │   │   ├── AccountList.svelte
│   │   │   └── AddAccountModal.svelte
│   │   └── common/
│   │       ├── SyncProgress.svelte
│   │       ├── SearchBar.svelte
│   │       └── LanguageSelector.svelte
│   ├── utils/
│   │   ├── i18n.ts             # 国际化
│   │   ├── format.ts           # 日期/大小格式化
│   │   └── theme.ts            # 主题管理
│   └── types/
│       └── index.ts            # 前端特有类型（bindings.ts 覆盖大部分）
└── routes/
    └── +page.svelte            # 主页面
```

### 5.2 Store 设计（Svelte 5 Runes）

```typescript
// lib/stores/account.svelte.ts
import { commands, type AccountDto } from '$lib/bindings';

// 使用 Svelte 5 runes 的状态管理
let accounts = $state<AccountDto[]>([]);
let loading = $state(false);
let selectedAccountId = $state<number | null>(null);

// 派生状态
let selectedAccount = $derived(
    accounts.find(a => a.id === selectedAccountId) ?? null
);

async function loadAccounts() {
    loading = true;
    try {
        accounts = await commands.listAccounts();
    } finally {
        loading = false;
    }
}

async function createAccount(request: CreateAccountRequest) {
    const account = await commands.createAccount(request);
    accounts.push(account);
}

async function deleteAccount(id: number) {
    await commands.deleteAccount(id);
    accounts = accounts.filter(a => a.id !== id);
}

// 导出为函数，每个组件获取同一个响应式引用
export function getAccountStore() {
    return {
        get accounts() { return accounts; },
        get loading() { return loading; },
        get selectedAccountId() { return selectedAccountId; },
        get selectedAccount() { return selectedAccount; },
        set selectedAccountId(id: number | null) { selectedAccountId = id; },
        loadAccounts,
        createAccount,
        deleteAccount,
    };
}
```

### 5.3 组件通信模式

- **父子组件**：props + 事件回调（Svelte 标准模式）
- **跨组件**：通过 Store 函数获取共享状态
- **Store 之间**：不直接引用，需要协调时在组件层处理

### 5.4 Tauri 事件监听（Svelte 5 方式）

```svelte
<!-- SyncProgress.svelte -->
<script lang="ts">
    import { events } from '$lib/bindings';
    import { getSyncStore } from '$lib/stores/sync.svelte';

    let syncStore = getSyncStore();

    $effect(() => {
        // $effect 自动清理：返回的函数在重新执行或组件销毁时调用
        const unlisten = events.syncProgress.listen((event) => {
            syncStore.updateProgress(event.payload);
        });

        return () => {
            unlisten.then(fn => fn());
        };
    });
</script>
```

### 5.5 国际化

使用轻量的自定义 i18n 方案（不需要 i18n 库）：

```typescript
// lib/utils/i18n.ts
import zhCN from './locales/zh-CN';
import enUS from './locales/en-US';

const locales = { 'zh-CN': zhCN, 'en-US': enUS };
let currentLocale = $state('zh-CN');

export function t(key: string): string {
    return locales[currentLocale]?.[key] ?? key;
}

export function setLocale(locale: string) {
    currentLocale = locale;
}
```

## 6. 核心功能范围

### 6.1 第一版包含

| 功能 | 说明 |
|------|------|
| 多账号管理 | 添加/删除/编辑，OAuth + 密码认证 |
| 邮件列表 | 按文件夹浏览，分页加载 |
| 邮件详情 | 查看 HTML/纯文本邮件 |
| 邮件同步 | 手动触发，实时进度显示 |
| 邮件搜索 | FTS5 全文搜索 |
| 邮件操作 | 标星、已读标记、删除、移动 |
| 发送邮件 | SMTP 发送，富文本编辑 |
| 系统托盘 | 最小化到托盘 |
| 自定义标题栏 | 无边框窗口 |
| 国际化 | 中文/英文 |
| 深色/浅色主题 | 跟随系统或手动切换 |
| 服务商检测 | 自动识别邮箱域名 |

### 6.2 暂不包含

- AI 聊天助手
- 日历/日程管理
- 工作流编辑器
- 附件管理（显示但无下载进度）
- 拖拽操作
- 快捷键系统

## 7. 重构步骤

### 阶段 1：后端基础层重构
1. 统一错误类型 `MailError`（加 specta 序列化支持）
2. 清理 `MailProvider` trait，拆分为 `MailProvider` + `OAuthProvider` + `ProviderDetector`
3. 引入 `tauri-specta`，配置 Builder 和 bindings 导出
4. 验证：现有测试全部通过

### 阶段 2：引入 Service 层
1. 创建 `service` 模块
2. 实现 `AccountService`，从 Command/Repository 中提取业务逻辑
3. 实现 `EmailService`
4. 实现 `SyncService`
5. 重构 Command 层为薄包装
6. 重构 `SyncManager` 拆分为 `SyncOrchestrator` + `SyncProgressEmitter`
7. 验证：现有测试全部通过，bindings.ts 正确生成

### 阶段 3：前端重建
1. 删除现有 `src/` 前端目录
2. 初始化 Svelte 5 + Vite 项目
3. 安装配置 shadcn-svelte + Tailwind CSS v4
4. 引入 `bindings.ts`
5. 实现布局组件（TitleBar、Sidebar、AppShell）
6. 实现 Store（account、email、sync、ui）
7. 实现核心页面（账号管理、邮件列表、邮件详情）
8. 实现同步进度、邮件操作
9. 实现发送邮件
10. 国际化 + 主题切换
11. 验证：端到端功能测试

### 阶段 4：清理与优化
1. 删除废弃的 Vue 依赖（package.json 清理）
2. 删除未使用的模块（flow_engine 未集成部分）
3. 更新文档
4. CI/CD 调整

## 8. 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| 后端重构破坏现有功能 | 每步跑 456+ 测试用例验证 |
| Tauri Specta 与 SeaORM 类型不兼容 | specta 支持自定义类型映射，已有 serde 兼容 |
| shadcn-svelte v2 不稳定 | 锁定 next 版本，关注社区更新 |
| Svelte 5 runes 学习曲线 | runes 语法比 Vue Composition API 更简单 |
| 数据库迁移冲突 | migration 模块独立 crate，不受影响 |

## 9. 不变的部分

以下模块/文件在重构中保持不变：

- `migration/` — 数据库迁移（独立 crate）
- `src-tauri/src/storage/models/` — SeaORM 实体模型
- `src-tauri/src/protocols/` — IMAP/SMTP 协议实现
- `src-tauri/src/auth/` — 认证模块（小调整，不改结构）
- `src-tauri/src/sys/` — 系统功能
- `src-tauri/tests/` — 测试用例
- `src-tauri/.env` — OAuth 配置
- `src-tauri/capabilities/` — Tauri 权限
