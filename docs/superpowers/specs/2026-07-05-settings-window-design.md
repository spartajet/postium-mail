# 设置页独立窗体重构设计

## 背景

当前设置页是 `/settings` 路由，被根 `src/routes/+layout.svelte` 包裹。这个根布局同时负责主邮件工作台的标题栏、侧边栏、状态栏、全局写邮件模态框、添加账号模态框和托盘事件监听。因此设置页虽然已经有自己的设置分组导航，但仍然运行在主邮件工作台的壳中。

这会带来两个问题：

1. 设置页职责和邮件工作台职责混在一起。用户进入设置页时，主窗口离开邮件列表上下文。
2. 后续设置项变多后，设置中心更适合拥有独立窗口尺寸、关闭行为和生命周期。

本次重构目标是把设置页转为 Tauri 单独窗体，并让主窗口继续专注邮件工作台。

## 目标

1. 点击主窗口侧边栏的设置入口时，打开一个独立设置窗体。
2. 设置窗体使用单例行为：已存在时聚焦和显示，不重复创建多个设置窗体。
3. `/settings` 页面不再被主邮件工作台布局包裹，不显示主侧边栏和状态栏。
4. 主窗口保留原有邮件工作台布局、托盘行为、写邮件模态框和状态栏。
5. 设置页现有分组导航继续保留：通用、外观、账号管理。
6. 现有设置功能继续可用：主题切换、语言切换、账号列表和删除。
7. 保留现有测试依赖的 `data-testid`，包括 `settings-nav`、`settings-page`、`settings-nav-general`、`settings-nav-appearance`、`settings-nav-accounts`、`theme-light`、`theme-dark`、`theme-system`、`settings-account-card`。

## 非目标

1. 不新增新的设置项。
2. 不调整账号、主题、语言的业务逻辑。
3. 不改动邮件同步、邮件列表、文件夹识别逻辑。
4. 不新增设置子路由。
5. 不允许设置窗体创建多个实例。
6. 不在本设计阶段提交代码。

## 推荐方案

采用“后端命令创建单例设置窗体 + SvelteKit route group 拆分布局”的方案。

### Tauri 窗口策略

新增 Tauri 命令：

```rust
open_settings_window(app: tauri::AppHandle) -> Result<(), MailError>
```

行为：

1. 先通过 `app.get_webview_window("settings")` 查询现有设置窗体。
2. 如果存在：
   - 调用 `show()`。
   - 调用 `set_focus()`。
   - 返回 `Ok(())`。
3. 如果不存在：
   - 使用 `tauri::WebviewWindowBuilder` 创建 label 为 `settings` 的窗口。
   - URL 指向 `/settings`。
   - 标题为 `Postium Mail Settings`。
   - 推荐尺寸：宽 960，高 720，最小宽 760，最小高 560。
   - 使用无原生装饰窗口，复用主窗口自定义标题栏样式。
   - 创建后按主窗口外框位置和尺寸计算中心点，再显示设置窗口。

命令放在新的 `src-tauri/src/command/window.rs` 中，避免继续扩大 `email.rs` 或 `account.rs` 这类业务命令文件。

### Capability 策略

当前 `src-tauri/capabilities/default.json` 和 `desktop.json` 只包含 `main` 窗口。新增设置窗体后，需要把 `settings` 加入相关 capability 的 `windows` 列表。

默认能力需要覆盖：

```json
"windows": ["main", "settings"]
```

窗口状态插件 capability 同样应包含：

```json
"windows": ["main", "settings"]
```

这样设置窗体能调用当前页面所需的 Tauri 命令和插件权限。

### SvelteKit 布局策略

将当前根布局拆成两层职责：

1. `src/routes/+layout.svelte`
   - 只保留全局基础能力：
     - `app.css`
     - 主题、国际化、账户、邮件、同步、toast store 创建
     - 全局错误处理
     - toast 容器
     - `{@render children()}`
   - 不渲染主工作台的 `TitleBar`、`Sidebar`、`StatusBar`。

2. `src/routes/(main)/+layout.svelte`
   - 承载当前主邮件工作台壳：
     - `TitleBar`
     - `Sidebar`
     - `StatusBar`
     - `ComposeModal`
     - `AddAccountModal`
     - 托盘事件监听
     - 背景装饰
   - 原主路由页面迁入 route group，URL 保持不变。

3. 路由移动：
   - `src/routes/+page.svelte` → `src/routes/(main)/+page.svelte`
   - `src/routes/calendar/+page.svelte` → `src/routes/(main)/calendar/+page.svelte`
   - `src/routes/workflow/+page.svelte` → `src/routes/(main)/workflow/+page.svelte`
   - `src/routes/settings/+page.svelte` 保持原路径，不进入 `(main)`。

