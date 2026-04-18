<!--
  Postium Mail - 状态栏组件
  StatusBar.svelte

  本组件是应用底部的状态信息栏，负责：
  1. 显示邮件同步状态（同步中、同步成功、同步失败）
  2. 显示同步进度详情（连接中、同步文件夹、同步邮件及进度百分比）
  3. 显示当前日期
  4. 提供语言切换按钮（中文/英文）
  5. 提供主题切换按钮（亮色/暗色）
  6. 显示应用版本号

  在架构中的位置：
  - 位于 +layout.svelte 主布局的最底部
  - 通过 Store 获取国际化、主题、同步、账户等全局状态
  - 使用 $derived 派生状态实现同步状态的自动计算
  - 同步状态信息支持国际化多语言显示
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入全局状态管理器
    import { getI18nState } from "$lib/stores/i18n.svelte"; // 国际化状态（多语言翻译）
    import { getThemeState } from "$lib/stores/theme.svelte"; // 主题状态（亮色/暗色切换）
    import { getSyncState } from "$lib/stores/sync.svelte"; // 同步状态（同步进度、错误信息）
    import { getAccountState } from "$lib/stores/account.svelte"; // 账户状态（账户列表）

    // 导入图标组件（来自 lucide-svelte 图标库）
    import {
        RefreshCw, // 刷新图标（同步中旋转动画）
        CircleX, // 圆形叉号图标（同步失败）
        CircleCheck, // 圆形勾号图标（同步成功）
        Globe, // 地球图标（语言切换）
        Sun, // 太阳图标（切换到亮色主题）
        Moon, // 月亮图标（切换到暗色主题）
    } from "lucide-svelte";

    // ==================== 全局状态获取 ====================

    // 获取各全局状态管理器的单例实例
    // 这些状态在 +layout.svelte 中通过 create*State() 创建，这里通过 get*State() 获取
    const i18n = getI18nState(); // 国际化状态
    const t = $derived(i18n.t); // 翻译函数（响应式派生，当语言切换时自动更新）
    const themeState = getThemeState(); // 主题状态
    const syncStore = getSyncState(); // 同步状态
    const accountStore = getAccountState(); // 账户状态

    // ==================== 响应式派生状态 ====================

    // 同步状态摘要：从 syncStore 派生当前同步状态
    // 返回三种状态之一：
    // - "syncing"：正在同步中
    // - "error"：同步出错
    // - "idle"：空闲（同步完成或未开始）
    const syncStatus = $derived<"idle" | "syncing" | "error">(
        syncStore.syncing ? "syncing" : syncStore.error ? "error" : "idle",
    );

    // ==================== 辅助函数 ====================

    // 根据账户 ID 获取账户邮箱地址
    // 参数 accountId：账户唯一标识
    // 返回值：账户邮箱地址字符串，未找到则返回空字符串
    function getAccountEmail(accountId: number): string {
        return (
            accountStore.accounts.find((a) => a.id === accountId)?.email ?? ""
        );
    }

    // 获取同步进度的描述文本
    // 根据同步阶段（stage）生成对应的状态描述：
    // - Connecting：正在连接服务器
    // - SyncingFolders：正在同步文件夹列表
    // - SyncingEmails：正在同步邮件（包含文件夹名和进度数字）
    // - Completed：同步完成
    // - Error：同步出错（显示错误信息）
    // 文本格式：[账户邮箱] - [阶段描述] [详细信息]
    function getProgressText(): string {
        if (!syncStore.progress) return "";
        const p = syncStore.progress;
        // 获取同步账户的邮箱地址，用于在进度文本前添加标识
        const email = getAccountEmail(p.account_id);
        const prefix = email ? `${email} - ` : "";

        switch (p.stage) {
            case "Connecting":
                // 正在建立与服务器的连接
                return `${prefix}${t.sync.connecting}`;
            case "SyncingFolders":
                // 正在获取文件夹列表
                return `${prefix}${t.sync.syncingFolders}`;
            case "SyncingEmails": {
                // 正在同步邮件，显示文件夹名和进度（如 "收件箱 (15/100)"）
                const folder = p.folder ?? "";
                const progress =
                    p.total > 0 ? ` (${p.current}/${p.total})` : "";
                return `${prefix}${t.sync.syncingEmails} ${folder}${progress}`;
            }
            case "Completed":
                // 同步完成，显示完成信息或默认文本
                return p.message || t.sync.completed;
            case "Error":
                // 同步出错，显示错误信息
                return p.message;
            default:
                return "";
        }
    }

    // 当前日期：响应式派生
    // 使用浏览器本地化格式显示（如 "Wed, Jan 15"）
    // 显示星期几、月份和日期
    const currentTime = $derived(
        new Date().toLocaleDateString(undefined, {
            weekday: "short",
            month: "short",
            day: "numeric",
        }),
    );

    // ==================== 用户交互函数 ====================

    // 切换主题：在亮色和暗色之间切换
    // 根据 resolved（实际生效的主题）判断当前主题，切换到相反主题
    function toggleTheme() {
        themeState.setTheme(themeState.resolved === "dark" ? "light" : "dark");
    }

    // 切换语言：在中文和英文之间切换
    // 当前为中文时切换到英文，反之亦然
    function toggleLocale() {
        i18n.setLocale(i18n.locale === "zh-CN" ? "en-US" : "zh-CN");
    }
