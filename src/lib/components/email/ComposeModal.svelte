<!--
  Postium Mail - 写邮件模态框组件
  ComposeModal.svelte

  本组件是邮件编辑和发送的模态框，作为应用全局模态框使用。

  ==================== 功能说明 ====================
  1. 提供邮件编辑界面：收件人、抄送、主题、正文（富文本）
  2. 支持发送邮件（调用后端 Tauri 命令）
  3. 支持回复邮件（预填充收件人和主题，带 Re: 前缀）
  4. 支持转发邮件（预填充主题和正文，带 Fwd: 前缀）
  5. 提供错误提示和发送状态反馈
  6. 支持点击遮罩层关闭模态框

  ==================== 在架构中的位置 ====================
  位于 src/lib/components/email/ 目录下，是邮件功能模块的子组件。
  被引用于：
    - +layout.svelte（作为全局模态框实例化，通过 Context 共享引用）
    - EmailDetail.svelte（通过 Context 获取引用，调用回复/转发方法）

  依赖的 Store：
    - accountStore：获取当前活跃账户 ID（用于发送邮件）
    - i18nStore：国际化翻译

  依赖的子组件：
    - RichTextEditor：富文本编辑器（邮件正文编辑）

  依赖的后端命令：
    - commands.sendEmail：发送邮件 API

  ==================== 使用方式 ====================
  1. 在 +layout.svelte 中通过 bind:this 获取组件实例
  2. 将实例引用通过 setContext 共享给子组件
  3. 子组件通过 getContext 获取引用，调用以下方法：
     - show()：打开空白写邮件窗口
     - showReply()：打开回复邮件窗口（预填充信息）
     - showForward()：打开转发邮件窗口（预填充信息）
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入账户状态管理，用于获取当前活跃账户 ID
    import { getAccountState } from "$lib/stores/account.svelte";
    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入 Tauri 后端命令接口，用于调用发送邮件等后端方法
    import { commands } from "$lib/bindings";
    // 导入关闭/清除图标
    import { ChevronDown, X } from "lucide-svelte";
    // 导入富文本编辑器子组件
    import RichTextEditor from "./RichTextEditor.svelte";

    // ==================== 状态初始化 ====================

    // 获取账户状态实例（包含当前活跃账户信息）
    const accountStore = getAccountState();
    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);

    // ==================== 模态框状态 ====================

    // 模态框是否打开（visible 状态）
    let open = $state(false);
    // 收件人邮箱地址（多个用逗号分隔）
    let to = $state("");
    // 抄送邮箱地址（多个用逗号分隔，可选）
    let cc = $state("");
    // 邮件主题
    let subject = $state("");
    // 发送中状态标志（防止重复提交）
    let sending = $state(false);
    // 错误信息（发送失败时显示）
    let error = $state("");
    // 所有账号视图下的当前发件账号
    let selectedAccountId = $state<number | null>(null);
    let accountDropdownOpen = $state(false);
    // 富文本编辑器组件实例引用
    let richEditor = $state<RichTextEditor>();
    let shouldShowAccountSelect = $derived(accountStore.isAllAccounts);
    let selectedSendAccount = $derived(
        accountStore.accounts.find((account) => account.id === selectedAccountId),
    );

    function hasAccount(accountId: number | null | undefined): accountId is number {
        return (
            typeof accountId === "number" &&
            accountStore.accounts.some((account) => account.id === accountId)
        );
    }

    function defaultSendAccountId(preferredAccountId?: number) {
        if (hasAccount(preferredAccountId)) {
            return preferredAccountId;
        }
        if (hasAccount(accountStore.lastConcreteAccountId)) {
            return accountStore.lastConcreteAccountId;
        }
        if (hasAccount(accountStore.activeAccountId)) {
            return accountStore.activeAccountId;
        }
        return accountStore.accounts[0]?.id ?? null;
    }

    function openWithAccount(preferredAccountId?: number) {
        selectedAccountId = defaultSendAccountId(preferredAccountId);
        accountDropdownOpen = false;
        open = true;
    }

    function selectSendAccount(accountId: number) {
        selectedAccountId = accountId;
        accountDropdownOpen = false;
    }

    function parseRecipients(value: string) {
        return value
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean);
    }

    // ==================== 邮件发送逻辑 ====================

    /**
     * 处理发送邮件操作
     *
     * 验证并发送当前编辑的邮件。流程如下：
     * 1. 检查是否有活跃账户
     * 2. 设置发送中状态（禁用按钮）
     * 3. 解析收件人列表（逗号分隔 → 数组）
     * 4. 调用后端 sendEmail 命令发送邮件
     * 5. 成功：关闭模态框并清空表单
     * 6. 失败：显示错误信息
     *
     * 注意：
     * - to 字段按逗号分隔并去除空白，生成收件人数组
     * - cc 字段如果非空，同样按逗号分隔生成抄送数组
     * - bcc（密送）当前为空数组，预留扩展
     * - 正文优先使用 HTML 格式，如果编辑器无法提供 HTML 则用 pre 标签包裹纯文本
     */
    async function handleSend() {
        const sendAccountId = accountStore.isAllAccounts
            ? selectedAccountId
            : accountStore.activeAccountId;
        // 没有可用发送账户，无法发送
        if (!sendAccountId) return;
        const toRecipients = parseRecipients(to);
        const ccRecipients = parseRecipients(cc);
        const trimmedSubject = subject.trim();
        const bodyText = richEditor?.getText() || "";
        const trimmedBodyText = bodyText.trim();
        if (toRecipients.length === 0) {
            error = t.email.sendRequiresRecipient;
            return;
        }
        if (!trimmedSubject) {
            error = t.email.sendRequiresSubject;
            return;
        }
        if (!trimmedBodyText) {
            error = t.email.sendRequiresBody;
            return;
        }
        // 进入发送中状态
        sending = true;
        // 清空之前的错误信息
        error = "";
        try {
            // 调用后端发送邮件命令
            const result = await commands.sendEmail({
                // 实际发送账户 ID：所有账号视图下来自发件账号选择器，否则来自当前活跃账号
                account_id: sendAccountId,
                // 收件人列表：将逗号分隔的字符串转为数组
                to: toRecipients,
                // 抄送列表：如果 cc 非空则解析，否则为空数组
                cc: ccRecipients,
                // 密送列表：当前预留为空
                bcc: [],
                // 邮件主题
                subject: trimmedSubject,
                // HTML 格式正文：优先使用编辑器的 HTML 输出
                // 如果编辑器无法提供 HTML，则用 <pre> 标签包裹纯文本
                body_html:
                    richEditor?.getHtml() ||
                    `<pre style="white-space:pre-wrap">${bodyText}</pre>`,
                // 纯文本正文：作为备用格式
                body_text: bodyText,
            });
            // 检查发送结果
            if (result.status === "error") {
                // 发送失败：显示后端返回的错误信息
                error = result.error.message as string;
            } else {
                // 发送成功：关闭模态框并重置表单
                close();
            }
        } catch (e: unknown) {
            error = e instanceof Error ? e.message : String(e);
        } finally {
            // 无论成功或失败，都结束发送中状态
            sending = false;
        }
    }

    /**
     * 关闭模态框并重置表单
     *
     * 将所有表单状态恢复到初始值，清除编辑器内容。
     * 在发送成功或用户手动关闭时调用。
     */
    function close() {
        open = false;
        to = "";
        cc = "";
        subject = "";
        error = "";
        selectedAccountId = null;
        accountDropdownOpen = false;
        // 清空富文本编辑器内容
        richEditor?.clear();
    }

    // ==================== 公开方法（供外部调用） ====================

    /**
     * 打开空白写邮件窗口
     *
     * 由以下场景调用：
     * - 侧边栏"写邮件"按钮点击
     * - 系统托盘"写邮件"菜单项点击
     * - 快捷键触发
     *
     * 通过 bind:this 暴露给父组件使用
     */
    export function show(options: { accountId?: number } = {}) {
        openWithAccount(options.accountId);
    }

    /**
     * 打开回复邮件窗口
     *
     * 预填充回复所需的字段：
     * - 收件人：原始邮件的发件人
     * - 主题：添加 "Re: " 前缀（自动去除已有的 Re:/Fwd: 前缀避免重复）
     * - 正文：引用原始邮件内容
     *
     * 由 EmailDetail 组件的"回复"按钮调用
     *
     * @param replyTo - 回复目标邮箱地址（原始发件人）
     * @param replySubject - 原始邮件主题
     * @param replyBody - 原始邮件正文（作为引用内容）
     */
    export function showReply(
        replyTo: string,
        replySubject: string,
        replyBody: string,
        accountId?: number,
    ) {
        openWithAccount(accountId);
        // 设置收件人为原始发件人
        to = replyTo;
        // 设置主题为 "Re: " + 原始主题（去除已有的 Re:/Fwd: 前缀）
        subject = `Re: ${replySubject.replace(/^(Re|Fwd):\s*/i, "")}`;
        // 在编辑器中预填充原始正文作为引用
        richEditor?.setContent(`<p>${replyBody}</p>`);
    }

    /**
     * 打开转发邮件窗口
     *
     * 预填充转发所需的字段：
     * - 收件人：留空（需要用户填写转发目标）
     * - 主题：添加 "Fwd: " 前缀（自动去除已有的 Re:/Fwd: 前缀避免重复）
     * - 正文：引用原始邮件内容
     *
     * 由 EmailDetail 组件的"转发"按钮调用
     *
     * @param fwdSubject - 原始邮件主题
     * @param fwdBody - 原始邮件正文（作为引用内容）
     */
    export function showForward(
        fwdSubject: string,
        fwdBody: string,
        accountId?: number,
    ) {
        openWithAccount(accountId);
        // 设置主题为 "Fwd: " + 原始主题（去除已有的 Re:/Fwd: 前缀）
        subject = `Fwd: ${fwdSubject.replace(/^(Re|Fwd):\s*/i, "")}`;
        // 在编辑器中预填充原始正文作为引用
        richEditor?.setContent(`<p>${fwdBody}</p>`);
    }
