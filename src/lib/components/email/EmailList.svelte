<!--
  Postium Mail - 邮件列表组件
  EmailList.svelte

  本组件是邮件列表视图，作为应用主页面左侧面板的核心组件。

  ==================== 功能说明 ====================
  1. 显示当前文件夹中的邮件列表
  2. 支持关键词搜索邮件（带防抖功能）
  3. 支持邮件选择，选中后通知全局状态
  4. 支持右键菜单操作（标星、已读/未读、删除等）
  5. 显示邮件基本信息：发件人、主题、预览文本、时间

  ==================== 在架构中的位置 ====================
  位于 src/lib/components/email/ 目录下，是邮件功能模块的子组件。
  被引用于：
    - +page.svelte（主页面的左侧面板）

  依赖的 Store：
    - emailStore：管理邮件数据和选中状态
    - accountStore：获取当前活跃账户信息
    - i18nStore：国际化翻译

  依赖的子组件：
    - EmailContextMenu：右键上下文菜单

  ==================== 数据流说明 ====================
  1. 用户选择文件夹 → accountStore.activeAccountId + emailState.currentFolder 变化
  2. $effect 自动触发 → 调用 emailState.loadEmailsByCategory() 加载邮件
  3. 邮件列表渲染 → 用户点击邮件 → emailState.selectEmail() 更新选中状态
  4. EmailDetail 组件监听 emailState.selectedEmailId 变化并显示详情
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入邮件状态管理，用于邮件列表数据、选中状态、加载操作等
    import { getEmailState } from "$lib/stores/email.svelte";
    // 导入账户状态管理，用于获取当前活跃账户 ID
    import { getAccountState } from "$lib/stores/account.svelte";
    // 导入 Tauri 后端命令接口，用于调用搜索等后端方法
    import { commands } from "$lib/bindings";
    // 导入搜索结果类型定义
    import type { SearchResult } from "$lib/bindings";
    // 导入右键菜单子组件
    import EmailContextMenu from "./EmailContextMenu.svelte";
    // 导入图标组件（Lucide 图标库）
    import {
        Search, // 搜索图标
        RefreshCw, // 刷新/同步图标
        X, // 关闭/清除图标
        List, // 列表视图图标
        LayoutGrid, // 网格视图图标
        Mail, // 邮件图标（空状态占位）
        Star, // 星标图标
    } from "lucide-svelte";

    // ==================== 状态初始化 ====================

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);
    // 获取邮件状态实例（包含邮件列表、选中状态等）
    const emailState = getEmailState();
    // 获取账户状态实例（包含当前活跃账户信息）
    const accountStore = getAccountState();

    // ==================== 搜索相关状态 ====================

    // 搜索关键词，绑定到搜索输入框
    let searchQuery = $state("");
    // 搜索结果列表，null 表示未在搜索模式
    let searchResults = $state<SearchResult[] | null>(null);
    // 搜索加载状态标志
    let searching = $state(false);

    // ==================== 右键菜单状态 ====================

    // 右键菜单的完整状态对象
    // 包含菜单的显示/隐藏、位置坐标、目标邮件的属性
    let contextMenu = $state<{
        visible: boolean; // 菜单是否可见
        x: number; // 菜单出现的 X 坐标（像素）
        y: number; // 菜单出现的 Y 坐标（像素）
        emailId: number; // 目标邮件的 ID
        isRead: boolean; // 目标邮件的已读状态
        isStarred: boolean; // 目标邮件的星标状态
    }>({
        visible: false,
        x: 0,
        y: 0,
        emailId: 0,
        isRead: false,
        isStarred: false,
    });

    /**
     * 打开右键上下文菜单
     *
     * 当用户在邮件项上右键点击时触发，记录鼠标位置和邮件信息，
     * 然后将状态传递给 EmailContextMenu 子组件渲染菜单。
     *
     * @param e - 鼠标事件对象，用于获取点击坐标
     * @param email - 目标邮件对象，包含 id、已读状态、星标状态
     */
    function openContextMenu(
        e: MouseEvent,
        email: { id: number; is_read: boolean; is_starred: boolean },
    ) {
        // 阻止浏览器默认的右键菜单
        e.preventDefault();
        // 更新右键菜单状态
        contextMenu = {
            visible: true,
            x: e.clientX, // 鼠标在视口中的 X 坐标
            y: e.clientY, // 鼠标在视口中的 Y 坐标
            emailId: email.id,
            isRead: email.is_read ?? false,
            isStarred: email.is_starred ?? false,
        };
    }

    /**
     * 关闭右键上下文菜单
     *
     * 将 visible 设为 false，菜单将不再渲染。
     * 由子组件 EmailContextMenu 的 onClose 回调触发。
     */
    function closeContextMenu() {
        contextMenu.visible = false;
    }

    // ==================== 自动加载邮件 ====================

    // 当活跃账号或当前文件夹变化时，自动重新加载邮件列表
    // 这是 Svelte 5 的 $effect rune，会自动追踪依赖并响应变化
    $effect(() => {
        // 追踪依赖：当 activeAccountId 或 currentFolder 变化时重新执行
        const accountId = accountStore.activeAccountId;
        const folder = emailState.currentFolder;
        // 确保有活跃账户才加载
        if (accountId) {
            emailState.loadEmailsByCategory(accountId, folder);
        }
    });

    // ==================== 搜索防抖 Effect ====================

    // 搜索防抖逻辑 - H-02: 正确清理 timeout
    // 当 searchQuery 变化时，等待 300ms 后才发送搜索请求
    // 如果在 300ms 内用户继续输入，取消前一个定时器，重新计时
    // effect 返回的清理函数会在下次 effect 执行前被调用，确保旧 timeout 被清除
    $effect(() => {
        // 追踪依赖：当 searchQuery 变化时重新执行
        const q = searchQuery;

        // 空查询：退出搜索模式，恢复原始邮件列表
        if (!q.trim()) {
            searchResults = null;
            searching = false;
            return;
        }

        // 进入搜索加载状态
        searching = true;
        // 设置 300ms 防抖定时器
        const timeout = setTimeout(async () => {
            try {
                // 获取当前活跃账户 ID
                const accountId = accountStore.activeAccountId;
                // 调用后端搜索命令
                const result = await commands.searchEmails(q, accountId, 50);
                // 搜索成功：更新搜索结果
                if (result.status === "ok") {
                    searchResults = result.data;
                }
            } catch (e) {
                console.error("Search failed:", e);
            } finally {
                // 无论成功或失败，都结束加载状态
                searching = false;
            }
        }, 300); // 300ms 防抖延迟

        // 清理函数：组件仍在时，取消未执行的定时器
        // 防止多次快速输入产生多个并发请求
        return () => clearTimeout(timeout);
    });

    /**
     * 清除搜索状态
     *
     * 重置搜索关键词和搜索结果，恢复到正常邮件列表模式。
     * 绑定到搜索框右侧的 X 按钮。
     */
    function clearSearch() {
        searchQuery = "";
        searchResults = null;
    }

    /**
     * 格式化邮件时间为简短字符串
     *
     * 根据时间距离现在的长短，返回不同格式的字符串：
     * - 不到 1 分钟："now"
     * - 不到 1 小时："Xm"（如 "5m"）
     * - 不到 24 小时："Xh"（如 "3h"）
     * - 不到 7 天："Xd"（如 "2d"）
     * - 超过 7 天：月日格式（如 "Jan 15"）
     *
     * @param timestamp - Unix 时间戳（秒）
     * @returns 格式化后的时间字符串
     */
    function formatDate(timestamp: number): string {
        // 将 Unix 时间戳（秒）转换为 JavaScript Date 对象（毫秒）
        const date = new Date(timestamp * 1000);
        const now = new Date();
        // 计算时间差（毫秒）
        const diffMs = now.getTime() - date.getTime();
        // 转换为不同时间单位
        const diffMins = Math.floor(diffMs / 60000); // 分钟
        const diffHours = Math.floor(diffMs / 3600000); // 小时
        const diffDays = Math.floor(diffMs / 86400000); // 天

        // 根据时间差返回相应格式
        if (diffMins < 1) return "now"; // 刚刚
        if (diffMins < 60) return `${diffMins}m`; // X 分钟前
        if (diffHours < 24) return `${diffHours}h`; // X 小时前
        if (diffDays < 7) return `${diffDays}d`; // X 天前
        // 超过 7 天，显示月日（本地化格式）
        return date.toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
        });
    }
