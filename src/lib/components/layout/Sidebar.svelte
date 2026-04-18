<!--
  Postium Mail - 侧边栏组件
  Sidebar.svelte

  本组件是应用的主导航侧边栏，负责：
  1. 显示应用 Logo 和同步按钮
  2. 提供账户选择器（支持多账户切换）
  3. 提供邮件文件夹导航（收件箱、星标、已发送、草稿、垃圾邮件、回收站）
  4. 提供标签分类导航（紧急、工作、个人、财务）
  5. 提供日历和工作流快捷入口
  6. 提供设置页面入口
  7. 显示存储空间使用情况

  在架构中的位置：
  - 位于 +layout.svelte 主布局中 TitleBar 下方、主内容区左侧
  - 通过 Context API 获取写邮件模态框和添加账户模态框的引用
  - 通过 Store 管理国际化、账户、邮件、同步等全局状态
  - 点击文件夹时调用 emailStore 加载对应分类的邮件列表
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入 Svelte 核心函数
    import { getContext, onMount } from "svelte";

    // 导入全局状态管理器
    import { getI18nState } from "$lib/stores/i18n.svelte"; // 国际化状态（多语言翻译）
    import { getAccountState } from "$lib/stores/account.svelte"; // 账户状态（账户列表、当前活动账户）
    import { getEmailState } from "$lib/stores/email.svelte"; // 邮件状态（邮件列表、当前文件夹）
    import type { EmailCategory } from "$lib/bindings"; // 邮件分类类型定义（inbox/starred/sent/drafts/spam/trash）
    import { getSyncState } from "$lib/stores/sync.svelte"; // 同步状态（同步进度、错误信息）

    // 导入模态框组件类型（仅用于类型注解，不实际实例化）
    import type ComposeModal from "$lib/components/email/ComposeModal.svelte"; // 写邮件模态框
    import type AddAccountModal from "$lib/components/settings/AddAccountModal.svelte"; // 添加账户模态框

    // 导入图标组件（来自 lucide-svelte 图标库）
    import {
        Inbox, // 收件箱图标
        Star, // 星标图标
        Send, // 发送图标
        FileText, // 文件/草稿图标
        AlertCircle, // 警告/垃圾邮件图标
        Trash2, // 回收站图标
        Plus, // 加号图标（新建/添加）
        RefreshCw, // 刷新图标（同步）
        Settings, // 设置图标
        ChevronDown, // 向下箭头图标（下拉菜单）
        Calendar, // 日历图标
        Workflow, // 工作流图标
    } from "lucide-svelte";

    // 导入 SvelteKit 路由导航函数
    import { goto } from "$app/navigation";

    // ==================== Context 键定义 ====================

    // 使用 Symbol.for 创建全局唯一的 Context 键
    // 这些键在 +layout.svelte 中通过 setContext 设置，这里通过 getContext 获取
    // Symbol.for 确保在不同模块中引用同一个 Symbol
    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal"); // 写邮件模态框的 Context 键
    const ADD_ACCOUNT_MODAL_KEY = Symbol.for("add-account-modal"); // 添加账户模态框的 Context 键

    // 从 Context 中获取模态框引用获取函数
    // 调用返回的函数可以得到模态框组件实例，从而调用 show() 等方法
    const getComposeModal =
        getContext<() => ComposeModal | undefined>(COMPOSE_MODAL_KEY);
    const getAddAccountModal = getContext<() => AddAccountModal | undefined>(
        ADD_ACCOUNT_MODAL_KEY,
    );

    // ==================== 全局状态获取 ====================

    // 获取各全局状态管理器的单例实例
    // 这些状态在 +layout.svelte 中通过 create*State() 创建，这里通过 get*State() 获取
    const i18n = getI18nState(); // 国际化状态
    const t = $derived(i18n.t); // 翻译函数（响应式派生，当语言切换时自动更新）
    const accountStore = getAccountState(); // 账户状态
    const emailStore = getEmailState(); // 邮件状态
    const syncStore = getSyncState(); // 同步状态

    // ==================== 组件内部状态 ====================

    // 当前激活的文件夹 ID
    // 默认为 "inbox"（收件箱），点击文件夹导航项时更新
    let activeFolder = $state("inbox");

    // 账户下拉菜单是否展开
    // 控制账户选择器的显示/隐藏
    let showAccountDropdown = $state(false);

    // ==================== 组件生命周期 ====================

    // onMount：组件挂载到 DOM 后执行
    // 加载账户列表，填充账户选择器
    onMount(() => {
        accountStore.loadAccounts();
    });

    // ==================== 文件夹选择处理 ====================

    // 选择文件夹（邮件分类）
    // 参数 folderId：邮件分类标识（inbox/starred/sent/drafts/spam/trash）
    // 执行以下操作：
    // 1. 更新本地 activeFolder 状态（高亮当前选中项）
    // 2. 更新全局 emailStore 的当前文件夹
    // 3. 加载该分类下的邮件列表
    // 4. 导航到首页（确保显示邮件列表视图）
    function selectFolder(folderId: EmailCategory) {
        activeFolder = folderId;
        emailStore.currentFolder = folderId;
        if (accountStore.activeAccountId) {
            emailStore.loadEmailsByCategory(
                accountStore.activeAccountId,
                folderId,
            );
        }
        goto("/");
    }

    // ==================== 邮件同步处理 ====================

    // 手动触发邮件同步
    // 1. 同步当前活动账户的邮件
    // 2. 同步完成后重新加载当前文件夹的邮件列表
    async function handleSync() {
        if (accountStore.activeAccountId) {
            await syncStore.syncAccount(accountStore.activeAccountId);
            await emailStore.loadEmailsByCategory(
                accountStore.activeAccountId,
                emailStore.currentFolder,
            );
        }
    }

    // ==================== 文件夹配置列表 ====================

    // 邮件文件夹导航项配置
    // $derived：响应式派生，当翻译语言变化时自动更新标签文本
    // 每个文件夹项包含：
    // - id：文件夹唯一标识（对应 EmailCategory 类型）
    // - label：显示名称（国际化文本）
    // - icon：图标组件
    const folders = $derived([
        { id: "inbox" as EmailCategory, label: t.sidebar.inbox, icon: Inbox },
        {
            id: "starred" as EmailCategory,
            label: t.sidebar.starred,
            icon: Star,
        },
        { id: "sent" as EmailCategory, label: t.sidebar.sent, icon: Send },
        {
            id: "drafts" as EmailCategory,
            label: t.sidebar.drafts,
            icon: FileText,
        },
        {
            id: "spam" as EmailCategory,
            label: t.sidebar.spam,
            icon: AlertCircle,
        },
        { id: "trash" as EmailCategory, label: t.sidebar.trash, icon: Trash2 },
    ]);

    // 标签分类列表配置
    // 用于按颜色标签对邮件进行分类筛选
    // 每个标签项包含：
    // - id：标签唯一标识
    // - name：显示名称（国际化文本）
    // - color：标签颜色（十六进制色值）
    const labels = $derived([
        { id: "urgent", name: t.sidebar.labelUrgent, color: "#EF4444" }, // 紧急 - 红色
        { id: "work", name: t.sidebar.labelWork, color: "#3B82F6" }, // 工作 - 蓝色
        { id: "personal", name: t.sidebar.labelPersonal, color: "#10B981" }, // 个人 - 绿色
        { id: "finance", name: t.sidebar.labelFinance, color: "#F59E0B" }, // 财务 - 黄色
    ]);
