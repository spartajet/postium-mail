<!--
  Postium Mail - 提示消息（Toast）组件
  Toast.svelte

  本组件是应用的全局提示消息容器，负责：
  1. 显示全局提示消息（Toast 通知）
  2. 支持四种消息类型：成功（success）、错误（error）、信息（info）、警告（warning）
  3. 自动根据消息类型应用不同的颜色样式和图标
  4. 支持自动定时消失（根据每条消息的 duration 属性）
  5. 支持手动关闭（点击关闭按钮）
  6. 消息以栈形式从底部右侧弹出，带滑入动画

  在架构中的位置：
  - 位于 +layout.svelte 主布局的最顶层（z-9999），覆盖在所有内容之上
  - 通过 toastStore（全局状态管理器）管理消息的添加和移除
  - 其他组件通过调用 toastStore.show() 方法触发消息显示
  - 使用 $effect 监听消息列表变化，为新消息启动自动消失计时器
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入全局提示消息状态管理器
    // getToastState：获取 toast 状态单例，包含 toasts 数组和 dismiss 方法
    import { getToastState } from "$lib/stores/toast.svelte";
    import { SvelteSet } from "svelte/reactivity";

    // 导入消息类型定义
    // ToastType：消息类型枚举（"success" | "error" | "info" | "warning"）
    import type { ToastType } from "$lib/stores/toast.svelte";

    // ==================== 全局状态获取 ====================

    // 获取提示消息状态管理器的单例实例
    // toastState 包含：
    // - toasts：当前显示中的消息数组
    // - show()：添加新消息
    // - dismiss(id)：移除指定消息
    let toastState = getToastState();

    // ==================== 消息追踪机制 ====================

    // 已处理消息 ID 集合
    // 用于追踪哪些消息已经启动了自动消失计时器
    // 防止同一条消息被重复设置计时器（$effect 可能多次触发）
    let seen = new SvelteSet<number>();

    // ==================== 自动消失逻辑 ====================

    // $effect：Svelte 5 Runes 副作用声明
    // 监听 toastState.toasts 数组变化，为新消息启动自动消失计时器
    $effect(() => {
        for (const t of toastState.toasts) {
            // 仅处理尚未被追踪的新消息
            if (!seen.has(t.id)) {
                // 将消息 ID 加入已处理集合
                seen.add(t.id);

                // 保存当前消息 ID（闭包中使用，防止循环变量问题）
                const id = t.id;

                // 设置定时器：在 duration 毫秒后自动移除消息
                setTimeout(() => {
                    // 从已处理集合中移除（允许同一 ID 的消息再次显示）
                    seen.delete(id);
                    // 调用 dismiss 从状态中移除消息，触发 UI 更新
                    toastState.dismiss(id);
                }, t.duration);
            }
        }
    });

    // ==================== 样式辅助函数 ====================

    // 根据消息类型获取对应的 CSS 类名组合
    // 参数 type：消息类型（success/error/info/warning）
    // 返回值：包含背景色、边框色、文字色的 Tailwind 类名字符串
    // 每种类型使用不同的语义化颜色：
    // - success：绿色系（bg-success）
    // - error：红色系（bg-destructive）
    // - info：蓝色系（bg-info）
    // - warning：黄色系（bg-warning）
    function getColor(type: ToastType): string {
        if (type === "success")
            return "bg-success/15 border-success/30 text-success";
        if (type === "error")
            return "bg-destructive/15 border-destructive/30 text-destructive";
        if (type === "info") return "bg-info/15 border-info/30 text-info";
        return "bg-warning/15 border-warning/30 text-warning";
    }

    // 根据消息类型获取对应的 Unicode 图标字符
    // 参数 type：消息类型（success/error/info/warning）
    // 返回值：Unicode 符号字符
    // - success：✓（对号）
    // - error：✗（叉号）
    // - info：ℹ（信息符号）
    // - warning：⚠（警告符号）
    function getIcon(type: ToastType): string {
        if (type === "success") return "\u2713"; // ✓
        if (type === "error") return "\u2717"; // ✗
        if (type === "info") return "\u2139"; // ℹ
        return "\u26A0"; // ⚠
    }

    // ==================== 手动关闭处理 ====================

    // 手动关闭消息（用户点击关闭按钮时调用）
    // 参数 id：要关闭的消息 ID
    // 执行以下操作：
    // 1. 从已处理集合中移除（允许同一 ID 的消息再次显示）
    // 2. 调用 dismiss 从状态中移除消息，触发 UI 更新
    function dismiss(id: number) {
        seen.delete(id);
        toastState.dismiss(id);
    }
