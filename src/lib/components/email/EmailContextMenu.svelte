<!--
  Postium Mail - 邮件右键上下文菜单组件
  EmailContextMenu.svelte

  本组件是邮件列表的右键菜单，提供对单封邮件的快捷操作。

  ==================== 功能说明 ====================
  1. 在邮件项上右键点击时弹出浮动菜单
  2. 提供邮件快捷操作：回复、全部回复、转发
  3. 提供邮件标记操作：星标/取消星标、标为已读/未读
  4. 提供邮件管理操作：删除、更多操作
  5. 自动调整菜单位置，防止超出视口边界
  6. 点击菜单外部区域自动关闭菜单

  ==================== 在架构中的位置 ====================
  位于 src/lib/components/email/ 目录下，是邮件功能模块的子组件。
  被引用于：
    - EmailList.svelte（邮件列表组件中的右键事件触发）

  依赖的 Store：
    - i18nStore：国际化翻译

  ==================== 交互流程 ====================
  1. 用户在 EmailList 中右键点击邮件项 → 调用 openContextMenu()
  2. EmailList 将菜单位置和邮件信息传递给本组件的 props
  3. 本组件渲染浮动菜单，自动调整位置防止溢出
  4. 用户点击菜单项 → 执行对应操作 → 调用 onClose 关闭菜单
  5. 用户点击菜单外部 → handleClickOutside 关闭菜单
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入图标组件（Lucide 图标库）
    import {
        Reply, // 回复图标
        ReplyAll, // 全部回复图标
        Forward, // 转发图标
        Flag, // 标记/旗帜图标（用于星标操作）
        Mail, // 邮件图标（用于已读/未读切换）
        Trash2, // 删除图标
        MoreHorizontal, // 更多操作图标（三个水平点）
    } from "lucide-svelte";

    // ==================== 状态初始化 ====================

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);

    // ==================== Props 定义 ====================

    /**
     * 组件属性（从父组件 EmailList 传入）
     *
     * @param x - 菜单出现的 X 坐标（鼠标右键点击位置，像素）
     * @param y - 菜单出现的 Y 坐标（鼠标右键点击位置，像素）
     * @param emailId - 目标邮件的唯一 ID
     * @param isRead - 目标邮件的当前已读状态
     * @param isStarred - 目标邮件的当前星标状态
     * @param disabled - 目标邮件是否正在执行操作
     * @param onToggleStar - 切换星标回调
     * @param onToggleRead - 切换已读/未读回调
     * @param onDelete - 删除回调
     * @param onForward - 转发回调
     * @param onClose - 关闭菜单的回调函数（通知父组件隐藏菜单）
     */
    let {
        x,
        y,
        emailId,
        isRead,
        isStarred,
        disabled = false,
        onToggleStar,
        onToggleRead,
        onDelete,
        onForward,
        onClose,
    }: {
        x: number;
        y: number;
        emailId: number;
        isRead: boolean;
        isStarred: boolean;
        disabled?: boolean;
        onToggleStar: (emailId: number) => Promise<void> | void;
        onToggleRead: (
            emailId: number,
            isRead: boolean,
        ) => Promise<void> | void;
        onDelete: (emailId: number) => Promise<void> | void;
        onForward: (emailId: number) => Promise<void> | void;
        onClose: () => void;
    } = $props();

    // 菜单 DOM 元素引用，用于检测点击是否在菜单外部
    let menuEl: HTMLDivElement | undefined = $state();

    // ==================== 菜单位置计算 ====================

    /**
     * 动态计算菜单位置样式
     *
     * 使用 $derived.by 实现派生计算，根据传入的 (x, y) 坐标
     * 自动调整菜单位置，确保菜单不会超出浏览器视口边界。
     *
     * 计算逻辑：
     * - 如果菜单右边界超出视口宽度 → 向左偏移（x - menuWidth）
     * - 如果菜单下边界超出视口高度 → 向上偏移（y - menuHeight）
     * - 使用 Math.max(0, ...) 确保坐标不为负数
     */
    let menuStyle = $derived.by(() => {
        // 预估菜单宽度和高度（像素）
        const menuWidth = 200;
        const menuHeight = 340;
        // 获取视口尺寸
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        // 调整 X 坐标：如果右侧溢出，则向左偏移
        const adjustedX = x + menuWidth > vw ? Math.max(0, x - menuWidth) : x;
        // 调整 Y 坐标：如果下方溢出，则向上偏移
        const adjustedY = y + menuHeight > vh ? Math.max(0, y - menuHeight) : y;
        // 返回内联样式字符串
        return `left: ${adjustedX}px; top: ${adjustedY}px;`;
    });

    // ==================== 事件处理函数 ====================

    /**
     * 处理菜单项点击操作
     *
     * 根据用户点击的菜单项执行对应的邮件操作。
     *
     * @param action - 操作类型标识符
     */
    async function handleAction(action: string) {
        if (disabled) return;

        if (action === "star") {
            await onToggleStar(emailId);
        } else if (action === "toggleRead") {
            await onToggleRead(emailId, !isRead);
        } else if (action === "delete") {
            await onDelete(emailId);
        } else if (action === "forward") {
            await onForward(emailId);
        }

        onClose();
    }

    /**
     * 处理点击菜单外部区域
     *
     * 当用户点击菜单以外的区域时关闭菜单。
     * 通过检查事件目标元素是否在菜单 DOM 内部来判断。
     * 绑定到 svelte:window 的 onclick 事件，实现全局点击监听。
     *
     * @param e - 鼠标点击事件对象
     */
    function handleClickOutside(e: MouseEvent) {
        // 检查点击目标是否不在菜单元素内
        if (menuEl && !menuEl.contains(e.target as Node)) {
            // 点击在菜单外部，关闭菜单
            onClose();
        }
    }
