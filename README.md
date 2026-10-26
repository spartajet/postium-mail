# Postium Mail

Postium Mail 是一个基于 **Rust + GPUI**（gpui-kit）的桌面邮件客户端，后端与前端同进程、纯原生实现，无 WebView。

> 架构沿革：本项目最初基于 Tauri 2 + SvelteKit，已于 2026-10 完成向 GPUI 的整体迁移（`gpui-migration` 分支）。`src-tauri/`、`src/`（Svelte）与 `e2e/`（WebdriverIO）在验收期间保留为遗留双轨，验收后移除。

## 架构

Cargo workspace（根目录）：

| Crate | 职责 |
|---|---|
| `crates/postium-core` | 后端核心：domain / service / infrastructure + `CoreApi` 进程内门面（44 个 API，无 UI 依赖），SQLite/IMAP/SMTP/OAuth |
| `crates/postium-editor` | 原生邮件编辑器组件（WYSIWYG，基于 gpui-kit-markdown-editor，md ⇄ HTML） |
| `crates/postium-ui` | GPUI 应用（bin `postium-mail`）：主窗口/写信/设置窗口、托盘、i18n、主题 |

关键设计：

- 无 IPC——UI 直接进程内调用 `CoreApi`（异步经共享 tokio runtime 桥接 gpui 的 smol 执行器）；
- 邮件正文用原生 HTML 渲染器；内联图片（cid:）由后端转 data URI 注入，远程图片默认屏蔽（防已读追踪），复杂布局可"浏览器打开"兜底；
- 数据目录 `~/.postium` 与旧 Tauri 版双向兼容。

## 开发环境

- Rust stable（1.98+）
- WebView2 Runtime（Windows 自带；仅附件"浏览器打开"等场景需要，应用本身不依赖）

## 常用命令

```bash
cargo run -p postium-ui          # 运行应用（数据与旧版共用 ~/.postium）
cargo test --workspace           # 全部测试（见下）
cargo test -p postium-ui e2e_tests   # 仅 E2E
```

## 测试

| 层 | 命令 | 覆盖 |
|---|---|---|
| core 集成/单元（240+） | `cargo test -p postium-core` | 账号/邮件/标签/同步/seed/迁移、协议解析、错误转换 |
| editor 组件（6） | `cargo test -p postium-editor` | md⇄HTML 转换往返、嵌套列表、脚本剔除 |
| UI 单元（6） | `cargo test -p postium-ui` | i18n 词条一致性、图片注入策略 |
| **UI E2E（8）** | `cargo test -p postium-ui e2e_tests` | headless 全栈：seed 数据 → 启动/账号切换/导航/搜索/写信/草稿/星标删除/主题 |
| Truth 真实邮箱（手动） | `cargo test -p postium-core --test real_mail_accounts -- --ignored` | 真实 IMAP 登录与读取（需 `.test_mail_accounts.json`） |

E2E 使用 gpui `TestAppContext` headless 运行真实全栈（CoreApi + SQLite + e2e seed 固定数据），不连接真实邮件服务；用户交互以 on_click 同款 handler 与键盘事件驱动。约 0.15 秒跑完全部场景（对比旧 WebdriverIO 分钟级）。

更多说明见 [测试现状总览](docs/testing/overview.md)、[E2E 测试指南](docs/testing/e2e.md)、[Truth 真实邮箱测试](docs/testing/truth.md)。

## 遗留双轨（验收后移除）

以下为 Tauri/Svelte 旧轨，仅迁移期保留：

- `src-tauri/`、`src/`：旧应用，`bun install && bun run dev` 可运行；
- `e2e/`：WebdriverIO E2E（已被 GPUI E2E 取代；`real-accounts.e2e.js` 真实账号 UI 联测待按新模式重建）；
- `docs/plans/`：迁移前的分阶段计划（历史存档）。

## 设计与计划文档

- 迁移设计与结论：`docs/superpowers/specs/2026-10-26-gpui-migration-design.md`
- 实施计划：`docs/superpowers/plans/`（计划 1：core/editor 地基；计划 2：postium-ui 应用）
