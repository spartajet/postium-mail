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

更多说明见 [docs/testing/e2e.md](docs/testing/e2e.md)。
