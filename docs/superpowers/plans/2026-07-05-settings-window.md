# Settings Window Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将设置页从主邮件工作台路由页重构为 Tauri 单例独立窗体。

**Architecture:** 后端新增 `open_settings_window` 命令负责创建或聚焦 label 为 `settings` 的 WebviewWindow。前端用 SvelteKit route group 将主邮件工作台布局移动到 `(main)`，根布局只保留全局 store/context，`/settings` 独立渲染设置中心页面。

**Tech Stack:** Tauri v2、SvelteKit、Svelte 5 Runes、tauri-specta、Vitest、Testing Library Svelte、Rust cargo test。

## Global Constraints

- 文档使用中文书写。
- 没有用户明确指令，不提交代码。
- shell 命令使用 `rtk` 前缀。
- 使用 TDD：每个行为改动先写失败测试，再实现。
- 保留现有 URL：`/`、`/calendar`、`/workflow`、`/settings`。
- 保留现有 `data-testid`：`settings-nav`、`settings-page`、`settings-nav-general`、`settings-nav-appearance`、`settings-nav-accounts`、`theme-light`、`theme-dark`、`theme-system`、`settings-account-card`。
- 设置窗体必须是单例窗口，label 固定为 `settings`。
- 不新增真实设置项。
- 不调整邮件同步、邮件列表和文件夹识别逻辑。

---

## File Structure

- Create: `src-tauri/src/command/window.rs`
  - 负责窗口相关 Tauri command。
  - 暴露 `open_settings_window(app: tauri::AppHandle) -> Result<(), MailError>`。
- Modify: `src-tauri/src/command/mod.rs`
  - 导出 `window` command 模块。
- Modify: `src-tauri/src/lib.rs`
  - 在 tauri-specta builder 中注册 `command::window::open_settings_window`。
- Modify: `src-tauri/capabilities/default.json`
  - 将 `settings` 加入 windows 列表。
- Modify: `src-tauri/capabilities/desktop.json`
  - 将 `settings` 加入 windows 列表。
- Modify: `src/lib/bindings.ts`
  - 同步新增 `commands.openSettingsWindow()`。
- Modify: `src/routes/+layout.svelte`
  - 保留全局 store/context 初始化、错误处理、Toast 容器和全局样式。
  - 移除主邮件工作台 shell。
- Create: `src/routes/(main)/+layout.svelte`
  - 承载当前主邮件工作台 shell：TitleBar、Sidebar、StatusBar、ComposeModal、AddAccountModal、托盘事件。
- Move: `src/routes/+page.svelte` → `src/routes/(main)/+page.svelte`
  - 主页面保持 URL `/`。
- Move: `src/routes/calendar/+page.svelte` → `src/routes/(main)/calendar/+page.svelte`
  - 日历页面保持 URL `/calendar`。
- Move: `src/routes/workflow/+page.svelte` → `src/routes/(main)/workflow/+page.svelte`
  - 工作流页面保持 URL `/workflow`。
- Modify: `src/lib/components/layout/Sidebar.svelte`
  - 设置按钮改为调用 `commands.openSettingsWindow()`。
- Modify: `src/routes/settings/+page.svelte`
  - 返回按钮改为关闭当前设置窗口，失败时 fallback 到 `goto("/")`。
- Create/Modify: `src/lib/__tests__/components/Sidebar.test.ts`
  - 覆盖设置按钮打开设置窗体。
- Modify: `src/lib/__tests__/routes/settings-page.test.ts`
  - 覆盖设置页独立渲染和关闭按钮 fallback。

---

### Task 1: Add Tauri Settings Window Command

**Files:**
- Create: `src-tauri/src/command/window.rs`
- Modify: `src-tauri/src/command/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/bindings.ts`

**Interfaces:**
- Produces Rust command:
  - `pub async fn open_settings_window(app: tauri::AppHandle) -> Result<(), MailError>`
- Produces frontend binding:
  - `commands.openSettingsWindow: () => Promise<Result<null, MailError>>`

- [ ] **Step 1: Write a compile-facing registration test**

Add command module declaration expectation by creating the command file first with a failing compile reference in `src-tauri/src/lib.rs` registration:

