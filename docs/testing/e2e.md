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

## CI 产物和故障排查

Linux CI 会在 E2E job 结束后上传 `e2e-artifacts`。如果 E2E 失败，应优先下载并查看：

- `e2e/artifacts/logs/environment-*.json`
- `e2e/artifacts/logs/tauri-driver-*.log`
- `e2e/artifacts/reports/*.xml`
- `e2e/artifacts/screenshots/*.png`

如果 E2E job 失败，CI 还会上传 `.e2e-data`，其中包含本次运行保留下来的 SQLite 数据库，可用于检查 seed 数据、迁移状态和搜索索引。

### 环境快照

`environment-*.json` 记录本次运行的关键环境：

- `runId`：本次运行 ID，和数据目录、driver 日志文件名对应
- `appPath`：Tauri debug 应用路径
- `driverPath`：`tauri-driver` 路径
- `runDataDir`：本次独立数据目录
- `wdioPort`：WebDriver 端口
- `bun`：Bun 版本探测结果
- `tauriDriver`：`tauri-driver` 探测结果
- `webkitWebDriver`：Linux WebKit WebDriver 探测结果
- `env.DISPLAY`、`env.WAYLAND_DISPLAY`、`env.XDG_SESSION_TYPE`：显示环境

如果 CI 或本地出现 WebKit/WebDriver 相关失败，先确认：

- `webkitWebDriver.status` 是否为 `0` 或是否能输出帮助信息
- `DISPLAY` 是否为空
- CI 是否通过 `xvfb-run bun run test` 启动
- `wdioPort` 是否被其他进程占用

### tauri-driver 日志

`tauri-driver-*.log` 记录 driver 生命周期和应用 stderr/stdout。正常结束时应能看到：

```json
{"event":"close-request", ...}
{"event":"exit","code":0,"signal":null,"expected":true, ...}
{"event":"close-complete","exitCode":0,"signalCode":null, ...}
```

判断方式：

- `expected: true`：测试框架主动关闭 driver，通常是正常结束。
- `expected: false`：driver 在测试框架关闭前退出，属于异常退出，应检查同一日志中前面的 WebKit、Tauri 或系统错误。
- `signal` 或 `signalCode` 非空：进程被信号结束，应结合 CI runner 日志判断是否超时、资源不足或被外部终止。
- 出现 `close-timeout`：测试框架请求关闭后 driver 没有及时退出，优先检查 WebKitWebDriver 是否卡死。

如果出现 `tauri-driver exited unexpectedly`，说明 driver 的 `exit` 事件发生在测试框架预期关闭之前。这种情况下 `.e2e-data/run-*` 会保留，便于复现。

### JUnit 和截图

`reports/*.xml` 用于定位失败的 spec 和测试名称。`screenshots/*.png` 只在测试失败时保存，文件名来自失败测试标题。

排查顺序建议：

1. 先看 GitHub Actions 中失败的 job 和 step。
2. 再看 `reports/*.xml` 确认失败用例。
3. 查看截图，判断是页面未加载、元素不可见还是断言内容不对。
4. 查看 `tauri-driver-*.log`，判断 driver 是否提前退出。
5. 查看 `environment-*.json`，确认显示环境、driver 路径和端口。
6. 如有 `.e2e-data`，打开 SQLite 数据库检查 seed 数据和迁移结果。

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
