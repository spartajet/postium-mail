<!--
  Postium Mail - 设置页面组件
  +page.svelte

  本组件是应用的设置页面，提供应用的全局配置选项。

  ==================== 功能说明 ====================
  1. 外观设置：主题切换（浅色/深色/跟随系统）
  2. 语言设置：中英文切换
  3. 账户管理：查看和管理已添加的邮箱账户
  4. 提供返回主页的导航

  ==================== 组件状态 ====================
  - i18n: 国际化状态管理
  - themeStore: 主题状态管理
  - accountStore: 账户状态管理

  ==================== 交互说明 ====================
  - 点击主题按钮：切换应用主题
  - 点击语言按钮：切换界面语言
  - 点击删除按钮：删除对应的邮箱账户
  - 点击返回按钮：返回主页

  ==================== 技术实现 ====================
  使用 Svelte 5 的 Runes API：
  - $derived: 定义派生状态
  - $effect: 自动执行副作用（如加载数据）
-->
<script lang="ts">
    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入主题状态管理，用于主题切换
    import { getThemeState } from "$lib/stores/theme.svelte";
    // 导入账户状态管理，用于账户管理
    import { getAccountState } from "$lib/stores/account.svelte";
    import TitleBar from "$lib/components/layout/TitleBar.svelte";
    import { goto } from "$app/navigation";
    // 导入图标组件
    import {
        Settings,
        Palette,
        Globe,
        Monitor,
        Moon,
        Sun,
        User,
        SlidersHorizontal,
        Trash2,
    } from "lucide-svelte";

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);
    // 获取主题状态实例
    const themeStore = getThemeState();
    // 获取账户状态实例
    const accountStore = getAccountState();

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

    // 页面加载时自动加载账户列表
    // $effect 在组件挂载时执行，并在依赖项变化时重新执行
    $effect(() => {
        accountStore.loadAccounts();
    });

</script>

<div
    data-testid="settings-page"
    class="flex h-screen flex-col overflow-hidden bg-background text-foreground"
>
    <TitleBar
        title="Postium Mail Settings"
        closeBehavior="close"
        onCloseError={() => goto("/")}
    />

    <div class="flex min-h-0 flex-1 flex-col overflow-hidden md:flex-row">
        <aside
            class="flex shrink-0 flex-col border-b border-border bg-card/80 md:w-60 md:border-b-0 md:border-r"
        >
            <div class="flex items-center gap-3 px-5 py-4">
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
                            {t.settings.generalTitle}
                        </h2>
                    </div>
                    <p class="text-sm text-muted-foreground">
                        {t.settings.generalDescription}
                    </p>
                </section>
            {:else if activeSection === "appearance"}
                <section
                    data-testid="settings-panel-appearance"
                    class="rounded-lg border border-border bg-card p-5"
                >
                    <div class="mb-4 flex items-center gap-2">
                        <Palette size={18} class="text-primary" />
                        <h2 class="text-sm font-semibold text-foreground">
                            {t.settings.appearance}
                        </h2>
                    </div>

                    <div class="space-y-4">
                        <div class="flex items-center justify-between">
                            <div class="flex items-center gap-2">
                                <Monitor size={16} class="text-muted-foreground" />
                                <span class="text-sm text-foreground"
                                    >{t.settings.theme}</span
                                >
                            </div>
                            <div
                                class="flex rounded-lg border border-border bg-glass p-0.5"
                            >
                                <button
                                    data-testid="theme-light"
                                    class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme ===
                                    'light'
                                        ? 'bg-primary text-primary-foreground'
                                        : 'text-muted-foreground hover:text-foreground'}"
                                    onclick={() => themeStore.setTheme("light")}
                                >
                                    <Sun size={12} />
                                    {t.settings.light}
                                </button>
                                <button
                                    data-testid="theme-dark"
                                    class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme ===
                                    'dark'
                                        ? 'bg-primary text-primary-foreground'
                                        : 'text-muted-foreground hover:text-foreground'}"
                                    onclick={() => themeStore.setTheme("dark")}
                                >
                                    <Moon size={12} />
                                    {t.settings.dark}
                                </button>
                                <button
                                    data-testid="theme-system"
                                    class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme ===
                                    'system'
                                        ? 'bg-primary text-primary-foreground'
                                        : 'text-muted-foreground hover:text-foreground'}"
                                    onclick={() => themeStore.setTheme("system")}
                                >
                                    <Monitor size={12} />
                                    {t.settings.system}
                                </button>
                            </div>
                        </div>

                        <div class="flex items-center justify-between">
                            <div class="flex items-center gap-2">
                                <Globe size={16} class="text-muted-foreground" />
                                <span class="text-sm text-foreground"
                                    >{t.settings.language}</span
                                >
                            </div>
                            <div
                                class="flex rounded-lg border border-border bg-glass p-0.5"
                            >
                                <button
                                    class="rounded-md px-3 py-1.5 text-xs transition-colors {i18n.locale ===
                                    'zh-CN'
                                        ? 'bg-primary text-primary-foreground'
                                        : 'text-muted-foreground hover:text-foreground'}"
                                    onclick={() => i18n.setLocale("zh-CN")}
                                >
                                    中文
                                </button>
                                <button
                                    class="rounded-md px-3 py-1.5 text-xs transition-colors {i18n.locale ===
                                    'en-US'
                                        ? 'bg-primary text-primary-foreground'
                                        : 'text-muted-foreground hover:text-foreground'}"
                                    onclick={() => i18n.setLocale("en-US")}
                                >
                                    English
                                </button>
                            </div>
                        </div>
                    </div>
                </section>
            {:else if activeSection === "accounts"}
                <section
                    data-testid="settings-accounts-panel"
                    class="rounded-lg border border-border bg-card p-5"
                >
                    <div class="mb-4 flex items-center justify-between">
                        <div class="flex items-center gap-2">
                            <User size={18} class="text-primary" />
                            <h2 class="text-sm font-semibold text-foreground">
                                {t.settings.accounts}
                            </h2>
                        </div>
                    </div>

                    {#if accountStore.error}
                        <div
                            data-testid="account-delete-error"
                            class="mb-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive"
                        >
                            {accountStore.error}
                        </div>
                    {/if}

                    <div class="space-y-2">
                        {#each accountStore.accounts as account (account.id)}
                            <div
                                data-testid="settings-account-card"
                                data-email={account.email}
                                class="flex items-center gap-3 rounded-lg border border-border bg-glass px-4 py-3"
                            >
                                <div
                                    class="flex h-8 w-8 items-center justify-center rounded-full bg-primary/15 text-xs font-bold text-primary"
                                >
                                    {account.email.charAt(0).toUpperCase()}
                                </div>
                                <div class="min-w-0 flex-1">
                                    <div
                                        class="truncate text-sm font-medium text-foreground"
                                    >
                                        {account.email}
                                    </div>
                                    <div class="text-xs text-muted-foreground">
                                        {account.provider}
                                    </div>
                                </div>
                                <button
                                    class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                                    onclick={() =>
                                        accountStore.deleteAccount(account.id)}
                                    title={t.account.delete}
                                >
                                    <Trash2 size={14} />
                                </button>
                            </div>
                        {/each}

                        {#if accountStore.accounts.length === 0}
                            <div class="py-6 text-center text-sm text-muted-foreground">
                                {t.settings.accounts}
                            </div>
                        {/if}
                    </div>
                </section>
            {/if}
            </div>
        </main>
    </div>
</div>