</script>

<!-- ==================== 模板渲染 ====================-->

<!--
  邮件列表面板容器

  类名说明：
  - list-panel：自定义面板样式
  - flex / flex-col：垂直方向弹性布局
  - h-full：占满父容器高度
  - w-95：固定宽度 24rem（约 380px）
  - shrink-0：不允许收缩，保持固定宽度
  - border-r border-border：右侧边框分隔线
  - bg-elevated/80：半透明的高亮背景色（玻璃态效果）
-->
<div
    class="list-panel flex h-full w-95 shrink-0 flex-col border-r border-border bg-elevated/80"
>
    <!-- ==================== 搜索栏 + 工具栏 ==================== -->

    <div class="border-b border-border px-4 py-3">
        <!--
          搜索输入框容器

          类名说明：
          - search-container：自定义搜索容器样式
          - flex / items-center / gap-2：水平排列，居中对齐，间距
          - rounded-lg：圆角
          - border border-border：边框
          - bg-glass：玻璃态背景
          - focus-within:border-primary：聚焦时边框变为主题色
        -->
        <div
            class="search-container flex items-center gap-2 rounded-lg border border-border bg-glass px-3 py-2 transition-colors focus-within:border-primary"
        >
            <!-- 搜索图标 -->
            <Search size={18} class="shrink-0 text-muted-foreground" />

            <!--
              搜索输入框
              - bind:value 双向绑定到 searchQuery
              - placeholder 使用国际化文本
            -->
            <input
                type="text"
                placeholder={t.email.search}
                bind:value={searchQuery}
                class="flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
            />

            <!--
              搜索状态指示器：
              - 搜索中：显示旋转的刷新图标
              - 有输入但不在搜索：显示清除按钮（X）
            -->
            {#if searching}
                <RefreshCw
                    size={16}
                    class="animate-spin text-muted-foreground"
                />
            {:else if searchQuery}
                <button
                    class="text-muted-foreground hover:text-foreground"
                    onclick={clearSearch}
                >
                    <X size={16} />
                </button>
            {/if}

            <!-- 键盘快捷键提示：Ctrl+K 聚焦搜索 -->
            <kbd
                class="rounded bg-glass-active px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
                >Ctrl+K</kbd
            >
        </div>

        <!--
          工具栏：视图切换和同步按钮
          - 刷新按钮：触发邮件同步
          - 列表/网格视图切换按钮（功能占位）
          - 邮件数量统计
        -->
        <div class="mt-3 flex items-center gap-1">
            <!-- 同步/刷新按钮 -->
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.sidebar.sync}
            >
                <RefreshCw size={18} />
            </button>
            <!-- 列表视图按钮 -->
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            >
                <List size={18} />
            </button>
            <!-- 网格视图按钮 -->
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            >
                <LayoutGrid size={18} />
            </button>
            <!--
              邮件数量统计
              - 搜索模式：显示搜索结果数量
              - 正常模式：显示当前文件夹邮件数量
            -->
            <span class="ml-auto text-xs text-muted-foreground">
                {searchResults !== null
                    ? searchResults.length
                    : emailState.emails.length}
            </span>
        </div>
    </div>

    <!-- ==================== 邮件列表区域 ==================== -->

    <!-- 可滚动区域：包含邮件列表或各种空/加载状态 -->
    <div class="flex-1 overflow-y-auto">
        {#if searchResults !== null}
            <!-- ========== 搜索结果模式 ========== -->

            {#if searchResults.length === 0}
                <!--
                  搜索无结果状态
                  显示搜索图标和"无邮件"提示文本
                -->
                <div
                    class="flex h-full flex-col items-center justify-center py-12 text-muted-foreground"
                >
                    <Search size={48} class="mb-4 opacity-30" />
                    <p class="text-sm">{t.email.noEmails}</p>
                </div>
            {:else}
                <!--
                  搜索结果列表
                  遍历 searchResults 数组渲染每一项搜索结果
                  (result.id) 作为 Svelte 的 key 优化列表渲染
                -->
                {#each searchResults as result (result.id)}
                    <!--
                      单个搜索结果项
                      - 点击：选中该邮件
                      - 右键：打开上下文菜单
                      - 根据是否选中添加 active 样式类
                    -->
                    <button
                        class="email-item group relative flex w-full flex-col border-b border-border px-5 py-3 text-left transition-colors {emailState.selectedEmailId ===
                        result.id
                            ? 'active'
                            : ''}"
                        onclick={() => emailState.selectEmail(result.id)}
                        oncontextmenu={(e) =>
                            openContextMenu(e, {
                                id: result.id,
                                is_read: true,
                                is_starred: false,
                            })}
                    >
                        <!-- 第一行：发件人 + 时间 -->
                        <div class="flex items-center justify-between gap-2">
                            <span
                                class="truncate text-sm font-medium text-foreground"
                            >
                                {result.sender_email}
                            </span>
                            <span
                                class="shrink-0 text-xs text-muted-foreground"
                            >
                                {formatDate(result.sent_at)}
                            </span>
                        </div>
                        <!-- 第二行：邮件主题 -->
                        <span
                            class="mt-0.5 truncate text-sm text-muted-foreground"
                        >
                            {result.subject || "(No Subject)"}
                        </span>
                        <!-- 第三行：预览文本（如有） -->
                        {#if result.preview}
                            <p
                                class="mt-0.5 truncate text-xs text-muted-foreground"
                            >
                                {result.preview}
                            </p>
                        {/if}
                    </button>
                {/each}
            {/if}
        {:else if emailState.loading}
            <!--
              加载中状态
              显示骨架屏（Skeleton），模拟邮件列表的加载外观
              使用不同宽度的横条模拟文本内容的视觉占位
            -->
            <div class="space-y-3 p-4">
                <div class="skeleton skeleton-text" style="width: 80%"></div>
                <div class="skeleton skeleton-text" style="width: 60%"></div>
                <div class="skeleton skeleton-text" style="width: 90%"></div>
                <div class="skeleton skeleton-text" style="width: 70%"></div>
                <div class="skeleton skeleton-text" style="width: 50%"></div>
                <div class="skeleton skeleton-text" style="width: 85%"></div>
            </div>
        {:else if emailState.emails.length === 0}
            <!--
              空文件夹状态
              当前文件夹没有任何邮件时显示
              展示邮件图标和空状态提示文本
            -->
            <div
                class="flex h-full flex-col items-center justify-center py-12 text-muted-foreground"
            >
                <Mail size={48} class="mb-4 opacity-30" strokeWidth={1} />
                <h3 class="text-lg text-secondary-foreground/70">
                    {t.email.noEmails}
                </h3>
                <p class="text-sm">{t.email.noEmails}</p>
            </div>
        {:else}
            <!--
              ========== 正常邮件列表模式 ==========
              遍历 emailState.emails 数组渲染每一封邮件
              (email.id) 作为 Svelte 的 key 优化列表渲染性能
            -->
            {#each emailState.emails as email (email.id)}
                <!--
                  单个邮件项

                  类名说明：
                  - email-item：自定义邮件项基础样式
                  - group：Tailwind 的组标记，用于 hover 子元素联动
                  - relative：相对定位（为未读圆点的绝对定位提供参考）
                  - active：选中状态（通过条件判断动态添加）

                  交互事件：
                  - onclick：选中该邮件，更新全局状态
                  - oncontextmenu：打开右键上下文菜单
                -->
                <button
                    class="email-item group relative flex w-full flex-col border-b border-border px-5 py-3 text-left transition-colors {emailState.selectedEmailId ===
                    email.id
                        ? 'active'
                        : ''}"
                    onclick={() => emailState.selectEmail(email.id)}
                    oncontextmenu={(e) => openContextMenu(e, email)}
                >
                    <!--
                      未读指示圆点
                      使用绝对定位放置在邮件项左侧
                      仅当邮件未读时显示
                    -->
                    {#if !email.is_read}
                        <div
                            class="unread-dot absolute left-2 top-1/2 h-1.5 w-1.5 -translate-y-1/2 rounded-full bg-primary"
                        ></div>
                    {/if}

                    <!-- 第一行：发件人名称 + 发送时间 -->
                    <div class="flex items-center justify-between gap-2">
                        <!--
                          发件人名称
                          - 已读邮件：普通文字颜色
                          - 未读邮件：加粗显示，突出提示
                          - 优先显示 sender_name，无则显示 sender_email
                        -->
                        <span
                            class="truncate text-sm {email.is_read
                                ? 'text-muted-foreground'
                                : 'font-semibold text-foreground'}"
                        >
                            {email.sender_name || email.sender_email}
                        </span>
                        <!-- 发送时间（简短格式） -->
                        <span class="shrink-0 text-xs text-muted-foreground">
                            {formatDate(email.sent_at)}
                        </span>
                    </div>

                    <!-- 第二行：星标 + 邮件主题 -->
                    <div class="mt-0.5 flex items-center gap-2">
                        <!--
                          星标图标
                          仅当邮件被标记星标时显示
                          使用黄色填充样式
                        -->
                        {#if email.is_starred}
                            <Star
                                size={14}
                                class="shrink-0 fill-yellow-400 text-yellow-400"
                            />
                        {/if}
                        <!--
                          邮件主题
                          - 已读邮件：普通文字颜色
                          - 未读邮件：中等加粗显示
                          - 无主题时显示 "(No Subject)"
                        -->
                        <span
                            class="truncate text-sm {email.is_read
                                ? 'text-muted-foreground'
                                : 'font-medium text-foreground'}"
                        >
                            {email.subject || "(No Subject)"}
                        </span>
                    </div>

                    <!-- 第三行：预览文本（邮件正文前几行，如有） -->
                    {#if email.preview}
                        <p
                            class="mt-0.5 truncate text-xs text-muted-foreground"
                        >
                            {email.preview}
                        </p>
                    {/if}
                </button>
            {/each}
        {/if}
    </div>

    <!-- ==================== 右键菜单组件 ==================== -->

    <!--
      条件渲染右键菜单
      当 contextMenu.visible 为 true 时渲染 EmailContextMenu 子组件
      传递菜单位置坐标、目标邮件信息和关闭回调函数
    -->
    {#if contextMenu.visible}
        <EmailContextMenu
            x={contextMenu.x}
            y={contextMenu.y}
            emailId={contextMenu.emailId}
            isRead={contextMenu.isRead}
            isStarred={contextMenu.isStarred}
            onClose={closeContextMenu}
        />
    {/if}
</div>

<!-- ==================== 组件样式 ====================-->

<!--
  组件局部样式说明：
  - .email-item：邮件列表项的基础样式，包括悬停和选中效果
  - .email-item.active：选中状态的邮件项，带主题色左边框高亮
  - .unread-dot：未读指示圆点样式（在模板中已通过 Tailwind 类实现）
-->
<style>
    /* 邮件列表项基础样式 */
    .email-item {
        cursor: pointer;
        transition: background 150ms ease;
    }

    /* 邮件列表项悬停效果：浅色玻璃态背景 */
    .email-item:hover {
        background: var(--color-glass-hover);
    }

    /*
      邮件列表项选中效果
      - 背景色：主题色与透明的 15% 混合，产生淡主题色背景
      - 左边框：3px 实线主题色，作为选中指示条
      - 左内边距：减去边框宽度，保持内容不偏移
    */
    .email-item.active {
        background: color-mix(in srgb, var(--color-primary) 15%, transparent);
        border-left: 3px solid var(--color-primary);
        padding-left: calc(1.25rem - 3px);
    }
</style>