```rust
command::window::open_settings_window,
```

Run:

```bash
rtk cargo test open_settings_window
```

Expected:

- Fails to compile because `command::window` does not exist.

- [ ] **Step 2: Create `src-tauri/src/command/window.rs`**

Add:

```rust
use crate::error::MailError;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_WINDOW_WIDTH: f64 = 960.0;
const SETTINGS_WINDOW_HEIGHT: f64 = 720.0;

fn centered_position(
    parent_position: PhysicalPosition<i32>,
    parent_size: PhysicalSize<u32>,
    child_width: f64,
    child_height: f64,
) -> PhysicalPosition<i32> {
    let x = parent_position.x + ((parent_size.width as f64 - child_width) / 2.0).round() as i32;
    let y = parent_position.y + ((parent_size.height as f64 - child_height) / 2.0).round() as i32;
    PhysicalPosition::new(x, y)
}

#[tauri::command]
#[specta::specta]
pub async fn open_settings_window(app: AppHandle) -> Result<(), MailError> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        window
            .show()
            .map_err(|err| MailError::InvalidParam(format!("显示设置窗口失败: {err}")))?;
        window
            .set_focus()
            .map_err(|err| MailError::InvalidParam(format!("聚焦设置窗口失败: {err}")))?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        SETTINGS_WINDOW_LABEL,
        WebviewUrl::App("/settings".into()),
    )
    .title("Postium Mail Settings")
    .inner_size(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT)
    .min_inner_size(760.0, 560.0)
    .decorations(false)
    .visible(false)
    .build()
    .map_err(|err| MailError::InvalidParam(format!("创建设置窗口失败: {err}")))?;

    if let Some(main_window) = app.get_webview_window("main")
        && let (Ok(parent_position), Ok(parent_size)) =
            (main_window.outer_position(), main_window.outer_size())
    {
        let position = centered_position(
            parent_position,
            parent_size,
            SETTINGS_WINDOW_WIDTH,
            SETTINGS_WINDOW_HEIGHT,
        );
        let _ = window.set_position(position);
    }

    window
        .show()
        .map_err(|err| MailError::InvalidParam(format!("显示设置窗口失败: {err}")))?;
    window
        .set_focus()
        .map_err(|err| MailError::InvalidParam(format!("聚焦设置窗口失败: {err}")))?;
    Ok(())
}
```

- [ ] **Step 3: Export the command module**

In `src-tauri/src/command/mod.rs`, add:

```rust
pub mod window;
```

- [ ] **Step 4: Register command with specta builder**

In `src-tauri/src/lib.rs`, inside `collect_commands!`, add:

```rust
command::window::open_settings_window,
```

- [ ] **Step 5: Sync TypeScript binding manually if generated export is not run**

In `src/lib/bindings.ts`, add in `commands`:

```ts
openSettingsWindow: () =>
    typedError<null, MailError>(
        __TAURI_INVOKE("open_settings_window", {}),
    ),
```

Use the same formatting style as nearby bindings.

- [ ] **Step 6: Verify Rust compile**

Run:

```bash
rtk cargo test open_settings_window
```

Expected:

- Command compiles.
- If no test name matches, cargo reports `0 passed` filtered output with exit code 0.

---

### Task 2: Update Capabilities for the Settings Window