</script>

<!-- ==================== 消息容器渲染 ==================== -->

<!--
  条件渲染：仅在有消息时显示容器
  使用 {#if} 块确保消息列表为空时不渲染任何 DOM 元素
-->
{#if toastState.toasts.length > 0}
    <!--
      消息容器：固定在视口右下角

      类名说明：
      - fixed：固定定位，相对于视口
      - bottom-4 right-4：距离底部和右侧各 16px
      - z-9999：极高的层级，确保覆盖在所有内容之上
      - flex flex-col：弹性布局，垂直方向排列（新消息在下方）
      - gap-2：消息之间的间距 8px
    -->
    <div class="fixed bottom-4 right-4 z-9999 flex flex-col gap-2">
        <!--
          遍历消息列表
          (t.id)：Svelte 的 key 语法，使用消息 ID 作为唯一标识
          确保消息的添加/移除操作能正确触发动画
        -->
        {#each toastState.toasts as t (t.id)}
            <!--
              单条消息卡片

              类名说明：
              - flex items-center：弹性布局，子元素垂直居中
              - gap-2：图标与文字间距 8px
              - rounded-lg：圆角 8px
              - border：边框
              - px-4 py-3：内边距
              - shadow-lg：大阴影（浮起效果）
              - backdrop-blur-sm：轻微背景模糊
              - animate-slide-in：自定义滑入动画（在下方 style 中定义）
              - {getColor(t.type)}：根据消息类型动态应用颜色样式

              role="alert"：无障碍属性，屏幕阅读器会立即朗读此消息
            -->
            <div
                class="flex items-center gap-2 rounded-lg border px-4 py-3 shadow-lg backdrop-blur-sm animate-slide-in {getColor(
                    t.type,
                )}"
                role="alert"
            >
                <!-- 消息类型图标：根据类型显示对应的 Unicode 符号 -->
                <span class="text-lg">{getIcon(t.type)}</span>

                <!-- 消息文本内容 -->
                <span class="text-sm flex-1">{t.message}</span>

                <!--
                  手动关闭按钮
                  类名说明：
                  - ml-2：左外边距 8px
                  - rounded p-1：圆角 + 内边距
                  - opacity-60：默认 60% 不透明度
                  - hover:opacity-100：悬停时完全显示
                  - transition-opacity：不透明度过渡动画
                -->
                <button
                    class="ml-2 rounded p-1 opacity-60 hover:opacity-100 transition-opacity"
                    onclick={() => dismiss(t.id)}
                    aria-label="close"
                >
                    &times;
                </button>
            </div>
        {/each}
    </div>
{/if}

<style>
    /* ==================== 滑入动画定义 ==================== */

    /*
      .animate-slide-in：消息弹出时的滑入动画类
      从右侧滑入并淡入，持续 0.2 秒
    */
    .animate-slide-in {
        animation: slideIn 0.2s ease-out;
    }

    /*
      @keyframes slideIn：滑入动画关键帧
      - from：初始状态，完全在右侧视口外（translateX(100%)），完全透明
      - to：结束状态，回到正常位置（translateX(0)），完全显示
      使用 ease-out 缓动函数，使动画开始快结束慢，更自然
    */
    @keyframes slideIn {
        from {
            transform: translateX(100%);
            opacity: 0;
        }
        to {
            transform: translateX(0);
            opacity: 1;
        }
    }
</style>
