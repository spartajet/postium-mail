# Postium Mail 自动化测试设计方案

**日期：** 2026-04-20
**状态：** 已批准
**方案：** Tauri 原生测试套件（方案 A）

---

## 1. 背景与目标

Postium Mail 是基于 Tauri 2 + SvelteKit 5 + Rust 的桌面邮件客户端，支持 19+ 邮件服务商。当前项目没有任何自动化测试。

**目标：** 建立全栈自动化测试体系，覆盖 Rust 后端、Svelte 前端和 E2E 用户流程，支持本地开发和 CI 自动化。

**测试策略：** Mock + 真实服务器混合。核心逻辑用 mock 保证速度和稳定性，关键流程用真实服务器验证。

---

## 2. 测试架构总览

采用测试金字塔分层：

```
                    ┌─────────┐
                    │  E2E    │  tauri-driver + WebdriverIO
                    │  Tests  │  (少量，关键用户流程)
                   ─┴─────────┴─
                  ┌─────────────┐
                  │ Integration │  Tauri mock_builder
                  │   Tests     │  (中等数量，命令+服务层)
                 ─┴─────────────┴─
                ┌─────────────────┐
                │   Unit Tests    │  Rust #[test] + Vitest
                │                 │  (大量，核心逻辑)
                └─────────────────┘
```

| 层级 | 工具 | 覆盖范围 | 数量 |
|------|------|----------|------|
| Rust 单元测试 | `#[test]` / `#[tokio::test]` + `mockall` | 领域逻辑、服务层、仓库层、协议解析 | 大量 |
| Rust 集成测试 | `tauri::test` + 内存 SQLite | Tauri 命令端到端调用链 | 中等 |
| 前端组件测试 | Vitest + `@testing-library/svelte` | Svelte 组件渲染/交互、状态管理 | 中等 |
| E2E 测试 | `tauri-driver` + WebdriverIO | 完整用户流程 | 少量 |

---

## 3. Rust 后端测试

### 3.1 单元测试

在每个源文件的 `#[cfg(test)] mod tests` 中编写。使用内存 SQLite（`:memory:`）进行数据库测试，无需 mock 数据库层。

**需要 mock 的外部依赖：**
- IMAP 连接（`async-imap`）→ `mockall` 创建 `MockImapClient` trait
- SMTP 发送（`lettre`）→ `mockall` 创建 `MockSmtpTransport` trait
- 系统 Keyring → `mockall` 创建 `MockKeyring` trait
- OAuth2 HTTP 请求 → `mockall` + `reqwest` mock

**不需要 mock 的：**
- SQLite 数据库 → 使用内存 SQLite
- 邮件解析（`mail-parser`）→ 纯函数
- Provider 检测逻辑 → 纯字符串匹配
- 错误类型转换 → 直接测试

**优先覆盖的模块：**

| 优先级 | 模块 | 测试内容 |
|--------|------|----------|
| P0 | `infrastructure/protocols/imap/parser.rs` | 邮件头解析、附件解析、编码处理 |
| P0 | `domain/providers/detect.rs` | 邮箱地址到服务商的自动检测 |
| P0 | `service/email_service.rs` | 邮件列表、搜索、发送的业务逻辑 |
| P1 | `service/account_service.rs` | 账号 CRUD |
| P1 | `service/label_service.rs` | 标签管理 |
| P1 | `infrastructure/storage/repository/*` | 数据库 CRUD（用内存 SQLite） |
| P2 | `domain/sync/*` | 同步调度逻辑 |
| P2 | `domain/auth/*` | 认证管理 |

### 3.2 集成测试

在 `src-tauri/tests/` 目录下，使用 Tauri 2 的测试工具构建 mock 应用上下文：

```
src-tauri/tests/
├── common/
│   └── mod.rs              # 测试辅助：创建 mock app、内存数据库
├── account_commands.rs     # 账号相关命令集成测试
├── email_commands.rs       # 邮件相关命令集成测试
├── sync_commands.rs        # 同步相关命令集成测试
└── label_commands.rs       # 标签相关命令集成测试
```

集成测试模式：
- 使用内存 SQLite 构建真实的服务层
- 通过 Tauri mock context 调用命令
- 验证完整的命令 → 服务 → 数据库链路

### 3.3 真实服务器测试

需要连接真实 IMAP/SMTP 的测试用 `#[ignore]` 标记：

```rust
#[tokio::test]
#[ignore] // cargo test -- --ignored 运行
async fn test_real_imap_fetch() { ... }
```

真实服务器测试凭证通过环境变量或 `.test_mail_accounts.json` 配置文件提供。

---

## 4. 前端组件测试

### 4.1 框架选型

- **Vitest** — 与 Vite 原生集成，启动快
- **@testing-library/svelte** — Svelte 社区推荐的组件测试库
- **jsdom** — 模拟浏览器环境

### 4.2 目录结构

```
src/lib/__tests__/
├── mocks/
│   ├── tauri.ts            # mock @tauri-apps/api 的 invoke
│   └── stores.ts           # mock Svelte stores
├── components/
│   ├── EmailList.test.ts
│   ├── EmailDetail.test.ts
│   ├── ComposeModal.test.ts
│   ├── Sidebar.test.ts
│   └── AddAccountModal.test.ts
└── stores/
    ├── account.test.ts
    └── email.test.ts
```