SvelteKit route group 不改变 URL，因此 `/`、`/calendar`、`/workflow`、`/settings` 对用户保持不变。

### 设置入口策略

`src/lib/components/layout/Sidebar.svelte` 中设置按钮不再执行：

```ts
goto("/settings")
```

改为：

```ts
await commands.openSettingsWindow()
```

失败处理：

1. 优先显示 toast 错误。
2. 如果 toast context 不可用，回退到 `goto("/settings")`，保证开发环境和浏览器环境仍能进入设置页。

### 设置页关闭行为

`src/routes/settings/+page.svelte` 顶部返回按钮语义改为关闭设置窗口。

行为：

1. 如果运行在 Tauri settings 窗口中，调用 `getCurrentWindow().close()`。
2. 如果不是 Tauri 环境或关闭失败，回退 `goto("/")`。

这保证 `/settings` 在浏览器开发模式下仍可访问。

## 数据与状态

全局 store 仍由根布局创建，因此主窗口和设置窗口各自会初始化一套前端 store 实例。后端数据库和 Tauri command state 是共享的，因此设置窗体内的账号、主题、语言操作仍通过现有命令和本地前端状态工作。

注意事项：

1. 主题和语言当前主要是前端本地状态。如果用户在设置窗体改主题，主窗口是否实时同步取决于现有 store 持久化和跨窗口事件机制。本次重构不新增跨窗口实时同步。
2. 账号列表通过后端命令加载，设置窗体打开时可以独立加载账号列表。
3. 托盘事件只保留在主工作台布局中，避免设置窗体重复监听托盘事件。

## 用户体验

1. 主窗口点击设置后，主邮件列表不离开当前上下文。
2. 设置窗体可以关闭，关闭后主窗口仍保持原状态。
3. 重复点击设置只聚焦已有设置窗体。
4. 设置窗体使用和主窗口一致的自定义标题栏，但关闭行为为关闭设置窗体而不是隐藏到托盘。
5. 设置页内部继续使用当前“左侧导航 + 右侧内容”的设置中心布局。

## 可访问性

1. 主窗口设置按钮保留 `data-testid="settings-nav"`。
2. 设置窗体内分组按钮继续使用 `button` 和 `aria-current="page"`。
3. 设置页关闭按钮保留可访问名称，标题为设置页标题或关闭设置。
4. 自定义标题栏提供关闭、最小化、最大化和拖拽行为。

## 测试策略

### Rust

新增或更新测试覆盖：

1. `open_settings_window` 命令可编译并注册到 specta builder。
2. 单例逻辑在代码结构上可通过单元拆分测试时覆盖；如果 Tauri window 运行时难以在单元测试中创建，则用编译测试和前端 mock 测试覆盖调用边界。

### 前端

新增或更新测试覆盖：

1. `Sidebar.svelte` 点击设置按钮调用 `commands.openSettingsWindow()`。
2. 设置命令失败时 fallback 到 `goto("/settings")`。
3. `settings/+page.svelte` 独立渲染仍默认显示通用分组。
4. 点击外观和账号管理分组行为保持不变。
5. 主布局 route group 后，主页面仍渲染侧边栏，设置页面测试中不依赖主侧边栏。

### 验证命令

实现完成后至少运行：

```bash
rtk bun run check
rtk bun run test:frontend
rtk cargo test
rtk cargo fmt --check
```

## 风险

1. SvelteKit route group 移动文件可能影响测试 import 路径。
2. Tauri capability 如果漏加 `settings`，设置窗体中的命令会被拒绝。
3. 根布局拆分时，如果全局 context 初始化放错层，设置页可能拿不到 i18n/theme/account store。
4. 如果保留托盘监听在根布局，设置窗体会重复监听托盘事件，因此托盘逻辑必须移动到 `(main)` layout。
5. 设置窗体使用无装饰窗口时，标题栏关闭行为必须区别于主窗口：主窗口隐藏到托盘，设置窗体直接关闭。

## 验收标准

1. 主窗口点击设置入口会打开独立设置窗体。
2. 重复点击设置入口不会创建多个设置窗体，只会聚焦已有窗体。
3. 设置窗体显示设置页，不显示主侧边栏、主状态栏和主邮件工作台。
4. 设置窗体弹出位置在主窗口中心附近。
5. 设置窗体标题栏视觉样式和主窗口一致。
4. 主窗口仍保留邮件工作台布局。
5. 设置页通用、外观、账号管理三个分组可正常切换。
6. 主题、语言、账号管理现有行为不回退。
7. `rtk bun run check`、`rtk bun run test:frontend`、`rtk cargo test`、`rtk cargo fmt --check` 均通过。