**Files:**
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/capabilities/desktop.json`

**Interfaces:**
- Consumes window label: `"settings"`.
- Produces capability windows lists containing both `"main"` and `"settings"`.

- [ ] **Step 1: Update default capability**

Change `src-tauri/capabilities/default.json`:

```json
"windows": ["main", "settings"]
```

- [ ] **Step 2: Update desktop capability**

Change `src-tauri/capabilities/desktop.json`:

```json
"windows": ["main", "settings"]
```

- [ ] **Step 3: Verify JSON validity**

Run:

```bash
rtk cargo test open_settings_window
```

Expected:

- No JSON parsing error during Tauri context generation.

---

### Task 3: Split Root Layout and Main Workspace Layout

**Files:**
- Modify: `src/routes/+layout.svelte`
- Create: `src/routes/(main)/+layout.svelte`
- Move: `src/routes/+page.svelte` → `src/routes/(main)/+page.svelte`
- Move: `src/routes/calendar/+page.svelte` → `src/routes/(main)/calendar/+page.svelte`
- Move: `src/routes/workflow/+page.svelte` → `src/routes/(main)/workflow/+page.svelte`

**Interfaces:**
- Root layout produces global store context.
- `(main)` layout consumes global stores and modal context setup for main workspace.
- URLs remain unchanged because `(main)` is a SvelteKit route group.

- [ ] **Step 1: Write/adjust a settings route test that proves no main shell is required**

Update `src/lib/__tests__/routes/settings-page.test.ts` to continue rendering:

```ts
render(SettingsPage);
expect(screen.getByTestId("settings-page").isConnected).toBe(true);
```

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- The test currently passes before the split.
- After the split it must continue passing.

- [ ] **Step 2: Create route group directories**

Create directories:

```text
src/routes/(main)/
src/routes/(main)/calendar/
src/routes/(main)/workflow/
```

- [ ] **Step 3: Move route page files**

Move:

```text
src/routes/+page.svelte -> src/routes/(main)/+page.svelte
src/routes/calendar/+page.svelte -> src/routes/(main)/calendar/+page.svelte
src/routes/workflow/+page.svelte -> src/routes/(main)/workflow/+page.svelte
```

- [ ] **Step 4: Replace root layout with global provider layout**

Keep in `src/routes/+layout.svelte`:

```svelte
<script lang="ts">
    import "../app.css";
    import { onMount } from "svelte";
    import { createThemeState } from "$lib/stores/theme.svelte";
    import { createI18nState } from "$lib/stores/i18n.svelte";
    import { createAccountState } from "$lib/stores/account.svelte";
    import { createEmailState } from "$lib/stores/email.svelte";
    import { createSyncState } from "$lib/stores/sync.svelte";
    import { createToastState } from "$lib/stores/toast.svelte";
    import { setupGlobalErrorHandler } from "$lib/utils/error.js";
    import ToastContainer from "$lib/components/common/Toast.svelte";

    let { children } = $props();

    createThemeState();
    createI18nState();
    createAccountState();
    createEmailState();
    createSyncState();
    createToastState();

    onMount(() => {
        setupGlobalErrorHandler();
    });
</script>

<div class="h-screen overflow-hidden bg-background text-foreground">
    {@render children()}
    <ToastContainer />
</div>
```

- [ ] **Step 5: Create `(main)` layout with the previous main shell**

Create `src/routes/(main)/+layout.svelte` using the previous root shell:

- import `listen`
- import `setContext`
- import `TitleBar`, `Sidebar`, `StatusBar`
- import `ComposeModal`, `AddAccountModal`
- get stores via `getAccountState()` and `getSyncState()`
- set modal contexts
- listen to `tray-action`
- render `TitleBar`, `Sidebar`, child content, `StatusBar`, modals

The outer shell must keep:

```svelte
<div class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground">
```

The child content wrapper must keep:

```svelte
<div class="flex-1 overflow-hidden">
    {@render children()}
</div>
```

- [ ] **Step 6: Verify route split**

Run:

```bash
rtk bun run check
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- Svelte check reports 0 errors.
- Settings page tests pass.

---

### Task 4: Wire Sidebar Settings Button to the Settings Window

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte`
- Create/Modify: `src/lib/__tests__/components/Sidebar.test.ts`

**Interfaces:**
- Consumes `commands.openSettingsWindow()`.
- Keeps `data-testid="settings-nav"`.
- Falls back to `goto("/settings")` if command fails.

- [ ] **Step 1: Write failing Sidebar test**

In `src/lib/__tests__/components/Sidebar.test.ts`, add mocks:

```ts
const openSettingsWindow = vi.fn().mockResolvedValue({ status: "ok", data: null });
const goto = vi.fn();