</script>

<!-- ==================== 状态栏布局 ==================== -->

<!--
  状态栏容器：
  固定高度 32px（h-8），位于应用窗口最底部

  类名说明：
  - relative：相对定位
  - flex：弹性布局，水平排列子元素
  - h-8：固定高度 32px
  - items-center：垂直居中对齐
  - justify-between：两端对齐（左侧同步状态，右侧工具栏）
  - border-t border-border：顶部边框分隔线
  - bg-elevated/80：半透明提升背景色
  - px-6：左右内边距 24px
  - text-xs：小号字体（12px）
  - text-muted-foreground：柔和文字颜色
  - backdrop-blur-md：中等程度的背景模糊（毛玻璃效果）
-->
<div
    class="status-bar relative flex h-8 items-center justify-between border-t border-border bg-elevated/80 px-6 text-xs text-muted-foreground backdrop-blur-md"
>
    <!-- ==================== 左侧：同步状态显示 ==================== -->

    <div class="flex items-center gap-6">
        <!-- 同步中状态：显示旋转的刷新图标和进度文本 -->
        {#if syncStatus === "syncing"}
            <div class="flex items-center gap-1.5">
                <!-- 刷新图标：同步中时持续旋转（animate-spin） -->
                <RefreshCw size={14} class="animate-spin text-primary" />
                <!-- 进度描述文本（如 "user@example.com - 正在同步邮件 收件箱 (15/100)"） -->
                <span>{getProgressText() || t.sync.syncing}</span>
            </div>

            <!-- 同步失败状态：显示红色叉号图标和错误信息 -->
        {:else if syncStatus === "error"}
            <div class="flex items-center gap-1.5">
                <!-- 错误图标：红色 -->
                <CircleX size={14} class="text-destructive" />
                <!-- 错误信息文本：红色高亮显示 -->
                <span class="text-destructive"
                    >{syncStore.error || t.sync.failed}</span
                >
            </div>

            <!-- 空闲状态（同步完成）：显示绿色勾号图标 -->
        {:else}
            <div class="flex items-center gap-1.5">
                <!-- 成功图标：绿色 -->
                <CircleCheck size={14} class="text-success" />
                <!-- 完成信息文本 -->
                <span>{syncStore.progress?.message || t.sync.completed}</span>
            </div>
        {/if}
    </div>

    <!-- ==================== 右侧：工具栏 ==================== -->

    <div class="flex items-center gap-5">
        <!-- 当前日期显示 -->
        <span>{currentTime}</span>

        <!-- 语言切换按钮 -->
        <!--
          点击在中英文之间切换
          显示当前非活跃语言的提示文字：
          - 当前中文时显示 "EN"（提示可切换到英文）
          - 当前英文时显示 "中"（提示可切换到中文）
        -->
        <button
            class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-glass-hover hover:text-foreground"
            onclick={toggleLocale}
            title="Toggle language"
        >
            <Globe size={14} />
            <span>{i18n.locale === "zh-CN" ? "EN" : "中"}</span>
        </button>

        <!-- 主题切换按钮 -->
        <!--
          点击在亮色/暗色主题之间切换
          图标显示当前非活跃主题的提示：
          - 当前暗色主题时显示太阳图标（提示可切换到亮色）
          - 当前亮色主题时显示月亮图标（提示可切换到暗色）
        -->
        <button
            class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-glass-hover hover:text-foreground"
            onclick={toggleTheme}
            title="Toggle theme"
        >
            {#if themeState.resolved === "dark"}
                <Sun size={14} />
            {:else}
                <Moon size={14} />
            {/if}
        </button>

        <!-- 应用版本号 -->
        <span class="text-[11px]">v0.1.0</span>
    </div>
</div>
