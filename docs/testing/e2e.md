# E2E 测试说明（GPUI）

本文档说明 Postium Mail GPUI 版 E2E 测试的运行方式、实现模式、场景清单与已知差异。

> 旧 WebdriverIO/tauri-driver 版 E2E（`e2e/`）已废弃，随 Tauri 轨道保留为遗留；本文档仅描述新套件。

## 运行方式

```bash
cargo test -p postium-ui e2e_tests              # 全部 E2E（约 0.2s）
cargo test -p postium-ui e2e_tests::boots_with_inbox_seed   # 单个场景
cargo test -p postium-ui e2e_tests -- --nocapture           # 显示日志
```

无需构建应用、无需 WebDriver、无需显示服务器（headless）——Windows/Linux/macOS 直接运行。

## 实现模式

测试位于 `crates/postium-ui/src/e2e_tests.rs`（`#[cfg(test)]` 模块，可访问 crate 内部构造）：

1. **真实全栈装配**：`boot(tempdir)` → `CoreApi::init(dir, e2e=true, false)`（SQLite + e2e seed + 全部服务），与生产启动同一条路径；
2. **headless 窗口**：`cx.add_window_view(...)` 构造 `AppShell` / `ComposeView` 根视图（真实渲染管线）；
3. **用户操作**：
   - 按钮类操作 = 调用与 `on_click` 完全相同的 handler（如 `store.navigate`、`toggle_theme`、`persist_draft`）；
   - 文本输入 = `window.focus(handle)` + `cx.simulate_keystrokes(...)`（真实键盘事件链路）；
4. **异步 settle**：`wait_for(cx, ...)` = `run_until_parked` + 15ms 轮询（tokio 后台线程完成 DB 查询需要真实时间），200 轮超时 3s。

### E2E 模式的 core 行为（与旧版 seed 完全一致的数据）

- seed 固定两账号：`primary.e2e@postium.test` / `secondary.e2e@postium.test`；
- 固定主题锚点：`Primary Inbox Message NN`、`Quarterly Planning Alpha/Beta/Archive`、`Sent Confirmation Message`、`Starred Reference Message` 等；
- 远程操作 / 草稿写入均为 LocalOnly 实现，凭证回退占位（见 overview.md）。

## 场景清单（与旧 WebdriverIO 用例一一对应）

| 测试函数 | 旧用例（e2e/test/specs/） | 断言要点 |
|---|---|---|
| `boots_with_inbox_seed` | smoke.e2e.js | 两账号 seed、inbox 装载 `Primary Inbox Message` 系列 |
| `switches_between_accounts` | account-switching.e2e.js | 次账号 INBOX 列表全为该账号邮件；切回全部收件箱 |
| `navigates_sent_starred_inbox` | navigation.e2e.js | Sent→`Sent Confirmation`、Starred→`Starred Reference`、Inbox→`Primary Inbox` |
| `searches_quarterly_planning` | email-list.e2e.js（搜索） | FTS 命中 ≥2 且全部匹配 `Quarterly Planning` |
| `stars_and_deletes_selected` | email-list.e2e.js（星标/删除） | 星标置位（DB 级）、删除后列表刷新不含该邮件 |
| `compose_fills_fields_and_draft` | compose.e2e.js | 账号自动选中、收件人键入生效、空主题拦截、草稿保存返回 draft_id |
| `compose_reopens_empty` | compose.e2e.js（重开清空） | 新写信窗口字段为空 |
| `toggles_theme_and_persists` | theme.e2e.js | 深浅双向切换 + `ui-state.json` 落盘 |

## 与旧版（WebdriverIO）的差异

| 维度 | 旧 | 新 |
|---|---|---|
| 驱动 | 构建 debug app + tauri-driver + WebKit WebDriver + xvfb | 同进程 `TestAppContext`，无外部进程 |
| 元素定位 | CSS 选择器 / `data-testid` | 视图与状态实体直接断言 |
| 交互 | 鼠标/键盘事件坐标点击 | on_click handler 直调 + 键盘事件模拟 |
| 速度 | 分钟级 | ~0.2 秒 |

已知取舍：

- 无元素坐标定位 API，纯指针类交互（如列表滚动到底）不做像素级模拟——滚动加载等功能由 `load_more`/`sync_older` handler 级验证覆盖；
- `real-accounts.e2e.js`（真实账号 UI 联测）未迁移：其 core 层真实验证已保留（`real_mail_accounts` 测试），UI 层待真实邮箱验收 GPUI 版后按新模式重建。

## 排障

- 超时（`等待条件超时`）：多为异步链路未完成——检查 handler 是否真的触发 spawn、`wait_for` 条件是否正确；
- 环境变量污染：`toggles_theme_and_persists` 会临时设置 `POSTIUM_E2E`/`POSTIUM_DATA_DIR`（结束时移除），并行测试如遇数据目录异常先怀疑此处；
- 每个测试使用独立 `tempdir`，无跨测试状态。
