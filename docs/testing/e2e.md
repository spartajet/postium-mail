# E2E 测试说明

本文档说明 Postium Mail 桌面端 E2E 测试的运行方式、测试数据和排障入口。

## 运行方式

首次运行前安装依赖：

```bash
bun install
cd e2e && bun install
cargo install tauri-driver --locked
```

运行完整 E2E：

```bash
bun run test:e2e
```

单独运行某个规格：

```bash
cd e2e
bunx wdio run wdio.conf.js --spec ./test/specs/smoke.e2e.js
```

## 数据隔离

E2E 运行由 `e2e/wdio.conf.js` 负责准备环境：

- 设置 `POSTIUM_E2E=1`
- 创建 `.e2e-data/run-<timestamp>-<pid>` 作为本次运行的数据目录
- 设置 `POSTIUM_DATA_DIR` 指向该目录
- 构建 Tauri debug 应用
- 启动 `tauri-driver`
- 失败时保存截图、日志和 JUnit 报告

测试通过时，本次 `.e2e-data/run-*` 会自动清理。测试失败时会保留数据目录，便于检查 SQLite 数据和复现问题。

## Seed 数据

E2E 模式下，应用启动时由 Rust seed 固定测试数据。数据不来自前端 mock，也不连接真实邮件服务。

固定账号：

- `primary.e2e@postium.test`
- `secondary.e2e@postium.test`

固定邮件数量至少 48 封：

- 主账号 36 封，其中 Inbox 至少 24 封
- 次账号 12 封

关键主题：

- `Primary Inbox Message 01`
- `Primary Inbox Message 24`
- `Secondary Inbox Message 01`
- `Quarterly Planning Alpha`
- `Quarterly Planning Beta`
- `Quarterly Planning Archive`
- `Starred Reference Message`
- `Sent Confirmation Message`

这些主题用于搜索、滚动、账号切换和分类导航断言。

## 覆盖范围

当前 E2E 规格位于 `e2e/test/specs`：

- `smoke.e2e.js`：应用启动和主账号 Inbox seed 可见性
- `account-switching.e2e.js`：主账号和次账号切换
- `email-list.e2e.js`：搜索和滚动到列表底部邮件
- `navigation.e2e.js`：Inbox、Sent、Starred 分类导航
- `compose.e2e.js`：写邮件弹窗打开、填写和重置
- `theme.e2e.js`：浅色、深色、系统主题按钮

## 选择器约定

E2E 交互优先使用 `data-testid`，不依赖 Tailwind class、DOM 顺序或翻译文案。可见文本只用于断言 seed 内容是否出现。

新增或修改 UI 时，如果该元素会被 E2E 使用，应添加稳定的 `data-testid`，并同步更新 page object。

## 产物位置

运行产物默认写入：

- `e2e/artifacts/screenshots`
- `e2e/artifacts/reports`
- `e2e/artifacts/logs`
- `.e2e-data`

这些目录已加入 `.gitignore`，不要提交。

## 常见问题

如果 `tauri-driver` 端口被占用，可以指定新端口：

```bash
WDIO_PORT=4445 bun run test:e2e
```

如果 Linux 无显示环境，需要通过 `xvfb-run` 运行：

```bash
cd e2e
xvfb-run bun run test
```

如果测试失败但截图显示页面内容存在，优先检查 helper 是否使用了 WebView 可见文本读取方式。当前 helper 使用 `document.body.textContent`，比 `$('body').getText()` 更适合 Wry WebView。
