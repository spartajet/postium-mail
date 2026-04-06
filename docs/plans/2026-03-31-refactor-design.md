# Postium Mail 架构重构设计文档

> 日期：2026-03-31
> 状态：已确认
> 前序文档：2026-03-30-architecture-refactor-design.md（草案）

## 1. 背景与动机

旧项目 (D:\Rust\postium-mail) 存在以下核心问题：

1. **缺少 Service 层** — 业务逻辑散落在 Command handler 和 Repository 中
2. **SyncManager 是 God Object** — 同时负责连接管理、同步逻辑、邮件处理、进度通知
3. **MailProvider Trait 职责混乱** — 配置、认证、检测、工具函数全塞在一个 trait 里
4. **前端架构混乱** — Vue 3 Store 过于臃肿（account.ts 612 行）
5. **错误处理粗暴** — 所有错误转为字符串，前端无法区分错误类型
6. **类型安全缺失** — 前端手动调用 invoke()，无编译时类型检查

## 2. 重构策略

- **代码策略**：参考旧代码重写，不直接复制
- **数据库**：沿用旧 schema，小幅修正不合理处
- **IDLE 推送**：不实现，使用定时/手动同步
- **前端功能**：完整实现所有 UI（含日历/工作流/AI），后端部分功能留桩

## 3. 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| 桌面框架 | Tauri | 2.x |
| 后端语言 | Rust | 2021 edition |
| ORM | SeaORM | 2.0.0-rc.x |
| 类型绑定 | tauri-specta + specta-typescript | 2.0.0-rc.21 |
| 错误处理 | tauri-specta Result 模式 | - |
| 前端框架 | SvelteKit (SPA mode) | 2.x |
| UI 核心 | Svelte | 5.x (runes) |
| 组件库 | shadcn-svelte | next |
| 样式 | Tailwind CSS | v4 |
| 富文本 | TipTap | latest |
| 类型系统 | TypeScript | 5.x |
| 构建 | Vite | 6.x |
| 包管理 | Bun | - |

## 4. 后端架构

### 4.1 四层架构

```
Command 层 (薄 IPC 包装) → Service 层 (业务逻辑) → Domain 层 (纯领域) → Infrastructure 层 (数据库/协议/系统)
```

依赖规则：只能向下一层依赖，禁止反向。Domain 层不依赖 Tauri。

### 4.2 目录结构

```
src-tauri/src/
├── main.rs
├── lib.rs                    # Builder 入口，specta 初始化
├── error/
│   ├── mod.rs
│   └── types.rs              # MailError 统一错误类型
├── command/                  # Command 层
│   ├── mod.rs
│   ├── account.rs
│   ├── email.rs
│   ├── sync.rs
│   └── auth.rs
├── service/                  # Service 层
│   ├── mod.rs
│   ├── account_service.rs
│   ├── email_service.rs
│   └── sync_service.rs
├── domain/                   # Domain 层
│   ├── mod.rs
│   ├── auth/                 # OAuth2 + 密码认证
│   ├── providers/            # 服务商
│   │   ├── traits.rs         # MailProvider + OAuthProvider + ProviderDetector
│   │   ├── detect.rs
│   │   ├── personal/
│   │   └── enterprise/
│   └── sync/
│       ├── orchestrator.rs
│       ├── progress.rs       # SyncProgressEmitter
│       ├── strategy/
│       └── folder/
├── infrastructure/
│   ├── storage/              # SeaORM + Repository
│   │   ├── database.rs
│   │   ├── models/
│   │   ├── service/
│   │   ├── cache.rs
│   │   └── search.rs
│   ├── protocols/            # IMAP + SMTP
│   └── sys/                  # 日志、托盘
└── migration/
```

### 4.3 Command 层

每个 Command 只做一件事：调用 Service 对应方法。无分支、无业务判断。

```rust
#[tauri::command]
#[specta::specta]
pub async fn create_account(
    service: tauri::State<'_, AccountService>,
    request: CreateAccountRequest,
) -> Result<AccountDto, MailError> {
    service.create_account(request).await
}
```

### 4.4 Service 层

- **AccountService**：创建（检测服务商→配置→存DB→Keyring→首次同步）、删除、编辑、列表
- **EmailService**：列表查询+分页+缓存、标记/删除/移动、FTS5搜索、SMTP发送
- **SyncService**：触发同步、进度事件、协调 SyncOrchestrator

### 4.5 Provider Trait 瘦身

```rust
// 核心配置
pub trait MailProvider: Send + Sync {
    fn info(&self) -> &ProviderInfo;
    fn imap_config(&self, email: &str) -> ImapServerConfig;
    fn smtp_config(&self, email: &str) -> SmtpServerConfig;
    fn capabilities(&self) -> ProviderCapabilities;
    fn folder_mapping(&self) -> StandardFolder;
}

// OAuth（独立 trait）
pub trait OAuthProvider: MailProvider {
    fn oauth_config(&self) -> OAuthConfig;
    fn generate_xoauth2(&self, email: &str, access_token: &str) -> String;
    fn redirect_uri(&self, port: u16) -> String;
}

// 检测（独立 trait）
pub trait ProviderDetector: Send + Sync {
    fn domains(&self) -> &[&'static str];
    fn detect(&self, email: &str) -> bool;
}
```

### 4.6 SyncManager 拆分

- `SyncOrchestrator`：只负责同步流程编排
- `SyncProgressEmitter`：独立的进度通知，通过 specta Event 发射

### 4.7 统一错误处理

