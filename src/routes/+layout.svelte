<!--
  Postium Mail 主布局组件
  +layout.svelte

  本组件是应用的主布局框架，负责：
  1. 初始化全局状态管理（主题、国际化、账户、邮件、同步、提示等）
  2. 管理全局模态框引用（写邮件模态框、添加账户模态框）
  3. 处理系统托盘事件监听（来自 Tauri 后端）
  4. 设置全局错误处理器
  5. 渲染应用的核心布局结构（标题栏、侧边栏、主内容区、状态栏）

  作为 SvelteKit 的布局组件，所有路由页面都会渲染在 children 插槽中。
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入全局样式
    import "../app.css";

    // 导入 Svelte 核心函数
    import { onMount, setContext } from "svelte";

    // 导入 Tauri 事件监听 API
    import { listen } from "@tauri-apps/api/event";

    // 导入布局组件
    import TitleBar from "$lib/components/layout/TitleBar.svelte"; // 标题栏
    import StatusBar from "$lib/components/layout/StatusBar.svelte"; // 状态栏
    import Sidebar from "$lib/components/layout/Sidebar.svelte"; // 侧边栏

    // 导入模态框组件
    import ComposeModal from "$lib/components/email/ComposeModal.svelte"; // 写邮件模态框
    import AddAccountModal from "$lib/components/settings/AddAccountModal.svelte"; // 添加账户模态框

    // 导入全局状态管理器
    import { createThemeState } from "$lib/stores/theme.svelte"; // 主题状态
    import { createI18nState } from "$lib/stores/i18n.svelte"; // 国际化状态
    import { createAccountState } from "$lib/stores/account.svelte"; // 账户状态
    import { createEmailState } from "$lib/stores/email.svelte"; // 邮件状态
    import { createSyncState } from "$lib/stores/sync.svelte"; // 同步状态
    import { createToastState } from "$lib/stores/toast.svelte"; // 提示消息状态

    // 导入错误处理工具和通知容器
    import { setupGlobalErrorHandler } from "$lib/utils/error.js"; // 全局错误处理器
    import ToastContainer from "$lib/components/common/Toast.svelte"; // 提示消息容器

    // ==================== Props 定义 ====================

    // children：子路由组件（通过 SvelteKit 自动传入）
    // 使用 $props() 解构，这是 Svelte 5 的新语法
    let { children } = $props();

    // ==================== 全局状态初始化 ====================

    // 创建各种状态实例，这些状态在整个应用中都是单例
    // 使用 Svelte 5 的 Runes API 实现响应式状态管理

    const theme = createThemeState(); // 管理亮色/暗色主题切换
    const i18n = createI18nState(); // 管理多语言翻译
    const account = createAccountState(); // 管理用户账户列表和当前活动账户
    const email = createEmailState(); // 管理邮件列表和邮件详情
    const sync = createSyncState(); // 管理邮件同步进度和状态
    const toast = createToastState(); // 管理全局提示消息（Toast）

    // ==================== 模态框引用管理 ====================

    // 使用 $state 声明模态框组件引用
    // 这些引用用于在代码中通过 JavaScript 控制模态框的显示/隐藏

    let composeModal = $state<ComposeModal>(); // 写邮件模态框引用
    let addAccountModal = $state<AddAccountModal>(); // 添加账户模态框引用

    // ==================== Context 共享机制 ====================

    // 通过 Svelte Context API 将模态框引用暴露给子组件
    // 这样子组件可以从 context 中获取引用，直接调用模态框方法
    // 避免了需要在每个页面中重复声明模态框实例

    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal"); // 写邮件模态框的唯一键
    const ADD_ACCOUNT_MODAL_KEY = Symbol.for("add-account-modal"); // 添加账户模态框的唯一键

    // 将模态框引用设置到 context 中，值是一个函数，返回引用
    // 使用函数封装是为了确保始终获取最新的引用（如果需要重新赋值的话）
    setContext(COMPOSE_MODAL_KEY, () => composeModal);
    setContext(ADD_ACCOUNT_MODAL_KEY, () => addAccountModal);

    // ==================== 组件生命周期和事件监听 ====================

    // onMount：组件挂载到 DOM 后执行
    onMount(() => {
        // 1. 设置全局错误处理器
        // 捕获未处理的 Promise rejection 和 JavaScript 错误
        setupGlobalErrorHandler();

        // 2. 监听来自 Tauri 后端的系统托盘事件
        // 系统托盘图标右键菜单的点击事件会通过 Tauri 事件系统发送到前端
        const unlisten = listen<string>("tray-action", (event) => {
            // event.payload：事件载荷，包含操作类型字符串

            // 处理"写邮件"操作
            if (event.payload === "compose") {
                // 调用写邮件模态框的 show() 方法显示模态框
                // 使用可选链操作符 (?.) 防止 modal 未初始化时报错
                composeModal?.show();
            }
            // 处理"同步"操作
            else if (event.payload === "sync") {
                // 检查是否有活动账户
                if (account.activeAccountId) {
                    // 触发该账户的邮件同步
                    sync.syncAccount(account.activeAccountId);
                }
            }
        });

        // 返回清理函数：组件卸载时执行
        // 取消事件监听，防止内存泄漏
        return () => {
            // unlisten 返回一个 Promise，resolve 时得到取消监听的函数
            unlisten.then((fn) => fn());
        };
    });
