<!--
  Postium Mail - 标题栏组件
  TitleBar.svelte

  本组件是应用的自定义窗口标题栏，负责：
  1. 显示应用图标和名称
  2. 提供窗口拖拽移动区域
  3. 提供窗口控制按钮（最小化、最大化/还原、关闭）
  4. 监听窗口大小变化，更新最大化状态

  在架构中的位置：
  - 位于 +layout.svelte 主布局的顶部
  - 使用 Tauri 窗口 API 实现原生窗口控制
  - 替代操作系统默认标题栏，实现统一的自定义 UI

  注意：关闭按钮实际执行的是窗口隐藏（hide）而非关闭（close），
  以支持系统托盘后台运行功能。
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入 Tauri 窗口管理 API
    // getCurrentWindow：获取当前应用窗口实例，用于调用窗口控制方法
    import { getCurrentWindow } from "@tauri-apps/api/window";

    // 导入图标组件（来自 lucide-svelte 图标库）
    import { Mail, Minus, Square, Copy, X } from "lucide-svelte";

    // ==================== 窗口实例获取 ====================

    // 获取当前 Tauri 窗口实例
    // 通过此实例可以调用 minimize()、toggleMaximize()、hide() 等窗口控制方法
    const appWindow = getCurrentWindow();

    type CloseBehavior = "hide" | "close";

    let {
        title = "Postium Mail",
        closeBehavior = "hide",
        onCloseError,
    } = $props<{
        title?: string;
        closeBehavior?: CloseBehavior;
        onCloseError?: () => void | Promise<void>;
    }>();

    // ==================== 响应式状态 ====================

    // 窗口是否处于最大化状态
    // $state：Svelte 5 Runes 响应式状态声明
    // 用于切换最大化/还原图标的显示
    let isMaximized = $state(false);

    // ==================== 窗口控制函数 ====================

    // 最小化窗口
    // 调用 Tauri API 将窗口最小化到任务栏
    async function minimize() {
        await appWindow.minimize();
    }

    // 切换窗口最大化/还原状态
    // 调用 Tauri API 在最大化和正常大小之间切换
    // 切换后需要更新 isMaximized 状态以反映当前窗口状态
    async function toggleMaximize() {
        await appWindow.toggleMaximize();
        await updateState();
    }

    // 主窗口使用 hide() 保持托盘后台运行，独立窗体使用 close() 释放窗口。
    async function close() {
        try {
            if (closeBehavior === "close") {
                await appWindow.close();
                return;
            }
            await appWindow.hide();
        } catch {
            await onCloseError?.();
        }
    }

    // 更新窗口最大化状态
    // 查询 Tauri 窗口的实际最大化状态并同步到响应式变量
    async function updateState() {
        isMaximized = await appWindow.isMaximized();
    }

    // 处理拖拽区域双击事件
    // 双击标题栏切换最大化/还原（模拟操作系统默认行为）
    function handleDragDblClick() {
        toggleMaximize();
    }

    // ==================== 组件生命周期 ====================

    // $effect：Svelte 5 Runes 副作用声明
    // 组件挂载时执行初始化，并设置窗口大小变化监听
    $effect(() => {
        // 初始化时更新一次最大化状态
        updateState();

        // 监听窗口大小变化事件
        // 当用户拖拽窗口边缘调整大小时，需要同步更新最大化状态
        // 例如：从最大化状态拖拽还原后，isMaximized 应变为 false
        const unlisten = appWindow.onResized(async () => {
            await updateState();
        });

        // 返回清理函数：组件卸载时取消事件监听，防止内存泄漏
        return () => {
            unlisten.then((fn) => fn());
        };
    });
</script>

<!-- ==================== 标题栏布局 ==================== -->

<!--
  标题栏容器：

  类名说明：
  - relative：相对定位，为子元素提供定位参考
  - flex：弹性布局，水平排列子元素
  - h-10：固定高度 40px（10 × 4px）
  - select-none：禁止文本选中（标题栏不需要选中文字）
  - items-center：垂直居中对齐
  - justify-between：两端对齐（标题在左，按钮在右）
  - border-b border-border：底部边框分隔线
  - bg-elevated/80：半透明提升背景色
  - backdrop-blur-md：中等程度的背景模糊（毛玻璃效果）
-->
<div
    class="relative flex h-10 select-none items-center justify-between border-b border-border bg-elevated/80 backdrop-blur-md"
>
    <!-- ==================== 拖拽区域与应用标识 ==================== -->

    <!--
      窗口拖拽区域：
      data-tauri-drag-region 属性使该区域可拖拽移动窗口
      双击该区域切换最大化/还原状态

      类名说明：
      - flex：弹性布局
      - flex-1：占据剩余空间
      - items-center：垂直居中
      - gap-2：子元素间距 8px
      - pl-4：左内边距 16px
    -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="flex flex-1 items-center gap-2 pl-4"
        data-tauri-drag-region
        ondblclick={handleDragDblClick}
    >
        <!-- 应用图标：邮件图标，使用主题色 -->
        <Mail size={16} class="text-primary" />

        <!-- 应用名称文本 -->
        <!-- data-tauri-drag-region：确保文字区域也可拖拽 -->
        <span
            data-testid="window-title"
            class="text-[13px] font-medium text-foreground"
            data-tauri-drag-region>{title}</span
        >
    </div>

    <!-- ==================== 窗口控制按钮组 ==================== -->

    <!--
      窗口控制按钮容器：
      style="-webkit-app-region: no-drag;" 确保按钮区域不触发窗口拖拽，
      使按钮可以被正常点击
    -->
    <div class="flex h-full" style="-webkit-app-region: no-drag;">
        <!-- 最小化按钮 -->
        <button class="control-btn" onclick={minimize} aria-label="Minimize">
            <Minus size={12} />
        </button>

        <!--
          最大化/还原按钮
          根据 isMaximized 状态切换显示不同图标：
          - 已最大化时显示还原图标（Copy，表示重叠的窗口）
          - 未最大化时显示最大化图标（Square，表示放大的窗口）
        -->
        <button
            class="control-btn"
            onclick={toggleMaximize}
            aria-label={isMaximized ? "Restore" : "Maximize"}
        >
            {#if isMaximized}
                <Copy size={12} />
            {:else}
                <Square size={12} />
            {/if}
        </button>

        <!-- 关闭按钮：使用特殊的 close-btn 样式，悬停时显示红色背景 -->
        <button
            class="control-btn close-btn"
            onclick={close}
            aria-label="Close"
        >
            <X size={12} />
        </button>
    </div>
</div>