</script>

<!-- ==================== 侧边栏布局 ==================== -->

<!--
  侧边栏容器：
  使用 <aside> 语义化标签

  类名说明：
  - flex：弹性布局，垂直方向排列子元素
  - h-full：高度占满父容器
  - w-65：固定宽度 260px（65 × 4px）
  - shrink-0：不允许收缩（防止布局挤压时侧边栏变窄）
  - flex-col：垂直方向排列（从上到下）
  - border-r border-border：右侧边框分隔线
  - bg-glass：玻璃态背景色
  - backdrop-blur-md：中等程度的背景模糊（毛玻璃效果）
-->
<aside
    class="flex h-full w-65 shrink-0 flex-col border-r border-border bg-glass backdrop-blur-md"
>
    <!-- ==================== 顶部区域：Logo + 同步按钮 ==================== -->

    <!--
      顶部栏：包含应用 Logo 和手动同步按钮
      使用底部边框与下方内容分隔
    -->
    <div
        class="flex items-center justify-between border-b border-border px-4 py-3"
    >
        <!-- 应用 Logo 组合：图标 + 文字 -->
        <div class="flex items-center gap-2">
            <!-- Logo 图标：圆角方块背景 + 首字母 "P" -->
            <div
                class="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-xs font-bold text-primary-foreground"
            >
                P
            </div>
            <span class="text-sm font-bold text-foreground">Postium</span>
        </div>

        <!-- 手动同步按钮 -->
        <!-- 同步中时图标旋转（animate-spin），按钮禁用防止重复点击 -->
        <button
            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            title={t.sidebar.sync}
            onclick={handleSync}
            disabled={syncStore.syncing}
        >
            <RefreshCw
                size={16}
                class={syncStore.syncing ? "animate-spin" : ""}
            />
        </button>
    </div>

    <!-- ==================== 账户选择器 ==================== -->

    <!--
      账户选择下拉菜单
      点击展开/收起账户列表，支持切换当前活动账户
    -->
    <div class="relative px-3">
        <!-- 账户选择触发按钮：显示当前活动账户邮箱首字母和邮箱地址 -->
        <button
            class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors hover:bg-glass-hover"
            onclick={() => (showAccountDropdown = !showAccountDropdown)}
        >
            <!-- 账户头像：圆形背景 + 邮箱首字母大写 -->
            <div
                class="flex h-6 w-6 items-center justify-center rounded-full bg-primary/15 text-[10px] font-bold text-primary"
            >
                {accountStore.activeAccount?.email?.charAt(0)?.toUpperCase() ||
                    "?"}
            </div>
            <!-- 账户邮箱地址：超长时截断显示 -->
            <span class="flex-1 truncate text-muted-foreground"
                >{accountStore.activeAccount?.email ||
                    t.sidebar.allAccounts}</span
            >
            <!-- 下拉箭头图标 -->
            <ChevronDown size={14} class="shrink-0 text-muted-foreground" />
        </button>

        <!-- 账户下拉菜单（条件渲染：仅在展开时显示） -->
        {#if showAccountDropdown}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!--
              下拉菜单面板：
              - 绝对定位，紧贴触发按钮下方
              - z-50 确保在其他内容之上
              - 点击事件阻止冒泡（防止误关闭）
            -->
            <div
                class="absolute left-3 right-3 top-full z-50 mt-1 rounded-lg border border-border bg-card shadow-lg"
                onclick={(e) => e.stopPropagation()}
            >
                <!-- 账户列表：遍历所有已登录账户 -->
                {#each accountStore.accounts as account}
                    <!--
                      单个账户项：
                      - 当前活动账户高亮显示（bg-primary/10 + text-primary）
                      - 点击切换活动账户并关闭下拉菜单
                    -->
                    <button
                        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-glass-hover {account.id ===
                        accountStore.activeAccountId
                            ? 'bg-primary/10 text-primary'
                            : 'text-foreground'}"
                        onclick={() => {
                            accountStore.setActive(account.id);
                            showAccountDropdown = false;
                        }}
                    >
                        <!-- 账户头像缩略图 -->
                        <div
                            class="flex h-5 w-5 items-center justify-center rounded-full bg-primary/15 text-[9px] font-bold text-primary"
                        >
                            {account.email.charAt(0).toUpperCase()}
                        </div>
                        <span class="truncate">{account.email}</span>
                    </button>
                {/each}

                <!-- 分隔线 -->
                <div class="border-t border-border">
                    <!-- 添加新账户按钮：打开添加账户模态框 -->
                    <button
                        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-primary transition-colors hover:bg-glass-hover"
                        onclick={() => {
                            getAddAccountModal?.()?.show();
                            showAccountDropdown = false;
                        }}
                    >
                        <Plus size={14} />
                        {t.account.add}
                    </button>
                </div>
            </div>
        {/if}
    </div>

    <!-- ==================== 写邮件按钮 ==================== -->

    <!--
      写邮件按钮：
      - 渐变背景（primary → secondary）
      - 紫色阴影效果
      - 悬停时轻微上移（translateY）
      - 点击打开写邮件模态框
    -->
    <div class="px-4 py-3">
        <button
            class="compose-btn flex w-full items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium text-white transition-all"
            onclick={() => getComposeModal?.()?.show()}
        >
            <Plus size={18} />
            {t.sidebar.compose}
        </button>
    </div>

    <!-- ==================== 文件夹导航 ==================== -->

    <!--
      导航区域：
      - flex-1：占据侧边栏剩余空间
      - overflow-y-auto：内容超出时垂直滚动
    -->
    <nav class="flex-1 overflow-y-auto px-3 py-1">
        <!-- 邮件文件夹列表 -->
        <div class="mb-3">
            {#each folders as folder}
                <!--
                  单个文件夹导航项：
                  - active 类名高亮当前选中文件夹
                  - 点击切换文件夹并加载对应邮件
                -->
                <button
                    class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors {activeFolder ===
                    folder.id
                        ? 'active'
                        : ''}"
                    onclick={() => selectFolder(folder.id)}
                >
                    <!-- 文件夹图标：动态渲染对应图标组件 -->
                    <folder.icon size={18} class="shrink-0" />
                    <!-- 文件夹名称（国际化文本） -->
                    <span class="flex-1">{folder.label}</span>
                </button>
            {/each}
        </div>

        <!-- 分隔线：文件夹与标签之间的视觉分隔 -->
        <div class="mx-2 my-3 h-px bg-border"></div>

        <!-- ==================== 标签分类区域 ==================== -->
        <div>
            <!-- 区域标题 -->
            <div
                class="px-3 py-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
            >
                {t.sidebar.labels}
            </div>
            <!-- 标签列表 -->
            {#each labels as label}
                <!--
                  单个标签导航项：
                  - 显示彩色圆点标识
                  - 显示标签名称
                -->
                <button
                    class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-1.5 text-left text-sm text-muted-foreground transition-colors"
                >
                    <!-- 彩色圆点：使用内联样式设置标签颜色 -->
                    <span
                        class="h-2 w-2 shrink-0 rounded-full"
                        style="background: {label.color}"
                    ></span>
                    <span>{label.name}</span>
                </button>
            {/each}
        </div>

        <!-- 分隔线：标签与快捷入口之间的视觉分隔 -->
        <div class="mx-2 my-3 h-px bg-border"></div>

        <!-- ==================== 快捷入口：日历 & 工作流 ==================== -->
        <div class="mb-1">
            <!-- 日历入口：导航到 /calendar 页面 -->
            <button
                class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
                onclick={() => {
                    activeFolder = "";
                    goto("/calendar");
                }}
            >
                <Calendar size={18} class="shrink-0" />
                <span class="flex-1">{t.sidebar.calendar}</span>
            </button>
            <!-- 工作流入口：导航到 /workflow 页面 -->
            <button
                class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
                onclick={() => {
                    activeFolder = "";
                    goto("/workflow");
                }}
            >
                <Workflow size={18} class="shrink-0" />
                <span class="flex-1">{t.sidebar.workflow}</span>
            </button>
        </div>
    </nav>

    <!-- ==================== 设置入口 ==================== -->

    <!--
      设置按钮：固定在侧边栏底部
      点击导航到 /settings 页面
    -->
    <div class="border-t border-border px-3 py-2">
        <button
            class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
            onclick={() => {
                activeFolder = "";
                goto("/settings");
            }}
        >
            <Settings size={18} class="shrink-0" />
            {t.sidebar.settings}
        </button>
    </div>

    <!-- ==================== 存储空间指示器 ==================== -->

    <!--
      存储空间使用情况：
      - 显示已用空间 / 总空间
      - 进度条可视化使用比例
      - 固定在侧边栏最底部
    -->
    <div class="border-t border-border p-4">
        <div class="mb-2 flex items-center justify-between">
            <span class="text-xs text-muted-foreground">Storage</span>
            <span class="text-xs text-muted-foreground">2.4 GB / 15 GB</span>
        </div>
        <!-- 进度条背景 -->
        <div class="h-1 overflow-hidden rounded-full bg-glass-active">
            <!-- 进度条填充：当前使用量 16% -->
            <div class="storage-fill h-full w-[16%] rounded-full"></div>
        </div>
    </div>
</aside>

<style>
    /* ==================== 写邮件按钮样式 ==================== */

    /*
      .compose-btn：写邮件按钮样式
      - 使用主色到次要色的渐变背景（135度斜角方向）
      - 紫色发光阴影效果，提升视觉层次感
    */
    .compose-btn {
        background: linear-gradient(
            135deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
        box-shadow: 0 4px 16px rgba(124, 58, 237, 0.3);
    }

    /* 悬停效果：按钮轻微上移，阴影增强 */
    .compose-btn:hover {
        transform: translateY(-1px);
        box-shadow: 0 6px 20px rgba(124, 58, 237, 0.4);
    }

    /* 按下效果：按钮恢复原位 */
    .compose-btn:active {
        transform: translateY(0);
    }

    /* ==================== 导航项样式 ==================== */

    /*
      .nav-item：侧边栏导航项通用样式
      - 悬停时显示玻璃态背景
      - 文字颜色变亮
    */
    .nav-item:hover {
        background: var(--color-glass-hover);
        color: var(--color-foreground);
    }

    /*
      .nav-item.active：当前选中导航项的高亮样式
      - 主色半透明背景（15% 不透明度混合）
      - 主色文字
    */
    .nav-item.active {
        background: color-mix(in srgb, var(--color-primary) 15%, transparent);
        color: var(--color-primary);
    }

    /* ==================== 存储空间进度条样式 ==================== */

    /*
      .storage-fill：存储进度条填充部分
      - 使用主色到次要色的水平渐变
      - 宽度变化时带有 300ms 的过渡动画
    */
    .storage-fill {
        background: linear-gradient(
            90deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
        transition: width 300ms ease;
    }
</style>