</script>

<!-- 全局点击事件监听：点击菜单外部时关闭菜单 -->
<svelte:window onclick={handleClickOutside} />

<!-- ==================== 模板渲染 ==================== -->

<!--
  右键菜单容器

  类名说明：
  - fixed：固定定位，相对于视口（配合 style 的 left/top 实现精确定位）
  - z-[100]：高层级 z-index，确保菜单显示在其他内容之上
  - min-w-[180px]：最小宽度 180px
  - rounded-lg：圆角
  - border border-border：边框
  - bg-card：卡片背景色（跟随主题）
  - py-1：垂直内边距
  - shadow-lg：大阴影，增强浮层视觉效果
-->
<div
    bind:this={menuEl}
    class="fixed z-100 min-w-45 rounded-lg border border-border bg-card py-1 shadow-lg"
    style={menuStyle}
>
    <!-- ==================== 邮件回复/转发操作 ==================== -->

    <!--
      回复按钮
      向原始发件人发送回复邮件
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-muted-foreground opacity-50"
        disabled
    >
        <Reply size={15} class="text-muted-foreground" />
        <span>{t.email.reply}</span>
    </button>

    <!--
      全部回复按钮
      向原始发件人和所有收件人发送回复邮件
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-muted-foreground opacity-50"
        disabled
    >
        <ReplyAll size={15} class="text-muted-foreground" />
        <span>{t.email.replyAll}</span>
    </button>

    <!--
      转发按钮
      将邮件转发给其他收件人
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("forward")}
        disabled={disabled}
    >
        <Forward size={15} class="text-muted-foreground" />
        <span>{t.email.forward}</span>
    </button>

    <!-- ==================== 分割线 ==================== -->

    <!--
      分隔线
      将回复/转发操作与标记操作分开，增强视觉分组
    -->
    <div class="my-1 border-t border-border"></div>

    <!-- ==================== 邮件标记操作 ==================== -->

    <!--
      星标切换按钮
      - 已星标：显示黄色旗帜图标 + "取消星标" 文本
      - 未星标：显示灰色旗帜图标 + "添加星标" 文本
      图标颜色根据当前星标状态动态切换
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("star")}
        disabled={disabled}
    >
        <Flag
            size={15}
            class={isStarred ? "text-yellow-500" : "text-muted-foreground"}
        />
        <span>{isStarred ? t.email.unstar : t.email.star}</span>
    </button>

    <!--
      已读/未读切换按钮
      - 已读邮件：显示 "标为未读" 选项
      - 未读邮件：显示 "标为已读" 选项
      文本根据当前已读状态动态切换
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm transition-colors hover:bg-glass-hover"
        onclick={() => handleAction("toggleRead")}
        disabled={disabled}
    >
        <Mail size={15} class="text-muted-foreground" />
        <span>{isRead ? t.email.markUnread : t.email.markRead}</span>
    </button>

    <!-- ==================== 分割线 ==================== -->

    <!--
      分隔线
      将标记操作与危险操作（删除）分开
    -->
    <div class="my-1 border-t border-border"></div>

    <!-- ==================== 邮件管理操作 ==================== -->

    <!--
      删除按钮
      使用红色文字（destructive）标识危险操作
      悬停时显示红色半透明背景
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-destructive transition-colors hover:bg-destructive/10"
        onclick={() => handleAction("delete")}
        disabled={disabled}
    >
        <Trash2 size={15} />
        <span>{t.email.delete}</span>
    </button>

    <!-- ==================== 分割线 ==================== -->

    <!--
      分隔线
      将主要操作与更多操作分开
    -->
    <div class="my-1 border-t border-border"></div>

    <!-- ==================== 更多操作 ==================== -->

    <!--
      更多操作按钮
      预留扩展入口，未来可添加移动到文件夹、设置标签等操作
    -->
    <button
        class="flex w-full items-center gap-2.5 px-3 py-1.5 text-left text-sm text-muted-foreground opacity-50"
        disabled
    >
        <MoreHorizontal size={15} class="text-muted-foreground" />
        <span>{t.common.operations}</span>
    </button>
</div>
