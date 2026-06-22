# Postium Mail E2E 稳定化设计

日期：2026-06-21
状态：已批准进入计划阶段

## 目标

稳定现有 Tauri E2E 测试框架，使它可以作为可靠的 Linux CI 门禁，并成为本地可重复运行的回归测试套件。

本工作会把当前 WebdriverIO + tauri-driver 从冒烟测试脚手架升级为稳定的测试平台，具备隔离数据、确定性测试数据、稳定选择器、独立用例和可诊断失败产物。

## 范围

包含：

- E2E 测试运行在每次独立的应用数据目录中。
- 增加专用 E2E 运行模式：`POSTIUM_E2E=1`。
- E2E 模式下应用启动时自动插入确定性本地测试数据。
- 至少 seed 2 个账号和 48 封邮件。
- 只添加 E2E 必需的 `data-testid` 锚点。
- 重写 page object，使用稳定选择器和显式等待。
- 重写当前 E2E specs，使测试相互独立，并且不静默跳过必要操作。
- 增加 1 条账号切换测试和 1 条邮件列表滚动测试。
- 本阶段让 Linux E2E 成为必过 CI 目标。
- 捕获截图、日志、报告和失败 run 的数据目录。
- 补充本地 E2E 运行和排障文档。

不包含：

- 真实 IMAP、SMTP、OAuth 或远程邮件服务商测试。
- 完整扩展 Rust command/service 测试。
- 完整扩展 Vitest store/component 测试。
- 将 Windows E2E 稳定化为必过 CI 门禁。
- 除稳定当前测试套件、账号切换和滚动测试之外的大型业务流扩展。
- 从生产 DOM 中移除 `data-testid` 属性。

## 设计原则

默认 E2E 测试应使用真实的应用内部链路，并避开外部服务。

以下链路应保持真实：

- Tauri 应用启动
- Svelte UI
- Tauri commands
- Rust services
- SQLite 数据库
- 前端 stores 和路由

以下外部依赖默认 E2E 不参与：

- 真实 IMAP 服务
- 真实 SMTP 发送
- 真实 OAuth 流程
- 真实系统 keyring 依赖
- 线上邮件服务商网络行为

测试数据来源是确定性的本地 seed 数据，不是 mock 前端 API response。这样既保留 E2E 的价值，又移除不稳定的外部依赖。

## 运行架构

E2E 每次运行使用由环境变量控制的专用数据路径。

```text
WDIO onPrepare
  -> 创建 .e2e-data/run-<timestamp>-<pid>
  -> 设置 POSTIUM_E2E=1
  -> 设置 POSTIUM_DATA_DIR=<absolute run dir>
  -> 构建 Tauri debug app

tauri-driver
  -> 启动 app binary

Tauri app run()
  -> 检测 POSTIUM_E2E=1
  -> 要求 POSTIUM_DATA_DIR 存在
  -> init_database(POSTIUM_DATA_DIR)
  -> 运行 migrations
  -> seed_e2e_data(db)
  -> 启动正常应用服务和窗口

WDIO specs
  -> 等待 app ready
  -> 通过稳定 data-testid 选择器交互
  -> 断言确定性 seed 状态
```

## 数据目录解析

Rust 启动逻辑不应继续在 `run()` 中直接硬编码 `~/.postium`。

新增一个小的解析函数：

```rust
fn resolve_data_dir() -> PathBuf
```

行为：

- 普通模式：继续使用当前 `~/.postium` 路径。
- E2E 模式：如果 `POSTIUM_E2E=1`，使用 `POSTIUM_DATA_DIR`。
- E2E 模式缺少 `POSTIUM_DATA_DIR`：启动失败，并给出明确错误。

这样可以保持生产行为不变，同时让 E2E 数据隔离变成显式行为。

## E2E Seed 数据

Seed 数据只在 `POSTIUM_E2E=1` 时由 Rust 在应用启动阶段插入。

建议模块位置：

```text
src-tauri/src/infrastructure/testing/e2e_seed.rs
```

Seed 模块必须幂等：

- 检查主测试账号是否存在。
- 如果存在，认为 seed 已执行，跳过插入。
- 如果不存在，插入全部 E2E 账号和邮件。

Seed 应尽量使用现有 repository/entity 模式。除非遇到 schema 特定行为，否则避免裸 SQL。

### 账号

插入 2 个固定账号。

主账号：

- email: `primary.e2e@postium.test`
- name: `Primary E2E`
- provider: `custom`
- auth_type: `Password`
- sync_enabled: `false`
- color: `#2563eb`

次账号：