</script>

<!-- ==================== 模板渲染 ==================== -->

<!--
  主容器布局：
  - relative：相对定位，为子元素的绝对定位提供参考
  - flex：弹性布局，垂直方向排列
  - h-screen：占满整个屏幕高度
  - overflow-hidden：隐藏溢出内容（防止整个页面滚动，内部组件独立滚动）
  - bg-background / text-foreground：应用主题背景色和文字颜色
-->
<div
    class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground"
>
    <!-- 背景装饰：玻璃态浮动球体 -->
    <!-- 这些球体在 app.css 中定义动画效果，创造动态背景 -->
    <div class="bg-orbs">
        <div class="orb orb-1"></div>
        <!-- 右上角紫色球体 -->
        <div class="orb orb-2"></div>
        <!-- 左下角紫红球体 -->
        <div class="orb orb-3"></div>
        <!-- 居中紫蓝球体 -->
    </div>

    <!-- ==================== 应用布局区域 ==================== -->

    <!-- 标题栏：显示应用名称、窗口控制按钮等 -->
    <TitleBar />

    <!-- 主内容区域：包含侧边栏和子页面内容 -->
    <main class="relative flex flex-1 overflow-hidden">
        <!-- 侧边栏：显示账户列表、邮件文件夹、标签等导航项 -->
        <Sidebar />

        <!-- 子页面内容容器 -->
        <!-- flex-1：占据剩余空间 -->
        <!-- overflow-hidden：防止内容溢出，内部滚动由子组件处理 -->
        <div class="flex-1 overflow-hidden">
            <!--
        {@render children()}：Svelte 5 的插槽语法
        渲染当前路由的页面组件（如 +page.svelte）
        所有路由页面都会在这里显示
      -->
            {@render children()}
        </div>
    </main>

    <!-- 状态栏：显示同步状态、邮件数量统计等信息 -->
    <StatusBar />

    <!-- ==================== 全局模态框 ==================== -->

    <!--
    写邮件模态框
    bind:this={composeModal}：将组件实例绑定到 composeModal 变量
    这样可以在 JavaScript 中通过 composeModal.show() 等方法控制模态框
  -->
    <ComposeModal bind:this={composeModal} />

    <!-- 添加账户模态框 -->
    <AddAccountModal bind:this={addAccountModal} />

    <!-- 提示消息容器：显示全局 Toast 通知（成功、错误、警告等） -->
    <ToastContainer />
</div>
