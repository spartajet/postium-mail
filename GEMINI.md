# Postium Mail - AI 驱动的桌面邮件客户端

Postium Mail 是一款基于 **Tauri 2**、**Vue 3** 和 **Rust** 构建的现代化、功能丰富的桌面邮件客户端。它结合了传统邮件功能、AI 驱动能力、日历管理以及自动化工作流。

## 🏗️ 架构与技术栈

### 前端
- **框架：** Vue 3 (使用 `<script setup>` 的 Composition API)
- **语言：** TypeScript
- **样式：** SCSS, Naive UI (组件库)
- **状态管理：** Pinia
- **构建工具：** Vite
- **编辑器：** Tiptap (基于 ProseMirror 的富文本编辑器)
- **国际化：** vue-i18n

### 后端 (Rust)
- **运行时：** Tauri 2
- **数据库：** SQLite
  - **ORM:** SeaORM 2.0 (CRUD 操作)
  - **全文检索 (FTS):** SQLite FTS5 配合 `sqlite-jieba-tokenizer` (中文分词搜索)
- **邮件协议：**
  - **IMAP:** `async-imap`
  - **SMTP:** `lettre`
- **身份验证：** OAuth2 (支持 Microsoft/Google)
- **安全性：** Keyring (操作系统原生凭据存储)
- **AI 集成：** LLM 支持 (OpenAI/Ollama)，RAG 增强 (Qdrant 计划中)

## 📁 项目结构

```text
postium-mail/
├── src/                # 前端源码 (Vue 3 + TS)
│   ├── components/     # UI 组件 (布局、邮件、日历、工作流、AI 聊天)
│   ├── stores/         # Pinia 状态管理 (邮件、账号、UI、同步等)
│   ├── locales/        # i18n 多语言翻译
│   └── assets/         # 样式 (SCSS) 和资源文件
├── src-tauri/          # 后端源码 (Rust)
│   ├── src/            # 核心逻辑、命令和服务
│   │   ├── auth/       # OAuth 和身份验证
│   │   ├── command/    # Tauri invoke 命令处理器
│   │   ├── engine/     # 工作流引擎
│   │   ├── models/     # 数据库实体 (SeaORM)
│   │   ├── services/   # 业务逻辑 (OAuth, IMAP, SMTP)
│   │   └── sync/       # 邮件同步管理器
│   ├── migration/      # 数据库迁移文件 (SeaORM)
│   └── tests/          # 集成测试与单元测试
├── docs/               # 设计文档与架构分析
└── scripts/            # 用于测试和 CI 的实用脚本
```

## 🚀 关键命令

### 开发环境
- **启动前端开发服务器：** `npm run dev`
- **启动 Tauri 开发模式 (热重载)：** `npm run tauri dev`
- **后端 Rust 编译检查：** 在 `src-tauri` 目录下运行 `cargo check` 或 `cargo build`

### 构建打包
- **构建前端：** `npm run build`
- **打包桌面应用：** `npm run tauri build`

### 测试
- **运行 Rust 测试：** 在 `src-tauri` 目录下运行 `cargo test`
- **运行集成测试：**
  - Windows: `.\scripts\run-integration-tests.bat`
  - Linux/macOS: `./scripts/run-integration-tests.sh`

## 🛠️ 开发规范

### 前端
- **样式：** 使用 `src/assets/styles/_variables.scss` 中定义的 CSS 变量以保持主题一致性。
- **组件：** 倾向于编写小型、可复用的组件。复杂 UI 元素优先使用 Naive UI。
- **状态管理：** UI 相关状态存放在 `ui.ts`，领域数据存放在特定 Store (如 `email.ts`)。

### 后端 (Rust)
- **错误处理：** 应用程序级错误使用 `anyhow`，库级错误使用 `thiserror`。
- **命令注册：** 在 `src-tauri/src/lib.rs` 中注册新的 Tauri 命令，并在 `src-tauri/src/command/` 中实现。
- **数据库：** 所有模式变更必须通过 `src-tauri/migration/` 中的 SeaORM 迁移任务完成。
- **异步编程：** 充分利用 `tokio` 处理异步操作，特别是网络和数据库 IO。

### 同步逻辑
- `SyncManager` 负责处理首次同步 (前 1000 封邮件) 和增量同步 (基于 UID)。
- 同步进度通过 Tauri 事件 (`sync-progress-{account_id}`) 推送，并在前端由 `useSyncStore` 捕获。

## 📖 相关文档
- [设计文档](docs/design.md) - 深入了解技术架构和数据库模式。
- [功能需求](docs/requirements.md) - 详细的功能点列表。
- [同步系统设计](docs/sync.md) - 同步系统的具体设计方案。
- [架构迁移分析](docs/architecture-migration-analysis.md) - 系统演进过程中的技术分析。