- email: `secondary.e2e@postium.test`
- name: `Secondary E2E`
- provider: `custom`
- auth_type: `Password`
- sync_enabled: `false`
- color: `#16a34a`

### 邮件

插入至少 48 封固定邮件。

主账号：36 封

- Inbox: 24
- Sent: 5
- Starred: 4
- Drafts: 2
- Trash: 1

次账号：12 封

- Inbox: 8
- Sent: 2
- Starred: 1
- Drafts: 1

Seed 必须使用固定 subject、sender、body、folder、read state、star state 和 timestamp。Timestamp 应保持确定性并可排序，使列表顺序可预测。

必需 subject 示例：

- `Primary Inbox Message 01`
- `Primary Inbox Message 24`
- `Secondary Inbox Message 01`
- `Quarterly Planning Alpha`
- `Quarterly Planning Beta`
- `Quarterly Planning Archive`
- `Starred Reference Message`
- `Sent Confirmation Message`
- `Draft Proposal Outline`
- `Trash Cleanup Notice`

第一条滚动测试使用 `Primary Inbox Message 24` 作为列表底部目标。

## 选择器策略

E2E 选择器不应依赖 Tailwind class、DOM 位置或翻译后的 UI 文案。

优先级：

1. `data-testid`
2. 稳定的 `aria-label`
3. role/name
4. 可见文本仅用于 seed 内容断言，不用于主交互定位

本阶段必需锚点如下。

Sidebar：

- `sidebar`
- `compose-button`
- `folder-inbox`
- `folder-sent`
- `folder-starred`
- `settings-nav`
- `account-switcher`
- `account-option-primary`
- `account-option-secondary`
- `active-account-label`

Email list：

- `email-list`
- `email-search-input`
- `email-item`
- `email-empty-state`
- `email-refresh-button`

Email detail：

- `email-detail`
- `email-detail-empty`
- `email-subject`
- `email-sender`
- `email-body`
- `email-star-button`
- `email-delete-button`

Compose modal：

- `compose-modal`
- `compose-to-input`
- `compose-cc-input`
- `compose-subject-input`
- `compose-body-editor`
- `compose-send-button`
- `compose-close-button`

Settings：

- `settings-page`
- `theme-light`
- `theme-dark`
- `theme-system`

## Page Object 设计

新增共享选择器 helper：

```text
e2e/helpers/selectors.js
```

示例：

```js
export const byTestId = (id) => $(`[data-testid="${id}"]`);
export const allByTestId = (id) => $$(`[data-testid="${id}"]`);
```

Page object 应满足：

- 使用 `data-testid` 作为主选择器机制。
- 为主要 UI 区域暴露 `waitForReady()` 方法。
- 使用 `waitForDisplayed` 和 `browser.waitUntil`。
- 必需元素缺失时直接失败。
- 避免对必需 UI 使用 `if (element) { ... }`。
- 避免把 `browser.pause()` 作为主要同步机制。
- 避免按位置选择 input。

## Spec 设计

围绕确定性状态和独立 setup 重写当前 specs。

推荐 specs：

```text
e2e/test/specs/smoke.e2e.js
e2e/test/specs/navigation.e2e.js
e2e/test/specs/account-switching.e2e.js
e2e/test/specs/email-list.e2e.js
e2e/test/specs/compose.e2e.js
e2e/test/specs/theme.e2e.js
```

Smoke：

- App 启动。
- Sidebar 出现。
- Email list 出现。
- 主 seed 账号处于激活状态。
- 主账号 inbox seed 邮件可见。

Navigation：

- Inbox 显示主账号 inbox 邮件。
- Sent 显示主账号 sent 邮件。
- Starred 显示星标邮件。
- 必需 folder 按钮可见并可点击。

Account switching：

- 默认激活账号是主账号。
- 打开账号切换器。
- 切换到次账号。
- 验证 `Secondary Inbox Message 01` 出现。
- 验证主账号专属 inbox 邮件不显示。
- 切回主账号。
- 验证主账号 inbox 邮件出现。

Email list：

- 搜索 `Quarterly Planning`。
- 验证确定性匹配 subject 出现。
- 清空搜索并验证普通 inbox 恢复。
- 滚动邮件列表到 `Primary Inbox Message 24`。
- 点击它，并验证 detail pane 显示同一 subject。

Compose：

- 打开 compose modal。
- 填写 recipient、subject 和 body。
- 验证值已输入。
- 关闭 modal。
- 在独立测试中重新打开 modal，并验证它从干净状态开始。

Theme：