### 4.3 Mock 策略

- `@tauri-apps/api` 的 `invoke` → 返回预定义数据
- `window.__TAURI__` → Tauri 运行时 mock
- 在 `vitest.setup.ts` 中统一注册 mock

### 4.4 优先覆盖

| 优先级 | 组件/Store | 测试内容 |
|--------|-----------|----------|
| P0 | `email.svelte.ts` | 邮件列表状态管理、分页、筛选 |
| P0 | `account.svelte.ts` | 账号切换、添加、删除 |
| P1 | `EmailList.svelte` | 列表渲染、点击选中、分页加载 |
| P1 | `ComposeModal.svelte` | 表单验证、发送逻辑 |
| P2 | `Sidebar.svelte` | 文件夹导航、账号折叠 |
| P2 | `theme.svelte.ts` | 主题切换 |
| P2 | `i18n.svelte.ts` | 国际化切换 |

---

## 5. E2E 测试

### 5.1 技术栈

- **tauri-driver** — Tauri 官方 WebDriver 服务器
- **WebdriverIO v9** — WebDriver 协议客户端
- 独立的 `e2e/` 目录和 `package.json`

### 5.2 目录结构

```
e2e/
├── package.json
├── tsconfig.json
├── wdio.conf.ts              # WebdriverIO 配置
├── capabilities/
│   └── tauri.conf.ts         # tauri-driver 能力配置
├── pageobjects/
│   ├── SidebarPage.ts        # 侧边栏页面对象
│   ├── EmailListPage.ts      # 邮件列表页面对象
│   ├── ComposePage.ts        # 写邮件页面对象
│   └── SettingsPage.ts       # 设置页面对象
└── specs/
    ├── account.e2e.ts        # 账号管理流程
    ├── email.e2e.ts          # 邮件收发流程
    └── navigation.e2e.ts     # 导航和 UI 交互
```

### 5.3 关键 E2E 场景

| 场景 | 描述 | 优先级 |
|------|------|--------|
| 添加邮箱账号 | 设置页添加账号 → 侧边栏出现新账号 | P0 |
| 浏览邮件列表 | 切换文件夹 → 邮件列表更新 | P0 |
| 查看邮件详情 | 点击邮件 → 显示完整内容 | P1 |
| 撰写并发送 | 打开编辑器 → 填写 → 发送 | P1 |
| 主题切换 | 切换暗色/亮色 → UI 更新 | P2 |

---

## 6. CI 集成

### GitHub Actions 配置

创建 `.github/workflows/test.yml`：

**Job 1 — Rust 测试：**
- `cargo test` — 运行所有单元 + 集成测试（mock 环境）
- `cargo test -- --ignored` — 运行真实服务器测试（可选，需要 secrets 配置凭证）
- 需要：Rust 工具链、系统依赖（libwebkit2gtk 等 Linux 依赖）

**Job 2 — 前端测试：**
- `yarn install` + `yarn test` — 运行 Vitest 组件测试
- 需要：Node.js 环境

**Job 3 — E2E 测试：**
- 依赖 Rust 和前端测试通过
- 安装 `tauri-driver`，构建 Tauri 应用，运行 WebdriverIO
- 需要：完整的 Tauri 构建环境

### 本地测试命令

| 命令 | 用途 |
|------|------|
| `cargo test` | Rust 单元 + 集成测试 |
| `cargo test -- --ignored` | 真实服务器测试 |
| `yarn test` | 前端 Vitest 测试 |
| `yarn test:watch` | 前端测试（监听模式） |
| `yarn e2e` | E2E 测试 |
| `yarn test:all` | 运行全部测试 |

---

## 7. 新增依赖

### Rust (Cargo.toml [dev-dependencies])

```toml
[dev-dependencies]
mockall = "0.13"
tokio-test = "0.4"
tempfile = "3"
```

### 前端 (package.json devDependencies)

```json
{
  "vitest": "^3.0",
  "@testing-library/svelte": "^5",
  "@testing-library/jest-dom": "^6",
  "jsdom": "^25"
}
```

### E2E (e2e/package.json devDependencies)

```json
{
  "devDependencies": {
    "@wdio/cli": "^9",
    "@wdio/local-runner": "^9",
    "@wdio/mocha-framework": "^9",
    "tauri-driver": "^0.1"
  }
}
```

---

## 8. 实施计划概览

按优先级分阶段实施：

**Phase 1 — 基础设施搭建：**
1. 添加所有 dev-dependencies
2. 创建测试辅助模块（mock、fixtures、测试数据库）
3. 配置 Vitest 和 WebdriverIO
4. 创建 CI workflow 文件

**Phase 2 — P0 测试用例：**
1. IMAP 解析器单元测试
2. Provider 检测逻辑测试
3. Email Service 核心逻辑测试
4. 邮件和账号状态管理测试
5. E2E：添加账号、浏览邮件列表

**Phase 3 — P1 测试用例：**
1. 账号/标签 Service 测试
2. Repository 层 CRUD 测试
3. Tauri 命令集成测试
4. 前端组件测试（EmailList、ComposeModal）
5. E2E：查看邮件详情、撰写发送

**Phase 4 — P2 测试用例：**
1. 同步调度逻辑测试
2. 认证管理测试
3. 前端辅助组件测试
4. E2E：主题切换等 UI 测试