vi.mock("$app/navigation", () => ({ goto }));
vi.mock("$lib/bindings", () => ({
    commands: {
        openSettingsWindow,
    },
}));
```

Add test:

```ts
it("点击设置入口打开独立设置窗体", async () => {
    render(Sidebar);

    await fireEvent.click(screen.getByTestId("settings-nav"));

    expect(openSettingsWindow).toHaveBeenCalledOnce();
    expect(goto).not.toHaveBeenCalledWith("/settings");
});
```

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/Sidebar.test.ts
```

Expected:

- Fails because Sidebar still calls `goto("/settings")`.

- [ ] **Step 2: Implement Sidebar handler**

In `src/lib/components/layout/Sidebar.svelte`, import:

```ts
import { commands } from "$lib/bindings";
```

Add function:

```ts
async function openSettings() {
    activeFolder = "";
    try {
        const result = await commands.openSettingsWindow();
        if (result.status === "error") {
            goto("/settings");
        }
    } catch {
        goto("/settings");
    }
}
```

Replace settings button click:

```svelte
onclick={openSettings}
```

- [ ] **Step 3: Verify Sidebar test**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/components/Sidebar.test.ts
```

Expected:

- Test passes.

---

### Task 5: Adjust Settings Page Close Behavior

**Files:**
- Modify: `src/routes/settings/+page.svelte`
- Modify: `src/lib/__tests__/routes/settings-page.test.ts`

**Interfaces:**
- Produces `closeSettingsWindow(): Promise<void>`.
- Uses `getCurrentWindow().close()` in Tauri.
- Fallbacks to `goto("/")`.

- [ ] **Step 1: Write failing settings close test**

In `src/lib/__tests__/routes/settings-page.test.ts`, add mock:

```ts
const closeWindow = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({
        close: closeWindow,
    }),
}));
```

Add test:

```ts
it("点击返回按钮关闭设置窗体", async () => {
    render(SettingsPage);

    await fireEvent.click(screen.getByTestId("settings-close-button"));

    expect(closeWindow).toHaveBeenCalledOnce();
    expect(mocks.goto).not.toHaveBeenCalledWith("/");
});
```

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- Fails because close button currently has no `settings-close-button` test id or still calls `goto("/")`.

- [ ] **Step 2: Implement close handler**

In `src/routes/settings/+page.svelte`, import:

```ts
import { getCurrentWindow } from "@tauri-apps/api/window";
```

Add:

```ts
async function closeSettingsWindow() {
    try {
        await getCurrentWindow().close();
    } catch {
        goto("/");
    }
}
```

Update close/back button:

```svelte
<button
    data-testid="settings-close-button"
    onclick={closeSettingsWindow}
    title={t.settings.title}
>
```

- [ ] **Step 3: Verify settings route test**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- Test passes.

---

### Task 6: Full Verification

**Files:**
- No new files.

**Interfaces:**
- Verifies all earlier tasks.

- [ ] **Step 1: Run Svelte check**

Run:

```bash
rtk bun run check
```

Expected:

- `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
rtk bun run test:frontend
```

Expected:

- All Vitest files pass.

- [ ] **Step 3: Run Rust tests**

Run:

```bash
rtk cargo test
```

from `src-tauri` or:

```bash
rtk bun run test:rust
```

from repo root.

Expected:

- All non-ignored Rust tests pass.

- [ ] **Step 4: Run Rust formatting check**

Run:

```bash
rtk cargo fmt --check
```

from `src-tauri`.

Expected:

- Exit code 0.

- [ ] **Step 5: Review git diff**

Run:

```bash
rtk git diff --stat
rtk git diff -- src-tauri/src/command/window.rs src-tauri/src/command/mod.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json src-tauri/capabilities/desktop.json src/lib/bindings.ts src/routes src/lib/components/layout/Sidebar.svelte src/lib/__tests__
```

Expected:

- Diff includes only settings-window refactor files and tests.
- No unrelated mail sync, folder detection, account auth, or attachment changes.

---

## Self-Review Notes

- Spec coverage: The plan covers Tauri command, capability windows, route group split, settings entry, close behavior, and verification.
- Completion marker scan: No unfinished markers remain.
- Type consistency: `open_settings_window` maps to `openSettingsWindow`, settings label is consistently `"settings"`, route group keeps URLs unchanged.
