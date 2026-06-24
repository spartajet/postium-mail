# 测试现状总览

本文档记录 Postium Mail 当前自动化测试体系、运行命令、CI 流程和维护重点。E2E 细节见 [E2E 测试指南](./e2e.md)。

## 测试分层

当前测试分为三层：

- Rust 测试：覆盖领域逻辑、服务层、数据库迁移和 Tauri 后端命令。
- 前端单元测试：覆盖 Svelte store、核心组件和前端状态行为。
- 桌面端 E2E 测试：覆盖 Tauri 应用真实启动后的关键用户流程。

## 本地命令

常用命令：

```bash
bun run test:rust
bun run test:frontend
bun run check
bun run test:e2e
```

完整验证建议顺序：

```bash
bun run check
bun run test:frontend
bun run test:rust
bun run test:e2e
```

`bun run test:e2e` 会构建 Tauri debug 应用、启动 `tauri-driver`，并使用隔离的 E2E 数据目录运行 WebdriverIO。

## Rust 测试

Rust 测试位于 `src-tauri/tests` 和源码模块内：

- `account_commands.rs`：账号创建、查询、更新、删除。
- `email_commands.rs`：邮件列表、详情、搜索、状态更新和异常路径。
- `label_commands.rs`：标签 CRUD 和异常路径。
- `e2e_seed.rs`：E2E seed 数据的确定性、幂等性、账号隔离和搜索支持。
- `migrations.rs`：完整迁移后的 FTS trigger 行为。
- 源码内单元测试：provider 检测、标准文件夹匹配、错误转换、IMAP parser 等。

测试公共构造位于 `src-tauri/tests/common/mod.rs`。集成测试使用内存 SQLite，并使用内存凭据存储，避免 CI 依赖 Linux Secret Service、DBus 或本机 keyring。

## 前端测试

前端测试位于 `src/lib/__tests__`：

- `stores/account.test.ts`：账号状态加载、创建、删除和 provider 检测。
- `stores/email.test.ts`：邮件状态加载、选择、星标、删除等行为。
- `components/Toast.test.ts`：Toast 组件展示和交互。

前端测试使用 Vitest、Testing Library 和 jsdom。运行前会执行 `svelte-kit sync`，确保 SvelteKit 生成类型和运行环境一致。

## E2E 测试

E2E 测试位于 `e2e/test`，使用 WebdriverIO、`tauri-driver` 和 Linux WebKit WebDriver。

当前覆盖：

- 应用启动并显示主账号收件箱。
- 主账号和次账号切换。
- 收件箱搜索固定数据。
- 至少 30 封 seed 邮件带来的列表滚动场景。
- Inbox、Sent、Starred 导航。
- 写邮件弹窗字段填写和重开清空。
- 深色、浅色、系统主题按钮。

E2E 默认不连接真实 IMAP、SMTP、OAuth 或外部邮件服务。运行时设置 `POSTIUM_E2E=1`，并在隔离目录中写入两个账号和固定邮件数据。

## CI 流程

GitHub Actions 工作流位于 `.github/workflows/test.yml`，包含三个 job：

- `Rust Tests`：安装 Rust 和 Linux/Tauri 系统依赖，运行 `cargo test`。
- `Frontend Tests`：安装 Bun 依赖，运行 `bun run test`。
- `E2E Tests (Linux)`：依赖 Rust 和 Frontend 成功后运行，安装 WebKit driver、`tauri-driver`、root/e2e 依赖，然后执行 `xvfb-run bun run test`。

E2E job 无论成功失败都会上传 `e2e-artifacts`。失败时还会尝试上传 `.e2e-data`，用于排查 seed 数据和数据库状态。

## Mock 和 Seed 数据策略

当前 E2E 使用确定性 seed 数据，而不是连接真实邮件服务器。

这样做的原因：

- 避免真实账号、网络、OAuth、IMAP/SMTP 限流导致 E2E 不稳定。
- 可以稳定覆盖账号切换、搜索、滚动、导航和写信 UI。
- CI 不需要保存真实邮件凭据。

需要真实服务覆盖时，应单独设计手动验收或隔离的集成测试，不应混入默认 CI E2E。

## Truth 真实账号测试

Truth 测试用于手动验证真实 IMAP 和真实 App 交互链路。它读取 `.test_mail_accounts.json` 并连接真实邮箱服务。SMTP 发送暂未纳入 truth 断言。

运行命令：

```bash
bun run test:rust:truth
bun run test:e2e:truth
```

Truth 测试不进入默认 CI，也不包含在默认 `test:rust`、`test:e2e`、`test:ci` 中。详细说明见 [Truth 真实邮箱测试说明](./truth.md)。

## 维护重点

新增功能时优先遵循以下规则：

- 用户可见主流程优先补 E2E。
- 服务层逻辑和数据库行为优先补 Rust 集成测试。
- 前端 store、组件状态和边界交互优先补 Vitest。
- E2E 选择器使用 `data-testid`，不要依赖 Tailwind class 或 DOM 顺序。
- E2E 数据必须确定、幂等、可重复，不依赖当前日期或真实外部服务。
