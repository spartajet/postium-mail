# Settings Page Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将设置页实现为左侧导航、右侧内容的设置中心布局，并默认显示通用分组。

**Architecture:** 保持 `/settings` 单路由，在 `src/routes/settings/+page.svelte` 内部用局部 `$state` 管理当前分组。现有主题、语言、账号状态管理不变，只调整信息架构、模板结构和响应式布局。

**Tech Stack:** Svelte 5 Runes、SvelteKit、Tailwind CSS v4、lucide-svelte、Vitest、Testing Library Svelte。

## Global Constraints

- 文档使用中文书写。
- 没有用户明确指令，不提交代码。
- shell 命令使用 `rtk` 前缀。
- 保留现有测试依赖的 `data-testid`：`settings-page`、`theme-light`、`theme-dark`、`theme-system`、`settings-account-card`。
- 不新增真实通用设置项。
- 不调整主题、语言、账号删除等业务逻辑。
- 不修改全局布局、侧边栏、状态栏或标题栏。
- 遵循 TDD：先写失败测试，再写实现。

---

## File Structure

- Modify: `src/routes/settings/+page.svelte`
  - 负责设置页全部 UI。
  - 新增内部分组状态、左侧导航、右侧分组渲染。
  - 继续调用现有 `i18n`、`themeStore`、`accountStore`。
- Create: `src/lib/__tests__/routes/settings-page.test.ts`
  - 负责设置页布局行为测试。
  - mock `$app/navigation`、`$lib/stores/i18n.svelte`、`$lib/stores/theme.svelte`、`$lib/stores/account.svelte`。
- No change: `src/lib/i18n/zh-CN.ts`
  - 已有 `settings.general`、`settings.appearance`、`settings.accounts`、`settings.title` 等键。
- No change: `src/lib/i18n/en-US.ts`
  - 已有对应英文键。
- No change: `e2e/pageobjects/settings.page.js`
  - 现有选择器继续有效，不需要修改。

---

### Task 1: Add Settings Page Layout Tests

**Files:**
- Create: `src/lib/__tests__/routes/settings-page.test.ts`

**Interfaces:**
- Consumes: `src/routes/settings/+page.svelte` default component.
- Produces: Tests that require these UI contracts:
  - `data-testid="settings-page"` exists.
  - `data-testid="settings-nav-general"` button exists and is selected by default.
  - `data-testid="settings-panel-general"` is visible by default.
  - Clicking `data-testid="settings-nav-appearance"` shows `theme-light`, `theme-dark`, `theme-system`.
  - Clicking `data-testid="settings-nav-accounts"` shows `data-testid="settings-accounts-panel"`.

- [ ] **Step 1: Write the failing test file**

Create `src/lib/__tests__/routes/settings-page.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "../../../routes/settings/+page.svelte";

const goto = vi.fn();
const setTheme = vi.fn();
const setLocale = vi.fn();
const loadAccounts = vi.fn();
const deleteAccount = vi.fn();

let currentTheme: "light" | "dark" | "system" = "system";
let currentLocale: "zh-CN" | "en-US" = "zh-CN";

vi.mock("$app/navigation", () => ({
    goto,
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        get locale() {
            return currentLocale;
        },
        setLocale,
        t: {
            settings: {
                title: "设置",
                general: "通用",
                accounts: "账号管理",
                appearance: "外观",
                language: "语言",
                theme: "主题",
                light: "浅色",
                dark: "深色",
                system: "跟随系统",
            },
            account: {
                delete: "删除",
            },
        },
    }),
}));

vi.mock("$lib/stores/theme.svelte", () => ({
    getThemeState: () => ({
        get theme() {
            return currentTheme;
        },
        setTheme,
    }),
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts: [],
        error: null,
        loadAccounts,
        deleteAccount,
    }),
}));

describe("Settings page layout", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        currentTheme = "system";
        currentLocale = "zh-CN";
    });

    it("默认显示通用分组", async () => {
        render(SettingsPage);

        await waitFor(() => {
            expect(loadAccounts).toHaveBeenCalledOnce();
        });

        expect(screen.getByTestId("settings-page")).toBeInTheDocument();
        expect(screen.getByTestId("settings-nav-general")).toHaveAttribute(
            "aria-current",
            "page",
        );
        expect(screen.getByTestId("settings-panel-general")).toBeVisible();
        expect(screen.getByText("基础设置")).toBeVisible();
    });

    it("点击外观导航后显示主题和语言设置", async () => {
        render(SettingsPage);

        await fireEvent.click(screen.getByTestId("settings-nav-appearance"));

        expect(screen.getByTestId("theme-light")).toBeVisible();
        expect(screen.getByTestId("theme-dark")).toBeVisible();
        expect(screen.getByTestId("theme-system")).toBeVisible();
        expect(screen.getByText("语言")).toBeVisible();
    });

    it("点击账号管理导航后显示账号管理面板", async () => {
        render(SettingsPage);

        await fireEvent.click(screen.getByTestId("settings-nav-accounts"));

        expect(screen.getByTestId("settings-accounts-panel")).toBeVisible();
        expect(screen.getByText("账号管理")).toBeVisible();
    });
});
```