- 进入 settings。
- 切换到 dark，并验证 `html.dark`。
- 切换到 light，并验证 `html.dark` 被移除。
- 点击 system theme，并验证控件可用。

## WDIO 生命周期

`e2e/wdio.conf.js` 应显式管理测试运行环境。

职责：

- 创建 `.e2e-data/run-<timestamp>-<pid>`。
- 将 `POSTIUM_E2E=1` 和 `POSTIUM_DATA_DIR` 传给 Tauri build/app 进程。
- 使用 `WDIO_PORT`，默认值为 `4444`。
- 测试前构建 debug app。
- 构建后检查 app binary 存在。
- 启动 `tauri-driver`。
- 等待 `127.0.0.1:<port>` 可连接。
- 测试完成或中断时 kill `tauri-driver`。
- 成功时删除本次 run 数据目录。
- 失败时保留本次 run 数据目录。

## 失败产物

创建目录：

```text
e2e/artifacts/
├── screenshots/
├── logs/
└── reports/
```

捕获：

- 每个失败测试的截图。
- WDIO report 输出。
- 可行时保留 driver 和 app 日志。
- 失败的 `.e2e-data/run-*` 目录。

成功运行可以清理 run 数据目录。失败运行应保留它，供本地排查和 CI 上传。

## CI 设计

Linux E2E 是本阶段必过目标。

推荐 CI 结构：

- Rust tests
- Frontend tests
- Linux E2E tests

Windows E2E 本阶段不应作为必过门禁。它可以从必过 matrix 中移除，或移动到单独的非阻塞 job。建议第一步从必过 E2E matrix 中移除 Windows，并在后续稳定化阶段恢复。

Linux E2E 应：

- 安装 Tauri Linux 依赖。
- 安装 `webkitgtk-webdriver` 和 `xvfb`。
- 安装 `tauri-driver`。
- 使用 `xvfb-run` 运行 E2E。
- 始终上传 `e2e/artifacts/**`。
- 仅失败时上传 `.e2e-data/**`。

## 文档

新增本地 E2E 文档，位置可以是 `README.md` 或 `docs/testing/e2e.md`。

文档应说明：

- 本地必需依赖。
- `cargo install tauri-driver --locked`。
- 在根目录和 `e2e` 下运行 `bun install`。
- `bun run test:e2e`。
- E2E 环境变量。
- 产物位置。
- 如何检查失败的 `.e2e-data` 目录。
- Linux CI 是本阶段唯一必过 E2E 门禁。

## 风险

Seed 和 schema 不匹配：

- 缓解：尽量使用 repository/entity 路径，并显式填写 seed 字段。

文件夹语义不匹配：

- 缓解：本阶段只断言当前已支持的 folder/category 行为。

生产 DOM 中存在 `data-testid`：

- 缓解：第一阶段接受它以换取稳定性；后续如有需要再考虑剥离。

滚动测试 flaky：

- 缓解：只测试底部目标变为可见且可点击，不断言精确像素位置。

Linux 图形环境差异：

- 缓解：继续使用 `xvfb-run`；Windows 稳定化推迟到后续阶段。

## 验收标准

- 本地 E2E 不读写 `~/.postium`。
- 本地 E2E 使用本次运行专属的 `.e2e-data/run-*` 目录。
- 如果 `POSTIUM_E2E=1` 但缺少 `POSTIUM_DATA_DIR`，E2E 模式会明确失败。
- Seed 创建至少 2 个账号。
- Seed 创建至少 48 封邮件。
- 主账号至少有 24 封 inbox 邮件。
- 现有 E2E specs 使用 `data-testid` 作为主选择器。
- 测试不再通过 `if (element)` 静默跳过必需操作。
- 测试不依赖前一个 `it` 留下的状态。
- 存在账号切换 E2E 测试。
- 存在邮件列表滚动 E2E 测试。
- 失败的 E2E 测试会保存截图。
- CI 上传 E2E artifacts。
- CI 上传失败 run 的数据目录。
- Linux E2E 作为必过门禁通过。
- Windows E2E 本阶段不是必过项。

## 实施顺序

1. 增加 Rust E2E 模式数据目录解析。
2. 增加 Rust E2E seed 模块，包含 2 个账号和至少 48 封邮件。
3. 添加必需的 `data-testid` 锚点。
4. 升级 WDIO 生命周期和 artifact 处理。
5. 围绕 `data-testid` 和显式等待重写 page objects。
6. 重写 smoke、navigation、account switching、email list scrolling、compose 和 theme specs。
7. 简化 CI，使 Linux E2E 成为必过项并上传 artifacts。
8. 添加本地 E2E 文档。

