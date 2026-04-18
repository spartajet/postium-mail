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
    // 导入图标组件
    import {
        Settings,
        Palette,
        Globe,
        Monitor,
        Moon,
        Sun,
        User,
        Plus,
        Trash2,
        ChevronLeft,
    } from "lucide-svelte";
    // 导入路由导航函数
    import { goto } from "$app/navigation";

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);
    // 获取主题状态实例
    const themeStore = getThemeState();
    // 获取账户状态实例
    const accountStore = getAccountState();

    // 页面加载时自动加载账户列表
    // $effect 在组件挂载时执行，并在依赖项变化时重新执行
    $effect(() => {
        accountStore.loadAccounts();
    });
</script>

<div class="flex h-full flex-col overflow-y-auto bg-background">
    <!-- 页面头部：返回按钮、标题图标、标题 -->
    <div class="flex items-center gap-3 border-b border-border px-6 py-4">
        <button
            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            onclick={() => goto("/")}
        >
            <ChevronLeft size={20} />
        </button>
        <Settings size={20} class="text-primary" />
        <h1 class="text-lg font-semibold text-foreground">
            {t.settings.title}
        </h1>
    </div>

    <div class="max-w-2xl space-y-6 p-6">
        <!-- 外观设置区域 -->
        <section class="rounded-xl border border-border bg-card p-5">
            <!-- 区域标题 -->
            <div class="mb-4 flex items-center gap-2">
                <Palette size={18} class="text-primary" />
                <h2 class="text-sm font-semibold text-foreground">
                    {t.settings.appearance}
                </h2>
            </div>

            <div class="space-y-4">
                <!-- 主题切换 -->
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                        <Monitor size={16} class="text-muted-foreground" />
                        <span class="text-sm text-foreground"
                            >{t.settings.theme}</span
                        >
                    </div>
                    <!-- 主题切换按钮组 -->
                    <div
                        class="flex rounded-lg border border-border bg-glass p-0.5"
                    >
                        <!-- 浅色主题按钮 -->
                        <button
                            class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme ===
                            'light'
                                ? 'bg-primary text-primary-foreground'
                                : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => themeStore.setTheme("light")}
                        >
                            <Sun size={12} />
                            {t.settings.light}
                        </button>
                        <!-- 深色主题按钮 -->
                        <button
                            class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme ===
                            'dark'
                                ? 'bg-primary text-primary-foreground'
                                : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => themeStore.setTheme("dark")}
                        >
                            <Moon size={12} />
                            {t.settings.dark}
                        </button>
                        <!-- 跟随系统主题按钮 -->
                        <button
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

                <!-- 语言切换 -->
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                        <Globe size={16} class="text-muted-foreground" />
                        <span class="text-sm text-foreground"
                            >{t.settings.language}</span
                        >
                    </div>
                    <!-- 语言切换按钮组 -->
                    <div
                        class="flex rounded-lg border border-border bg-glass p-0.5"
                    >
                        <!-- 中文按钮 -->
                        <button
                            class="rounded-md px-3 py-1.5 text-xs transition-colors {i18n.locale ===
                            'zh-CN'
                                ? 'bg-primary text-primary-foreground'
                                : 'text-muted-foreground hover:text-foreground'}"
                            onclick={() => i18n.setLocale("zh-CN")}
                        >
                            中文
                        </button>
                        <!-- 英文按钮 -->
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

        <!-- 账户管理区域 -->
        <section class="rounded-xl border border-border bg-card p-5">
            <div class="mb-4 flex items-center justify-between">
                <div class="flex items-center gap-2">
                    <User size={18} class="text-primary" />
                    <h2 class="text-sm font-semibold text-foreground">
                        {t.settings.accounts}
                    </h2>
                </div>
            </div>

            <!-- 账户列表 -->
            <div class="space-y-2">
                {#each accountStore.accounts as account (account.id)}
                    <!-- 账户卡片 -->
                    <div
                        class="flex items-center gap-3 rounded-lg border border-border bg-glass px-4 py-3"
                    >
                        <!-- 账户头像：显示邮箱首字母 -->
                        <div
                            class="flex h-8 w-8 items-center justify-center rounded-full bg-primary/15 text-xs font-bold text-primary"
                        >
                            {account.email.charAt(0).toUpperCase()}
                        </div>
                        <!-- 账户信息：邮箱地址和服务商 -->
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
                        <!-- 删除账户按钮 -->
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

                <!-- 空状态：没有账户时的提示 -->
                {#if accountStore.accounts.length === 0}
                    <div class="py-6 text-center text-sm text-muted-foreground">
                        {t.settings.accounts}
                    </div>
                {/if}
            </div>
        </section>
    </div>
</div>