- [ ] **Step 2: Run the new test to verify it fails**

Run:

```bash
rtk bun node_modules/@sveltejs/kit/svelte-kit.js sync
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- The test run fails.
- The failure is caused by missing `settings-nav-general`, missing `settings-panel-general`, or missing `settings-accounts-panel`.
- If the failure is an import or mock setup error, fix the test setup before implementing production code.

---

### Task 2: Implement Internal Navigation and Panels

**Files:**
- Modify: `src/routes/settings/+page.svelte`

**Interfaces:**
- Consumes: Tests from Task 1.
- Produces:
  - `type SettingsSection = "general" | "appearance" | "accounts"`.
  - Local state `activeSection` initial value `"general"`.
  - Navigation buttons with `data-testid` values:
    - `settings-nav-general`
    - `settings-nav-appearance`
    - `settings-nav-accounts`
  - Panels with `data-testid` values:
    - `settings-panel-general`
    - `settings-panel-appearance`
    - `settings-accounts-panel`

- [ ] **Step 1: Add section state and nav model**

In `src/routes/settings/+page.svelte`, update imports to include icons for the three sections. The existing imports already include `Settings`, `Palette`, `Globe`, `Monitor`, `Moon`, `Sun`, `User`, `Trash2`, and `ChevronLeft`. Remove unused `Plus` if it remains unused, and add `SlidersHorizontal`.

Add this script code after store initialization:

```svelte
    type SettingsSection = "general" | "appearance" | "accounts";

    let activeSection = $state<SettingsSection>("general");

    const settingsSections = $derived([
        {
            id: "general" as const,
            label: t.settings.general,
            icon: SlidersHorizontal,
            testId: "settings-nav-general",
        },
        {
            id: "appearance" as const,
            label: t.settings.appearance,
            icon: Palette,
            testId: "settings-nav-appearance",
        },
        {
            id: "accounts" as const,
            label: t.settings.accounts,
            icon: User,
            testId: "settings-nav-accounts",
        },
    ]);
```

- [ ] **Step 2: Replace top header and vertical cards with settings-center layout shell**

Replace the current top-level markup inside `data-testid="settings-page"` with this shell. This step only creates the structural shell and the general panel; Step 3 and Step 4 move the existing appearance and accounts content into their panels.

```svelte
<div
    data-testid="settings-page"
    class="flex h-full flex-col overflow-hidden bg-background md:flex-row"
