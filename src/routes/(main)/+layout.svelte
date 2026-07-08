<!--
  Postium Mail - 主应用布局组件
  routes/(main)/+layout.svelte

  本组件是应用主界面布局，负责组织应用的核心 UI 结构。

  ==================== 功能说明 ====================
  1. 渲染应用窗口标题栏（TitleBar）
  2. 渲染侧边栏导航（Sidebar）
  3. 渲染状态栏（StatusBar）
  4. 渲染全局模态框（写邮件、添加账户）
  5. 通过 Context API 共享模态框引用给子组件
  6. 监听系统托盘事件（compose、sync）
  7. 渲染背景装饰元素

  ==================== 在架构中的位置 ====================
  - 作为 SvelteKit 的 (main) 分组布局
  - 包裹所有主页面路由（首页、日历、工作流等）
  - 排除设置页面（设置页面在独立窗口中打开）

  ==================== Context 共享 ====================
  使用 Symbol.for 创建全局唯一键，确保跨模块引用：
  - compose-modal: 写邮件模态框引用
  - add-account-modal: 添加账户模态框引用
  子组件通过 getContext 获取这些引用，从而调用模态框的 show() 等方法

  ==================== 系统托盘事件 ====================
  监听 Tauri 的 "tray-action" 事件：
  - compose: 打开写邮件模态框
  - sync: 触发邮件同步
-->
<script lang="ts">
    // ==================== 导入 Svelte 核心函数 ====================
    import { onMount, setContext } from "svelte";
    // ==================== 导入 Tauri 事件监听 API ====================
    import { listen } from "@tauri-apps/api/event";

    // ==================== 导入布局组件 ====================
    import TitleBar from "$lib/components/layout/TitleBar.svelte"; // 窗口标题栏
    import Sidebar from "$lib/components/layout/Sidebar.svelte"; // 侧边栏导航
    import StatusBar from "$lib/components/layout/StatusBar.svelte"; // 底部状态栏

    // ==================== 导入全局模态框组件 ====================
    import ComposeModal from "$lib/components/email/ComposeModal.svelte"; // 写邮件模态框
    import AddAccountModal from "$lib/components/settings/AddAccountModal.svelte"; // 添加账户模态框

    // ==================== 导入全局状态管理器 ====================
    import { getAccountState } from "$lib/stores/account.svelte"; // 账户状态
    import { getSyncState } from "$lib/stores/sync.svelte"; // 同步状态

    // ==================== Props 定义 ====================
    // children: SvelteKit 提供的子页面内容插槽
    let { children } = $props();

    // ==================== 全局状态获取 ====================
    const account = getAccountState(); // 账户状态实例
    const sync = getSyncState(); // 同步状态实例

    // ==================== 模态框组件引用 ====================
    // 使用 $state 声明响应式状态，绑定到模态框组件实例
    let composeModal = $state<ComposeModal>(); // 写邮件模态框引用
    let addAccountModal = $state<AddAccountModal>(); // 添加账户模态框引用

    // ==================== Context 键定义 ====================
    // 使用 Symbol.for 创建全局唯一的 Context 键
    // 这些键在子组件中通过 getContext 获取，实现跨组件通信
    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal"); // 写邮件模态框的键
    const ADD_ACCOUNT_MODAL_KEY = Symbol.for("add-account-modal"); // 添加账户模态框的键

    // ==================== Context 设置 ====================
    // 将模态框引用获取函数设置到 Context 中
    // 子组件可以通过 getContext 获取这些函数，调用后返回模态框实例
    setContext(COMPOSE_MODAL_KEY, () => composeModal);
    setContext(ADD_ACCOUNT_MODAL_KEY, () => addAccountModal);

    // ==================== 组件生命周期 ====================
    // onMount：组件挂载到 DOM 后执行
    onMount(() => {
        // ==================== 系统托盘事件监听 ====================
        // 监听 Tauri 的 "tray-action" 事件，处理系统托盘菜单点击
        const unlisten = listen<string>("tray-action", (event) => {
            // 事件类型：compose（写邮件）
            if (event.payload === "compose") {
                // 打开写邮件模态框
                composeModal?.show();
            }
            // 事件类型：sync（同步邮件）
            else if (event.payload === "sync") {
                // 判断是否为"所有账户"视图
                if (account.isAllAccounts) {
                    // 同步所有账户
                    sync.syncAllAccounts();
                }
                // 同步单个活跃账户
                else if (account.activeAccountId) {
                    sync.syncAccount(account.activeAccountId);
                }
            }
        });

        // ==================== 清理函数 ====================
        // 组件卸载时取消事件监听，防止内存泄漏
        return () => {
            unlisten.then((fn) => fn());
        };
    });
</script>

<!-- ==================== 模板渲染 ==================== -->

<!--
  主容器：
  - h-screen: 高度占满整个视口（100vh）
  - flex flex-col: 垂直弹性布局（标题栏在上，主内容在中，状态栏在下）
  - overflow-hidden: 防止内容溢出产生滚动条（滚动在各组件内部处理）
  - bg-background text-foreground: 应用主题背景色和文字色
-->
<div
    class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground"
>
    <!-- ==================== 背景装饰 ==================== -->
    <!--
      背景装饰元素：漂浮的彩色光球
      使用 CSS 动画实现缓慢漂浮效果，增加视觉层次感
    -->
    <div class="bg-orbs">
        <div class="orb orb-1"></div>
        <div class="orb orb-2"></div>
        <div class="orb orb-3"></div>
    </div>

    <!-- ==================== 窗口标题栏 ==================== -->
    <!--
      TitleBar 组件：
      - 显示应用图标和名称
      - 提供窗口拖拽区域
      - 提供最小化、最大化、关闭按钮
      - 替代操作系统默认标题栏，实现统一的自定义 UI
    -->
    <TitleBar />

    <!-- ==================== 主内容区域 ==================== -->
    <!--
      main 容器：
      - flex-1: 占据标题栏和状态栏之间的所有剩余空间
      - flex: 水平弹性布局（侧边栏在左，页面内容在右）
      - overflow-hidden: 防止内容溢出
    -->
    <main class="relative flex flex-1 overflow-hidden">
        <!--
          侧边栏：
          - 固定宽度，显示文件夹导航、账户选择、设置入口等
          - 独立滚动
        -->
        <Sidebar />

        <!--
          页面内容容器：
          - flex-1: 占据侧边栏右侧的所有剩余空间
          - overflow-hidden: 防止内容溢出（滚动在页面组件内部处理）
          - {@render children()}: 渲染当前激活的子路由页面内容
        -->
        <div class="flex-1 overflow-hidden">
            {@render children()}
        </div>
    </main>

    <!-- ==================== 状态栏 ==================== -->
    <!--
      StatusBar 组件：
      - 显示同步状态、错误信息
      - 显示当前日期
      - 提供语言切换按钮
      - 提供主题切换按钮
      - 显示应用版本号
    -->
    <StatusBar />

    <!-- ==================== 全局模态框 ==================== -->
    <!--
      写邮件模态框：
      - bind:this 将组件实例绑定到 composeModal 变量
      - 通过 Context 共享引用，子组件可调用 show() 等方法
      - 默认隐藏，通过调用 show() 方法显示
    -->
    <ComposeModal bind:this={composeModal} />

    <!--
      添加账户模态框：
      - bind:this 将组件实例绑定到 addAccountModal 变量
      - 通过 Context 共享引用，子组件可调用 show() 等方法
      - 默认隐藏，通过调用 show() 方法显示
    -->
    <AddAccountModal bind:this={addAccountModal} />
</div>