```rust
#[derive(Debug, thiserror::Error, specta::Type, serde::Serialize)]
#[serde(tag = "type", content = "message")]
pub enum MailError {
    AccountNotFound(i32),
    AuthFailed(String),
    ImapConnectionFailed(String),
    SmtpSendFailed(String),
    SyncFailed(String),
    DatabaseError(String),
    KeyringError(String),
    ProviderNotSupported(String),
    InvalidParam(String),
}
```

前端通过 tauri-specta Result 模式拿到类型化错误。

### 4.8 Tauri Specta 集成

```rust
let mut builder = Builder::<tauri::Wry>::new()
    .commands(collect_commands![/* all commands */])
    .events(collect_events![SyncProgressEvent]);

#[cfg(debug_assertions)]
builder.export(Typescript::default(), "../src/lib/bindings.ts")
    .expect("Failed to export typescript bindings");
```

## 5. 数据库调整

### 5.1 emails 表

- `body_text` / `body_html` 改为可 NULL（同步后填充）
- 新增 `preview TEXT`（截取前 200 字符，列表展示用）

### 5.2 新增 folders 表

```sql
CREATE TABLE folders (
    id INTEGER PRIMARY KEY,
    account_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    delimiter TEXT,
    parent_id INTEGER,
    standard_folder TEXT,
    unread_count INTEGER DEFAULT 0,
    total_count INTEGER DEFAULT 0,
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);
```

### 5.3 accounts 表

新增 `display_name TEXT`（发件人显示名称）

### 5.4 FTS5

用 `preview` 替代 `body_text` 建索引，性能更好

## 6. 前端架构

### 6.1 路由（轻量路由方案）

```
src/routes/
├── +layout.svelte        # 全局布局（TitleBar）
├── +layout.ts            # ssr = false
├── +page.svelte          # 主页（三栏邮件）
├── calendar/
│   └── +page.svelte      # 日历视图
├── workflow/
│   └── +page.svelte      # 工作流编辑器
└── settings/
    └── +page.svelte      # 设置页面
```

### 6.2 三栏布局

```
┌─────────────────────────────────────────────────────────────┐
│  TitleBar (40px) — 拖拽区域 + 标题 + WindowControls          │
├────────────┬──────────────────┬─────────────────────────────┤
│  Sidebar   │   EmailList      │   EmailDetail               │
│  (240px)   │   (flex)         │   (flex)                    │
├────────────┴──────────────────┴─────────────────────────────┤
│  StatusBar (32px) — 同步状态 + 错误提示                       │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Sidebar

- Logo + 设置按钮 + 同步按钮（旋转动画）
- 账号选择器（头像圆 + 名称 + 未读数下拉）
- 写邮件按钮
- 文件夹导航（收件箱/星标/已发送/草稿/垃圾邮件/回收站）+ 未读数
- 视图：日历、工作流
- 标签：紧急/工作/个人/财务（彩色圆点）
- 底部：存储用量条

### 6.4 EmailList

- 搜索栏（⌘K 快捷键）
- 工具栏（刷新/筛选/全选）
- 邮件条目：发件人+时间 → 主题 → 预览 → 标签+附件数
- 无限滚动/分页

### 6.5 EmailDetail

- 空状态提示
- 主题 + 发件人头像/名称/邮箱 + 时间/收件人
- AI 摘要卡片（UI 就绪，后端 stub）
- HTML 正文
- 附件列表（文件类型图标 + 名称 + 大小）
- 操作栏：回复/转发/星标/归档/删除
- AI 操作栏：智能回复/摘要/翻译/提取任务（UI 就绪，后端 stub）

### 6.6 视觉风格

Glassmorphism（毛玻璃）风格，Tailwind CSS + shadcn-svelte CSS 变量：
- 背景渐变光球
- 半透明面板 + backdrop-blur
- 深色/浅色主题切换

### 6.7 Store 设计

Svelte 5 runes 模式（$state / $derived），每个 store 导出 getter + action 函数。

### 6.8 国际化

轻量自定义方案，支持 zh-CN / en-US。

## 7. 功能范围

### 7.1 v1 前后端都实现

| 功能 | 说明 |
|------|------|
| 多账号管理 | 添加/删除/编辑，OAuth + 密码 |
| 邮件列表/详情 | 文件夹浏览、分页、HTML渲染 |
| 邮件同步 | 手动/定时，实时进度 |
| 邮件搜索 | FTS5 |
| 邮件操作 | 标星、已读、删除、移动 |
| 发送邮件 | SMTP + TipTap 富文本 |
| 标签系统 | 创建/编辑/着色/邮件打标签 |
| 系统托盘 | 最小化到托盘 |
| 自定义标题栏 | 无边框窗口 |
| 国际化 | zh-CN / en-US |
| 深色/浅色主题 | 跟随系统或手动 |
| 服务商检测 | 自动识别邮箱域名 |

### 7.2 前端实现，后端留桩

| 功能 | 说明 |
|------|------|
| 日历视图 | 月/周视图 + 事件卡片，后端返回空数据 |
| 工作流编辑器 | 可视化流程编辑，后端返回空数据 |
| AI 操作 | 摘要/智能回复/翻译/提取任务 UI，后端返回空数据 |

## 8. 实施阶段

### 阶段 0：项目基建
### 阶段 1：后端基础层（错误类型 + Provider + 协议层）
### 阶段 2：后端 Service + Command 层
### 阶段 3：前端基建 + 布局
### 阶段 4：前端核心功能（邮件收发）
### 阶段 5：前端高级功能（日历/工作流/AI UI）
### 阶段 6：集成测试 + 清理

详见实施计划文档。