</script>

<!-- ==================== 模板渲染 ==================== -->

{#if open}
    <!--
      模态框遮罩层 + 定位容器

      类名说明：
      - fixed inset-0：固定定位，覆盖整个视口
      - z-50：高层级 z-index，确保模态框在最上层
      - flex items-end justify-end：内容靠右下角对齐
      - p-6：外边距

      交互行为：
      - 点击遮罩层（非模态框区域）关闭模态框
      - Svelte a11y 忽略注释：因为点击区域有视觉反馈
    -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-end justify-end p-6"
        onclick={(e) => e.target === e.currentTarget && close()}
    >
        <!--
          模态框主体容器

          类名说明：
          - h-[70vh]：高度为视口的 70%
          - w-140：宽度 35rem（约 560px）
          - flex / flex-col：垂直弹性布局
          - overflow-hidden：隐藏溢出内容
          - rounded-xl：大圆角
          - border border-border：边框
          - bg-card：卡片背景色
          - shadow-2xl：超大阴影
          - backdrop-blur-md：毛玻璃模糊效果
        -->
        <div
            data-testid="compose-modal"
            class="flex h-[70vh] w-140 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl backdrop-blur-md"
        >
            <!-- ==================== 标题栏 ==================== -->

            <!--
              模态框标题栏
              包含标题文字和关闭按钮
            -->
            <div
                class="flex items-center justify-between border-b border-border px-4 py-3"
            >
                <!-- 标题：使用国际化的"写邮件"文本 -->
                <h3 class="text-sm font-semibold text-foreground">
                    {t.sidebar.compose}
                </h3>
                <!-- 关闭按钮 -->
                <button
                    data-testid="compose-close-button"
                    class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                    aria-label="Close"
                >
                    <X size={16} />
                </button>
            </div>

            <!-- ==================== 邮件字段区域 ==================== -->

            <!--
              邮件字段输入区域
              包含：收件人（To）、抄送（CC）、主题（Subject）
              每个字段占一行，左侧标签 + 右侧输入框
            -->
            <div class="border-b border-border">
                {#if shouldShowAccountSelect}
                    <div class="flex items-center border-b border-border px-4">
                        <span class="w-14 shrink-0 text-sm text-muted-foreground"
                            >发件</span
                        >
                        <div class="relative min-w-0 flex-1 py-1">
                            <button
                                type="button"
                                data-testid="compose-account-select"
                                data-value={selectedAccountId}
                                aria-haspopup="listbox"
                                aria-expanded={accountDropdownOpen}
                                class="flex h-8 w-full items-center justify-between gap-2 rounded-md border border-transparent bg-transparent px-0 text-left text-sm text-foreground outline-none transition-colors hover:bg-glass-hover hover:px-2 focus:border-border focus:bg-glass-hover focus:px-2"
                                onclick={() =>
                                    (accountDropdownOpen = !accountDropdownOpen)}
                            >
                                <span class="truncate">
                                    {selectedSendAccount?.display_name ||
                                        selectedSendAccount?.email ||
                                        "选择发件账号"}
                                </span>
                                <ChevronDown
                                    size={14}
                                    class="shrink-0 text-muted-foreground"
                                />
                            </button>

                            {#if accountDropdownOpen}
                                <div
                                    data-testid="compose-account-options"
                                    role="listbox"
                                    class="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-lg border border-border bg-card shadow-xl"
                                >
                                    {#each accountStore.accounts as account}
                                        <button
                                            type="button"
                                            role="option"
                                            aria-selected={account.id ===
                                                selectedAccountId}
                                            data-testid={`compose-account-option-${account.id}`}
                                            class="flex w-full flex-col px-3 py-2 text-left text-sm transition-colors hover:bg-glass-hover {account.id ===
                                            selectedAccountId
                                                ? 'bg-primary/10 text-primary'
                                                : 'text-foreground'}"
                                            onclick={() =>
                                                selectSendAccount(account.id)}
                                        >
                                            <span class="truncate font-medium">
                                                {account.display_name ||
                                                    account.email}
                                            </span>
                                            {#if account.display_name}
                                                <span
                                                    class="truncate text-xs text-muted-foreground"
                                                >
                                                    {account.email}
                                                </span>
                                            {/if}
                                        </button>
                                    {/each}
                                </div>
                            {/if}
                        </div>
                    </div>
                {/if}
                <!-- 收件人输入行 -->
                <div class="flex items-center border-b border-border px-4">
                    <!-- 字段标签：固定宽度 3.5rem -->
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.to}</span
                    >
                    <!-- 收件人输入框 -->
                    <input
                        data-testid="compose-to-input"
                        type="text"
                        bind:value={to}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                        placeholder="email@example.com"
                    />
                </div>

                <!-- 抄送输入行 -->
                <div class="flex items-center border-b border-border px-4">
                    <!-- 字段标签 -->
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.cc}</span
                    >
                    <!-- 抄送输入框 -->
                    <input
                        data-testid="compose-cc-input"
                        type="text"
                        bind:value={cc}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                    />
                </div>

                <!-- 主题输入行 -->
                <div class="flex items-center px-4">
                    <!-- 字段标签 -->
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.subject}</span
                    >
                    <!-- 主题输入框 -->
                    <input
                        data-testid="compose-subject-input"
                        type="text"
                        bind:value={subject}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                    />
                </div>
            </div>

            <!-- ==================== 邮件正文编辑器 ==================== -->

            <!--
              富文本编辑器区域
              使用 RichTextEditor 子组件实现邮件正文编辑
              占据模态框的剩余空间（flex-1）
              overflow-hidden 防止编辑器溢出
            -->
            <div
                data-testid="compose-body-editor"
                class="flex-1 overflow-hidden"
            >
                <!--
                  富文本编辑器组件
                  bind:this 将组件实例绑定到 richEditor 变量
                  以便在脚本中调用编辑器的方法（getHtml、setText、clear 等）
                -->
                <RichTextEditor bind:this={richEditor} />
            </div>

            <!-- ==================== 错误提示 ==================== -->

            <!--
              错误信息显示区域
              仅在发送失败时显示（error 非空）
              使用红色文字（destructive）提示错误
            -->
            {#if error}
                <div class="px-4 py-2 text-sm text-destructive">{error}</div>
            {/if}

            <!-- ==================== 底部操作栏 ==================== -->

            <!--
              底部操作栏
              包含发送按钮和取消按钮
            -->
            <div
                class="flex items-center justify-between border-t border-border px-4 py-3"
            >
                <!--
                  发送按钮
                  - 使用渐变背景（compose-btn 自定义样式）
                  - disabled 条件：发送中、收件人为空、主题为空
                  - 显示发送中/发送文本（根据状态切换）
                -->
                <button
                    data-testid="compose-send-button"
                    class="compose-btn rounded-lg px-5 py-2 text-sm font-medium text-white transition-all disabled:opacity-50"
                    onclick={handleSend}
                    disabled={sending ||
                        (accountStore.isAllAccounts && !selectedAccountId)}
                >
                    {sending ? t.email.loading : t.email.send}
                </button>

                <!-- 取消按钮：关闭模态框 -->
                <button
                    class="rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                >
                    {t.common.cancel}
                </button>
            </div>
        </div>
    </div>
{/if}
