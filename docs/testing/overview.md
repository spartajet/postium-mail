# 测试现状总览

本文档记录 Postium Mail 当前自动化测试体系、运行命令和维护重点。E2E 细节见 [E2E 测试指南](./e2e.md)，真实邮箱测试见 [Truth 说明](./truth.md)。

> 2026-10 GPUI 迁移后，测试体系已整体迁移至 Cargo workspace；旧 Vitest / WebdriverIO 套件随 Tauri 轨道保留为遗留（验收后移除）。

## 测试分层

| 层 | 位置 | 运行 | 覆盖 |
|---|---|---|---|
| core 集成测试 | `crates/postium-core/tests/` | `cargo test -p postium-core` | 账号/邮件/标签/同步命令级行为、seed 确定性、schema 迁移、FTS |
| core 单元测试 | core 源码内 `#[cfg(test)]` | 同上 | provider 检测、文件夹匹配、错误转换、IMAP parser 等 |
| editor 组件测试 | `crates/postium-editor/tests/` | `cargo test -p postium-editor` | md⇄HTML 往返、嵌套列表/引用/图片、script/style 剔除 |
| UI 单元测试 | `crates/postium-ui/src` 内 | `cargo test -p postium-ui` | i18n 中英词条一致、图片注入（cid 替换/远程屏蔽） |
| UI E2E | `crates/postium-ui/src/e2e_tests.rs` | `cargo test -p postium-ui e2e_tests` | 真实全栈 headless 用户流程（见 e2e.md） |
| Truth 真实邮箱 | `crates/postium-core/tests/real_mail_accounts.rs` | `--ignored` 手动 | 真实 IMAP 登录/读取 |

一条命令跑全部：

```bash
cargo test --workspace
```

## core 集成测试

位于 `crates/postium-core/tests/`（自 `src-tauri/tests/` 迁移，含 `common/` 公共构造）：

- `account_commands.rs` / `email_commands.rs` / `label_commands.rs`：命令级 CRUD 与异常路径；
- `all_accounts_sync.rs` / `sync_history.rs`：同步编排与历史；
- `e2e_seed.rs`：seed 数据确定性、幂等性、账号隔离与搜索支持；
- `schema.rs`：完整迁移后 FTS trigger 行为；
- `api_tests.rs`：`CoreApi` 装配冒烟（init → 账号/provider/统计可用）。

集成测试使用内存 SQLite 与内存凭据存储，CI 不依赖 Linux Secret Service / DBus / keyring。

## E2E 测试（GPUI headless）

位于 `crates/postium-ui/src/e2e_tests.rs`，基于 gpui `TestAppContext`：

- **同进程真实全栈**：`CoreApi::init(tempdir, e2e=true)` 触发与旧版完全相同的 seed（两账号 + 固定邮件），全部视图/状态/服务为生产实现；
- **交互驱动**：以按钮 on_click 同款 handler 直调（gpui 无元素定位 API）+ `simulate_keystrokes` 键盘输入（写信字段真实键入）；
- **异步 settle**：`run_until_parked` + 轮询等待。

覆盖场景与旧 WebdriverIO 套件一一对应（启动 seed 显示、账号切换、Sent/Starred/Inbox 导航、Quarterly Planning 搜索、星标+删除、写信字段/草稿/重开清空、主题切换+持久化）。详见 [e2e.md](./e2e.md)。

### E2E 模式的 core 行为

- 远程邮件操作注入 `LocalOnlyMailRemoteOperator`；
- 草稿远端注入 `LocalOnlyDraftRemoteWriter`（确定性假 UID）；
- 凭证缺失时回退占位凭证（`EmailService::e2e_local` 标志）。

> 后两点是 2026-10 E2E 迁移时发现并修复的缺陷：Tauri 版 E2E 模式下保存草稿会因真连 IMAP/读 keyring 而必然失败（旧 E2E 无保存草稿用例故未暴露）。

## CI 流程

`.github/workflows/test.yml` 当前仍面向旧轨（Rust/`src-tauri`、前端 Vitest、WebdriverIO xvfb）。**待办**：迁移为 `cargo test --workspace` 单 job（新 E2E 无需 GUI/WebDriver，Windows/Linux 均可直接跑）。

## Mock 与 Seed 策略

维持不变：E2E 使用确定性 seed（`POSTIUM_E2E` 语义，GPUI 版由 `CoreApi::init` 的 `e2e` 参数承接），不连接真实邮件服务；需要真实服务时走 Truth 手动链路，不混入默认 CI。

## 维护重点

- 用户可见主流程优先补 `e2e_tests.rs` 场景（headless，秒级）；
- 服务层与数据库行为优先补 core 集成测试；
- 视图状态与纯逻辑（i18n、图片注入、转换器）补 UI/editor 单元测试；
- E2E 断言基于 seed 固定主题（如 `Primary Inbox Message NN`、`Quarterly Planning Alpha`），保持确定、幂等、不依赖当前日期。
