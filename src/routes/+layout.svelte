<!--
  Postium Mail - 根布局组件
  routes/+layout.svelte

  本组件是应用的根布局，负责初始化全局状态和提供基础结构。

  ==================== 功能说明 ====================
  1. 初始化全局状态管理器（主题、国际化、账户、邮件、同步、提示）
  2. 设置全局错误处理器
  3. 导入全局样式文件
  4. 渲染子页面内容
  5. 渲染全局 Toast 提示容器

  ==================== 在架构中的位置 ====================
  - 作为 SvelteKit 的根布局，包裹所有页面路由
  - 在所有页面加载前执行，确保全局状态可用
  - 位于路由文件层级的最高层

  ==================== 全局状态初始化 ====================
  按依赖顺序初始化各状态管理器：
  1. ThemeState: 主题状态（影响整体 UI 颜色）
  2. I18nState: 国际化状态（影响所有文本显示）
  3. AccountState: 账户状态（邮件功能依赖）
  4. EmailState: 邮件状态
  5. SyncState: 同步状态
  6. ToastState: 提示消息状态

  ==================== 错误处理 ====================
  在组件挂载时设置全局错误处理器，捕获应用中的未处理异常。
-->
<script lang="ts">
    // ==================== 导入全局样式 ====================
    import "../app.css";

    // ==================== 导入 Svelte 核心函数 ====================
    import { onMount } from "svelte";

    // ==================== 导入全局组件 ====================
    import ToastContainer from "$lib/components/common/Toast.svelte";

    // ==================== 导入全局状态管理器 ====================
    import { createAccountState } from "$lib/stores/account.svelte"; // 账户状态
    import { createEmailState } from "$lib/stores/email.svelte"; // 邮件状态
    import { createI18nState } from "$lib/stores/i18n.svelte"; // 国际化状态
    import { createSyncState } from "$lib/stores/sync.svelte"; // 同步状态
    import { createThemeState } from "$lib/stores/theme.svelte"; // 主题状态
    import { createToastState } from "$lib/stores/toast.svelte"; // 提示消息状态

    // ==================== 导入工具函数 ====================
    import { setupGlobalErrorHandler } from "$lib/utils/error.js"; // 全局错误处理

    // ==================== Props 定义 ====================
    // children: SvelteKit 提供的子页面内容插槽
    let { children } = $props();

    // ==================== 全局状态初始化 ====================
    // 按顺序创建各全局状态管理器的单例实例
    // 这些实例在整个应用生命周期中保持唯一，可通过 get*State() 获取
    createThemeState(); // 主题状态：管理亮色/暗色主题切换
    createI18nState(); // 国际化状态：管理中英文切换
    createAccountState(); // 账户状态：管理邮箱账户列表和当前活跃账户
    createEmailState(); // 邮件状态：管理邮件列表、选中邮件等
    createSyncState(); // 同步状态：管理邮件同步进度和状态
    createToastState(); // 提示消息状态：管理全局提示消息

    // ==================== 组件生命周期 ====================
    // onMount：组件挂载到 DOM 后执行
    // 设置全局错误处理器，捕获应用中的未处理异常
    onMount(() => {
        setupGlobalErrorHandler();
    });
</script>

<!-- ==================== 模板渲染 ==================== -->

<!--
  SvelteKit 插槽渲染：
  {@render children()} 是 Svelte 5 的语法，用于渲染子路由页面内容。
  所有匹配此布局的页面内容都将在这里渲染。
-->
{@render children()}

<!--
  全局 Toast 提示容器：
  ToastContainer 是全局提示消息的容器组件。
  它固定在视口右下角，显示来自任何组件的提示消息。
-->
<ToastContainer />
