# Postium Mail 应用架构设计文档

> **版本**: 1.0.0
> **更新日期**: 2026-03-19
> **目标版本**: v2.0.0

---

## 📋 目录

1. [概述](#概述)
2. [架构设计](#架构设计)
3. [后端模块架构](#后端模块架构)
4. [前端模块架构](#前端模块架构)
5. [数据流设计](#数据流设计)
6. [部署架构](#部署架构)

---

## 概述

Postium Mail 是一款基于 Tauri + Vue 3 的跨平台桌面邮件客户端，采用 Rust 后端 + Vue 前端的混合架构设计。

### 设计原则

- **关注点分离**: 每个模块只负责单一职责
- **依赖注入**: 使用 Arc + 泛型实现灵活的依赖注入
- **可测试性**: 所有核心组件都支持单元测试
- **可扩展性**: 支持动态添加邮件服务商和协议扩展
- **类型安全**: 充分利用 Rust 的类型系统保证安全

---

## 架构设计

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Postium Mail 架构                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    前端层 (Frontend)                          │  │
│  │  Vue 3 + TypeScript + TailwindCSS + shadcn/ui                │  │
│  │  ├──  views/        - 页面视图                                │  │
│  │  ├── components/   - 可复用组件                              │  │
│  │  ├── stores/       - Pinia 状态管理                          │  │
│  │  └── api/          - Tauri API 调用封装                      │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕ IPC                                 │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                   命令层 (Commands)                           │  │
│  │  Tauri Commands - 薄层转换，参数验证，结果序列化              │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                   引擎层 (Engine)                             │  │
│  │  ├── FlowEngine      - 核心引擎，统一编排                     │  │
│  │  ├── TaskScheduler   - 定时任务调度                           │  │
│  │  └── NotificationManager - 通知管理                          │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                   业务层 (Services)                           │  │
│  │  ├── auth/           - 认证管理                               │  │
│  │  ├── providers/      - 服务商抽象                             │  │
│  │  ├── sync/           - 同步引擎                               │  │
│  │  ├── protocols/      - 协议实现                               │  │
│  │  ├── storage/        - 数据存储                               │  │
│  │  └── services/       - 兼容服务层                             │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              ↕                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                   数据层 (Data)                               │  │
│  │  ├── SQLite (sea-orm) - 关系数据存储                         │  │
│  │  ├── Keyring         - 敏感信息存储                           │  │
│  │  └── Cache           - 临时缓存                               │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 后端模块架构

### 目录结构

```
src-tauri/src/
├── engine/                 # 引擎层 - 协调中心
│   ├── mod.rs
│   ├── flow_engine.rs      - 核心引擎
│   ├── task_scheduler.rs   - 定时任务调度器
│   ├── notification_manager.rs - 通知管理器
│   ├── conflict_resolver.rs - 冲突解决器 ✅
│   └── operation_manager.rs - 操作管理器 ✅
│
├── auth/                   # 认证模块
│   ├── mod.rs
│   ├── auth_manager.rs     - 统一认证入口
│   ├── token_manager.rs    - Token 生命周期管理
│   ├── oauth_handler.rs    - OAuth 2.0 处理器
│   ├── password_auth.rs    - 密码认证
│   └── pkce_verifier_store.rs - PKCE 验证器存储
│
├── providers/              # 服务商抽象层
│   ├── mod.rs
│   ├── traits.rs           - MailProvider trait
│   ├── provider_pool.rs    - 服务商池
│   ├── oauth_utils.rs      - OAuth 工具函数
│   ├── personal/           # 个人邮箱
│   │   ├── gmail.rs
│   │   ├── outlook.rs
│   │   ├── yahoo.rs
│   │   ├── native_163.rs
│   │   ├── native_qq.rs
│   │   └── native_icloud.rs
│   └── enterprise/         # 企业邮箱
│       ├── microsoft_365.rs
│       ├── google_workspace.rs
│       └── custom.rs
│
├── sync/                   # 同步引擎
│   ├── mod.rs
│   ├── sync_manager.rs     - 同步管理器
│   ├── delta_sync.rs       - 增量同步
│   ├── change_detector.rs  - 变更检测
│   ├── folder_manager.rs   - 文件夹管理
│   ├── mail_processor.rs   - 邮件处理
│   ├── sync_state.rs       - 同步状态管理
│   └── sync_error.rs       - 同步错误管理
│
├── protocols/              # 协议层
│   ├── mod.rs
│   ├── imap/               # IMAP 协议
│   │   ├── client.rs       - 异步 IMAP 客户端
│   │   ├── types.rs        - 类型定义
│   │   ├── error.rs        - 错误类型
│   │   ├── auth.rs         - 认证类型
│   │   ├── condstore_helpers.rs - CONDSTORE 辅助
│   │   ├── idle_manager.rs - IDLE 管理
│   │   ├── parser.rs       - 邮件解析
│   │   └── service.rs      - IMAP 服务包装
│   └── smtp/               # SMTP 协议
│       ├── sender.rs       - SMTP 发送器
│       └── types.rs        - 类型定义
│
├── storage/                # 存储层 ✅ 已整合
│   ├── mod.rs
│   ├── models/             # 数据模型 ✅ 已迁移
│   │   ├── mod.rs
│   │   ├── account.rs      - 账号实体模型
│   │   ├── email.rs        - 邮件实体模型
│   │   ├── folder.rs       - 文件夹实体模型
│   │   ├── attachment.rs   - 附件实体模型
│   │   ├── sync_state.rs   - 同步状态模型
│   │   └── sync_error.rs   - 同步错误模型
│   ├── migration/          # 数据库迁移 ✅ 已迁移
│   │   ├── mod.rs
│   │   ├── m001_20250314_init.rs
│   │   ├── m002_20250314_add_oauth_fields.rs
│   │   ├── m003_20250315_add_sync_tables.rs
│   │   ├── m004_20250315_remove_sensitive_fields.rs
│   │   ├── m005_20250315_add_imap_metadata.rs
│   │   ├── m006_20250315_add_sync_operations.rs
│   │   ├── m007_20250317_add_account_types.rs
│   │   └── m008_20250317_add_modseq_support.rs
│   ├── accounts.rs          - 账号存储
│   ├── emails.rs            - 邮件存储
│   ├── folders.rs           - 文件夹存储
│   ├── search.rs            - 邮件搜索 ✅ 已迁移
│   ├── cache.rs             - 缓存管理
│   └── database.rs          - 数据库连接
│
├── command/                # Tauri 命令层
│   ├── mod.rs
│   ├── account.rs          - 账号命令
│   ├── email.rs            - 邮件命令
│   ├── folder.rs           - 文件夹命令
│   ├── sync.rs             - 同步命令
│   ├── oauth.rs            - OAuth 命令
│   ├── connection.rs       - 连接命令
│   └── flow_engine.rs      - FlowEngine 命令
│
├── error.rs                # 错误类型
├── lib.rs                  # 库入口
└── main.rs                 # 应用入口
```

### 核心模块说明

#### 1. 引擎层 (Engine)

**FlowEngine** - 核心协调引擎

```rust
pub struct FlowEngine {
    provider_pool: Arc<ProviderPool>,
    auth_manager: Arc<AuthManager>,
    sync_manager: Arc<SyncManager>,
    task_scheduler: Arc<TaskScheduler>,
    notification_manager: Arc<NotificationManager>,
    running: Arc<AtomicBool>,
}
```

职责:
- 统一管理所有核心组件
- 提供启动/停止控制
- 协调各模块之间的交互
- 统一的状态报告

**TaskScheduler** - 定时任务调度器

```rust
pub struct TaskScheduler {
    scheduled_tasks: Arc<RwLock<HashMap<i32, ScheduledTask>>>,
    handles: Arc<RwLock<HashMap<i32, JoinHandle<()>>>>,
}
```

职责:
- 管理定时同步任务
- 支持任务的启动、暂停、恢复、删除
- 优雅关闭和清理

**NotificationManager** - 通知管理器

```rust
pub struct NotificationManager {
    recent_notifications: Arc<RwLock<Vec<NotificationRecord>>>,
    stats: Arc<RwLock<NotificationStats>>,
}
```

职责:
- 新邮件通知去重和合并
- 发送 Tauri 事件通知前端
- 统计追踪

---

#### 2. 认证模块 (Auth)

**AuthManager** - 统一认证入口

```rust
pub struct AuthManager {
    db: Arc<DbConn>,
    keyring: Arc<Keyring>,
    provider_pool: Arc<ProviderPool>,
    token_manager: Arc<TokenManager>,
    oauth_handler: Arc<OAuthHandler>,
}
```

核心方法:
- `get_oauth_url()` - 获取 OAuth 授权 URL
- `authenticate_oauth()` - OAuth 认证
- `authenticate_password()` - 密码认证
- `refresh_token()` - Token 刷新
- `get_imap_auth()` / `get_smtp_auth()` - 获取认证信息

**TokenManager** - Token 生命周期管理

```rust
pub struct TokenManager {
    db: Arc<DbConn>,
    keyring: Arc<Keyring>,
    memory_cache: Arc<RwLock<HashMap<i32, TokenData>>>,
    access_token_cache: Arc<RwLock<HashMap<i32, AccessTokenCache>>>,
}
```

职责:
- Token 存储到 Keyring（只存储 refresh_token）
- 内存缓存优化性能
- 自动过期检测和刷新
- access_token 缓存（5 分钟 TTL）

**OAuthHandler** - OAuth 2.0 处理器

```rust
pub struct OAuthHandler {
    pkce_store: Arc<PkceVerifierStore>,
    client: ReqwestClient,
}
```

职责:
- 完整的 OAuth 2.0 + PKCE 流程
- 授权码交换
- Token 刷新
- XOAUTH2 字符串生成

---

#### 3. 服务商抽象 (Providers)

**MailProvider Trait** - 服务商统一接口

```rust
#[async_trait]
pub trait MailProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn provider_name(&self) -> String;
    fn account_type(&self) -> AccountType;
    fn auth_types(&self) -> Vec<AuthType>;
    fn default_imap_config(&self) -> ImapConfig;
    fn default_smtp_config(&self) -> SmtpConfig;
    fn oauth_config(&self) -> Option<&OAuthConfig>;
    fn enterprise_config(&self) -> Option<&EnterpriseConfig>;
    fn capabilities(&self) -> &ProviderCapabilities;
    async fn detect(&self, email: &str) -> Result<bool, MailError>;
    fn supported_domains(&self) -> Vec<&'static str>;
    fn box_clone(&self) -> Box<dyn MailProvider>;
}
```

支持的服务商:

| 类型 | 服务商 | OAuth | IDLE | CONDSTORE |
|------|--------|-------|------|-----------|
| 个人 | Gmail | ✅ | ✅ | ✅ |
| 个人 | Outlook | ✅ | ✅ | ✅ |
| 个人 | Yahoo | ✅ | ✅ | ⚠️ |
| 个人 | 163/QQ/iCloud | ⚠️ | ⚠️ | ⚠️ |
| 企业 | Microsoft 365 | ✅ | ✅ | ✅ |
| 企业 | Google Workspace | ✅ | ✅ | ✅ |
| 自定义 | Custom | ❌ | ⚠️ | ⚠️ |

---

#### 4. 同步引擎 (Sync)

**SyncManager** - 同步管理器

```rust
pub struct SyncManager {
    db: Arc<DbConn>,
    provider_pool: Arc<ProviderPool>,
    auth_manager: Arc<AuthManager>,
    delta_sync: Arc<DeltaSync>,
    folder_manager: Arc<FolderManager>,
    mail_processor: Arc<MailProcessor>,
    sync_state_manager: Arc<SyncStateManager>,
    sync_error_manager: Arc<SyncErrorManager>,
}
```

同步流程:
1. 获取账号信息
2. 检测服务商配置
3. 连接 IMAP 服务器
4. 同步文件夹列表
5. 增量同步邮件
6. 更新同步状态

**DeltaSync** - 增量同步

```rust
pub enum SyncStrategy {
    Condstore,    // 使用 MODSEQ 增量同步
    UidSearch,    // 使用 UID 搜索
    FullSync,     // 完全同步
}
```

职责:
- 检测服务器 CONDSTORE 支持
- 选择最优同步策略
- 执行增量同步

**ChangeDetector** - 变更检测

```rust
pub struct ChangeDetectionResult {
    pub new_emails: Vec<UidSet>,
    pub deleted_emails: Vec<UidSet>,
    pub flag_changes: Vec<FlagChange>,
}
```

职责:
- 检测新邮件
- 检测删除的邮件
- 检测标志变更

---

#### 5. 协议层 (Protocols)

**AsyncImapClient** - 异步 IMAP 客户端

```rust
pub struct AsyncImapClient {
    client: Option<Client<Stream>>,
    account_id: Option<i32>,
    current_folder: Option<String>,
}
```

核心功能:
- 异步 IMAP 连接
- OAuth 2.0 / XOAUTH2 认证
- CONDSTORE 支持
- IDLE 实时监听

**SmtpSender** - SMTP 发送器

```rust
pub struct SmtpSender {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
}
```

核心功能:
- 异步 SMTP 发送
- OAuth 2.0 / XOAUTH2 认证
- 附件支持
- 发送状态追踪

---

#### 6. 存储层 (Storage)

**存储模块职责**:
- `accounts` - 账号 CRUD 操作
- `emails` - 邮件 CRUD 操作
- `folders` - 文件夹 CRUD 操作
- `database` - 数据库连接管理

技术栈:
- SQLite (通过 sea-orm)
- Keyring (敏感信息)
- 内存缓存 (性能优化)

---

### 兼容服务层 (Services)

**🎉 Services 层已完成迁移，目录已删除！**

所有功能已迁移到新架构：

| 原服务模块 | 迁移目标 | 状态 |
|-----------|---------|------|
| `search_service` | `storage::search` | ✅ 已迁移 |
| `conflict_resolver` | `engine::conflict_resolver` | ✅ 已迁移 |
| `operation_manager` | `engine::operation_manager` | ✅ 已迁移 |
| `oauth_service` | `auth::AuthManager` | ✅ 已整合 |
| `sync_manager` | `sync::SyncManager` | ✅ 已迁移 |
| `smtp_service` | `protocols::smtp` | ✅ 已迁移 |
| `imap/*` | `protocols::imap` | ✅ 已迁移 |

**注意**: 部分 command 层仍使用 storage repositories（accounts, emails, folders）进行 CRUD 操作，这是正常的架构分层设计。

---

## 前端模块架构

### 目录结构

```
src/
├── views/                 # 页面视图
│   ├── MailboxView.vue    # 邮箱主视图
│   ├── ComposeView.vue    # 写信视图
│   ├── SettingsView.vue   # 设置视图
│   └── AccountView.vue    # 账号管理视图
│
├── components/            # 可复用组件
│   ├── mail/              # 邮件相关组件
│   │   ├── MailList.vue   # 邮件列表
│   │   ├── MailItem.vue   # 邮件项
│   │   ├── MailDetail.vue # 邮件详情
│   │   └── MailToolbar.vue # 邮件工具栏
│   ├── folder/            # 文件夹组件
│   │   ├── FolderTree.vue # 文件夹树
│   │   └── FolderItem.vue # 文件夹项
│   ├── compose/           # 写信组件
│   │   ├── ComposeEditor.vue # 编辑器
│   │   └── AttachmentList.vue # 附件列表
│   └── ui/                # UI 组件
│       ├── Button.vue
│       ├── Input.vue
│       └── Modal.vue
│
├── stores/                # Pinia 状态管理
│   ├── mail.ts            # 邮件状态
│   ├── folder.ts          # 文件夹状态
│   ├── account.ts         # 账号状态
│   ├── sync.ts            # 同步状态
│   └── ui.ts              # UI 状态
│
├── api/                   # API 调用封装
│   ├── mail.ts            # 邮件 API
│   ├── folder.ts          # 文件夹 API
│   ├── account.ts         # 账号 API
│   ├── sync.ts            # 同步 API
│   └── flow_engine.ts     # FlowEngine API
│
├── composables/           # 组合式函数
│   ├── useMailSync.ts     # 邮件同步
│   ├── useMailFetch.ts    # 邮件获取
│   ├── useCompose.ts      # 写信
│   └── useNotification.ts # 通知
│
├── utils/                 # 工具函数
│   ├── date.ts            # 日期格式化
│   ├── email.ts           # 邮件解析
│   └── validation.ts      # 验证
│
├── types/                 # TypeScript 类型
│   ├── mail.ts
│   ├── folder.ts
│   ├── account.ts
│   └── sync.ts
│
└── App.vue                # 应用入口
```

### 核心状态管理

**Mail Store** (stores/mail.ts)

```typescript
export const useMailStore = defineStore('mail', () => {
  // 状态
  const mails = ref<Mail[]>([])
  const currentMail = ref<Mail | null>(null)
  const selectedMails = ref<Set<number>>(new Set())
  const loading = ref(false)

  // 操作
  async function fetchMails(folder: string, page: number)
  async function fetchMailDetail(id: number)
  async function updateMailFlags(id: number, flags: MailFlags)
  async function deleteMails(ids: number[])
  async function moveMails(ids: number[], folder: string)

  return {
    mails, currentMail, selectedMails, loading,
    fetchMails, fetchMailDetail, updateMailFlags,
    deleteMails, moveMails
  }
})
```

**Sync Store** (stores/sync.ts)

```typescript
export const useSyncStore = defineStore('sync', () => {
  // 状态
  const syncState = ref<Record<number, SyncStatus>>({})
  const syncProgress = ref<SyncProgress | null>(null)

  // 操作
  async function startSync(accountId: number)
  async function stopSync(accountId: number)
  function onSyncProgress(event: SyncProgressEvent)

  return {
    syncState, syncProgress,
    startSync, stopSync, onSyncProgress
  }
})
```

---

## 数据流设计

### 邮件同步流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                         邮件同步数据流                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  前端触发                                                            │
│    │                                                                │
│    ▼                                                                │
│  command/sync.rs - start_sync()                                    │
│    │                                                                │
│    ▼                                                                │
│  sync/SyncManager - sync_account()                                  │
│    │                                                                │
│    ├─→ auth/AuthManager - get_imap_auth()                          │
│    │       │                                                        │
│    │       └─→ providers/ProviderPool - detect_provider()          │
│    │       └─→ auth/TokenManager - get_access_token()              │
│    │                                                                │
│    ├─→ protocols/imap - AsyncImapClient::connect()                 │
│    │                                                                │
│    ├─→ sync/FolderManager - sync_folders()                         │
│    │                                                                │
│    ├─→ sync/DeltaSync - sync_incremental()                         │
│    │       │                                                        │
│    │       ├─→ sync/ChangeDetector - detect_changes()              │
│    │       │                                                        │
│    │       └─→ sync/MailProcessor - process_mails()               │
│    │               │                                                │
│    │               └─→ storage/emails - 保存到数据库               │
│    │                                                                │
│    └─→ sync/SyncStateManager - update_sync_completed()            │
│        │                                                            │
│        ▼                                                            │
│    发送 Tauri 事件: sync://progress                                │
│        │                                                            │
│        ▼                                                            │
│  前端 stores/sync - onSyncProgress()                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 邮件发送流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                         邮件发送数据流                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  前端 ComposeView                                                   │
│    │                                                                │
│    ▼                                                                │
│  api/mail.ts - sendMail()                                          │
│    │                                                                │
│    ▼                                                                │
│  command/email.rs - send_email()                                   │
│    │                                                                │
│    ├─→ auth/AuthManager - get_smtp_auth()                          │
│    │       │                                                        │
│    │       └─→ auth/TokenManager - get_access_token()              │
│    │                                                                │
│    ├─→ protocols/smtp - SmtpSender::send()                         │
│    │                                                                │
│    ├─→ storage/emails - 保存到已发送                               │
│    │                                                                │
│    └─→ 返回发送结果                                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 部署架构

### Tauri 应用结构

```
Postium Mail.app/
├── Contents/
│   ├── MacOS/
│   │   └── postium-mail          # Rust 后端可执行文件
│   ├── Resources/
│   │   └── *.icns                # 应用图标
│   └── app.rs                    # 前端资源（内嵌）
└── postium-mail.config.*         # Tauri 配置
```

### 数据存储位置

| 平台 | 数据库 | Keyring |
|------|--------|---------|
| macOS | `~/Library/Application Support/postium-mail/mail.db` | Keychain |
| Linux | `~/.config/postium-mail/mail.db` | libsecret |
| Windows | `%APPDATA%\postium-mail\mail.db` | Windows Credential Manager |

---

## 技术栈总结

### 后端 (Rust)

| 组件 | 技术选型 |
|------|---------|
| 框架 | Tauri 2.x |
| 异步运行时 | Tokio |
| 数据库 ORM | sea-orm v2.0.0-rc.37 |
| 数据库 | SQLite |
| 敏感信息 | keyring |
| IMAP 协议 | async-imap 0.11.0 |
| SMTP 协议 | lettre 0.11 |
| HTTP 客户端 | reqwest |
| 序列化 | serde |

### 前端 (Vue 3)

| 组件 | 技术选型 |
|------|---------|
| 框架 | Vue 3.5 |
| 语言 | TypeScript |
| 构建 | Vite 6 |
| UI 库 | shadcn/ui + TailwindCSS |
| 状态管理 | Pinia |
| 路由 | Vue Router |

---

## 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 1.1.0 | 2026-03-19 | 更新架构结构，记录 services 层完全迁移 |
| 1.0.0 | 2026-03-19 | 初始版本，整合 FlowEngine 架构设计 |
