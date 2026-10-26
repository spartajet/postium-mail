# Postium Mail

Postium Mail 是一个基于 Tauri 2、SvelteKit、TypeScript 和 Rust 的桌面邮件客户端。

## 开发环境

需要安装：

- Bun 1.2.x
- Rust stable
- Tauri 2 需要的系统依赖
- `tauri-driver`，用于运行桌面端 E2E 测试

安装依赖：

```bash
bun install
cd e2e && bun install
```

安装 `tauri-driver`：

```bash
cargo install tauri-driver --locked
```

## 常用命令

```bash
bun run dev
bun run build
bun run check
bun run test:rust
bun run test:frontend
bun run test:e2e
```

`bun run test:e2e` 会构建 Tauri debug 应用，启动 `tauri-driver`，并使用独立的 E2E 数据目录运行 WebdriverIO 测试。

## 测试

当前测试分三层：

- Rust 测试：`bun run test:rust`
- 前端单元测试：`bun run test:frontend`
- 桌面端 E2E 测试：`bun run test:e2e`

E2E 测试默认不连接真实 IMAP、SMTP、OAuth 或远程邮件服务。测试运行时会设置 `POSTIUM_E2E=1`，并在隔离目录中 seed 两个账号和固定邮件数据，从而覆盖账号切换、邮件列表滚动、搜索、导航、写邮件弹窗和主题切换等流程。

更多说明见 [测试现状总览](docs/testing/overview.md) 和 [E2E 测试指南](docs/testing/e2e.md)。

## GPUI 迁移（进行中）

本仓库正迁移到 GPUI 原生 UI。Cargo workspace 位于根目录：

- `crates/postium-core`：后端（domain/service/infrastructure + `CoreApi` 进程内门面，无 UI 依赖）
- `crates/postium-editor`：原生邮件编辑器组件（WYSIWYG，md⇄HTML）

常用命令：

- `cargo test --workspace`——全部测试（core 240+ / editor 6 / **UI 单元 6 + E2E 8**）
- `cargo test -p postium-ui e2e_tests`——GPUI 版 E2E（headless 全栈：seed 数据 → 导航/搜索/账号切换/写信/草稿/星标删除/主题，迁移自 WebdriverIO 套件，交互以 handler + 键盘事件驱动）

`src-tauri/` 与 `src/`（Svelte）在迁移期间保留双轨，验收后移除。
设计文档：`docs/superpowers/specs/2026-10-26-gpui-migration-design.md`

GPUI 版运行：`cargo run -p postium-ui`（数据与 Tauri 版共用 `~/.postium`）