>
    <aside
        class="flex shrink-0 flex-col border-b border-border bg-card/80 md:w-60 md:border-r md:border-b-0"
    >
        <div class="flex items-center gap-3 px-5 py-4">
            <button
                class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                onclick={() => goto("/")}
                title={t.settings.title}
            >
                <ChevronLeft size={20} />
            </button>
            <Settings size={20} class="text-primary" />
            <h1 class="text-lg font-semibold text-foreground">
                {t.settings.title}
            </h1>
        </div>

        <nav
            class="flex gap-2 overflow-x-auto px-4 pb-4 md:flex-col md:overflow-visible md:px-3"
            aria-label={t.settings.title}
        >
            {#each settingsSections as section (section.id)}
                {@const SectionIcon = section.icon}
                <button
                    data-testid={section.testId}
                    class="flex min-w-max items-center gap-2 rounded-lg px-3 py-2 text-left text-sm transition-colors md:min-w-0 {activeSection ===
                    section.id
                        ? 'bg-primary text-primary-foreground'
                        : 'text-muted-foreground hover:bg-glass-hover hover:text-foreground'}"
                    aria-current={activeSection === section.id ? "page" : undefined}
                    onclick={() => {
                        activeSection = section.id;
                    }}
                >
                    <SectionIcon size={16} />
                    <span>{section.label}</span>
                </button>
            {/each}
        </nav>
    </aside>

    <main class="min-w-0 flex-1 overflow-y-auto p-5 md:p-6">
        <div class="mx-auto max-w-3xl">
            {#if activeSection === "general"}
                <section
                    data-testid="settings-panel-general"
                    class="rounded-lg border border-border bg-card p-5"
                >
                    <div class="mb-2 flex items-center gap-2">
                        <SlidersHorizontal size={18} class="text-primary" />
                        <h2 class="text-sm font-semibold text-foreground">
                            基础设置
                        </h2>
                    </div>
                    <p class="text-sm text-muted-foreground">
                        管理应用级偏好设置。后续通用选项会放在这里。
                    </p>
                </section>
            {:else if activeSection === "appearance"}
                <section
                    data-testid="settings-panel-appearance"
                    class="rounded-lg border border-border bg-card p-5"
                >
                </section>
            {:else if activeSection === "accounts"}
                <section
                    data-testid="settings-accounts-panel"
                    class="rounded-lg border border-border bg-card p-5"
                >
                </section>
            {/if}
        </div>
    </main>
</div>
```

- [ ] **Step 3: Move the existing appearance settings into `settings-panel-appearance`**

Move the current appearance section content from `src/routes/settings/+page.svelte` into the empty `settings-panel-appearance` section created in Step 2.

The moved content starts with this title block:

```svelte
<div class="mb-4 flex items-center gap-2">
    <Palette size={18} class="text-primary" />
    <h2 class="text-sm font-semibold text-foreground">
        {t.settings.appearance}
    </h2>
</div>
```

It must include the existing theme and language controls, including the three existing theme buttons:

```svelte
<button data-testid="theme-light" ...>
<button data-testid="theme-dark" ...>
<button data-testid="theme-system" ...>
```

Do not change the existing handlers:

```svelte
onclick={() => themeStore.setTheme("light")}
onclick={() => themeStore.setTheme("dark")}
onclick={() => themeStore.setTheme("system")}
onclick={() => i18n.setLocale("zh-CN")}
onclick={() => i18n.setLocale("en-US")}
```

- [ ] **Step 4: Move the existing account settings into `settings-accounts-panel`**

Move the current accounts section content from `src/routes/settings/+page.svelte` into the empty `settings-accounts-panel` section created in Step 2.

The moved content starts with this title block:

```svelte
<div class="mb-4 flex items-center justify-between">
    <div class="flex items-center gap-2">
        <User size={18} class="text-primary" />
        <h2 class="text-sm font-semibold text-foreground">
            {t.settings.accounts}
        </h2>
    </div>
</div>
```

Keep these exact runtime contracts:

- Keep the exact existing `theme-light`, `theme-dark`, `theme-system` buttons.
- Keep the exact existing `settings-account-card` and `data-email={account.email}`.
- Keep the existing delete handler `accountStore.deleteAccount(account.id)`.
- Keep the existing error block `data-testid="account-delete-error"`.

- [ ] **Step 5: Run the focused test to verify it passes**

Run:

```bash
rtk node node_modules/vitest/vitest.mjs run src/lib/__tests__/routes/settings-page.test.ts
```

Expected:

- All tests in `settings-page.test.ts` pass.

---

### Task 3: Validate Svelte and Full Frontend Tests

**Files:**
- Test-only task.

**Interfaces:**
- Consumes: Implemented settings page and tests from Task 1 and Task 2.
- Produces: Verification results.

- [ ] **Step 1: Run Svelte autofixer on the edited component**

Run:

```bash
rtk npx @sveltejs/mcp svelte-autofixer ./src/routes/settings/+page.svelte --svelte-version 5
```

Expected:

- No blocking Svelte syntax issues.
- If it reports a concrete fix, apply the fix with `apply_patch`, then rerun this command.

- [ ] **Step 2: Run project check**

Run:

```bash
rtk bun run check
```

Expected:

- Svelte check completes successfully.

- [ ] **Step 3: Run frontend tests**

Run:

```bash
rtk bun run test:frontend
```

Expected:

- Frontend test suite passes.

- [ ] **Step 4: Inspect git diff**

Run:

```bash
rtk git diff -- src/routes/settings/+page.svelte src/lib/__tests__/routes/settings-page.test.ts docs/superpowers/specs/2026-07-01-settings-page-sidebar-design.md docs/superpowers/plans/2026-07-01-settings-page-sidebar.md
```

Expected:

- Diff only includes the settings page implementation, the new settings page test, and the two docs files.
- No generated build artifacts are included.
- Do not stage or commit unless the user explicitly asks.

---

## Self-Review

- Spec coverage:
  - Three navigation sections are covered in Task 2.
  - Default general section is covered in Task 1 and Task 2.
  - Existing appearance and accounts behavior preservation is covered in Task 2.
  - Existing `data-testid` preservation is covered in Task 1 and Task 2.
  - Responsive layout is covered by Tailwind classes in Task 2.
  - Verification is covered in Task 3.
- Placeholder scan:
  - No TBD/TODO placeholders remain.
- Type consistency:
  - `SettingsSection` values match nav IDs and panel conditionals: `general`, `appearance`, `accounts`.
  - Test IDs match Task 1 expectations and Task 2 implementation.
