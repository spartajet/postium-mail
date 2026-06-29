<!--
  Postium Mail - 邮件详情组件
  EmailDetail.svelte

  本组件是邮件详情视图，作为应用主页面右侧面板的核心组件。

  ==================== 功能说明 ====================
  1. 显示选中邮件的完整内容（主题、发件人、收件人、正文、附件等）
  2. 支持 AI 智能摘要（自动加载邮件摘要信息）
  3. 提供邮件操作按钮：回复、转发、星标、归档、删除
  4. 支持 AI 操作：智能回复、摘要、翻译、任务提取
  5. 显示附件列表（带文件类型图标和大小信息）
  6. 安全渲染 HTML 邮件正文（使用 DOMPurify 防止 XSS）

  ==================== 在架构中的位置 ====================
  位于 src/lib/components/email/ 目录下，是邮件功能模块的子组件。
  被引用于：
    - +page.svelte（主页面的右侧面板）

  依赖的 Store：
    - emailStore：获取选中邮件数据、执行邮件操作
    - i18nStore：国际化翻译

  依赖的子组件：
    - ComposeModal（通过 Context 引用）：回复、转发时打开写邮件模态框

  依赖的工具库：
    - DOMPurify：HTML 内容净化，防止 XSS 攻击

  ==================== 数据流说明 ====================
  1. emailState.selectedEmail 变化 → 组件重新渲染邮件内容
  2. $effect 自动触发 → 加载 AI 摘要（带 race condition 防护）
  3. 用户点击操作按钮 → 调用 emailState 或 ComposeModal 方法
-->

<script lang="ts">
    // ==================== 导入依赖 ====================

    // 导入 Svelte Context API，用于获取父组件共享的模态框引用
    import { getContext } from "svelte";
    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入邮件状态管理，用于获取选中邮件和执行邮件操作
    import { getEmailState } from "$lib/stores/email.svelte";
    // 导入账号状态管理，用于识别当前邮件所属账号邮箱
    import { getAccountState } from "$lib/stores/account.svelte";
    // 导入写邮件模态框类型定义（用于 Context 引用类型）
    import type ComposeModal from "./ComposeModal.svelte";
    import type { AttachmentDto } from "$lib/bindings";
    // 导入 HTML 净化库，防止 XSS 攻击
    // 邮件正文可能包含恶意脚本，渲染前必须经过净化处理
    import DOMPurify from "dompurify";
    import { save } from "@tauri-apps/plugin-dialog";
    // 导入图标组件（Lucide 图标库）
    import {
        Archive, // 归档图标
        ChevronUp, // 上箭头（导航：上一封邮件）
        ChevronDown, // 下箭头（导航：下一封邮件）
        Download, // 下载图标
        ExternalLink, // 打开图标
        File, // 通用文件图标
        FileArchive, // 压缩包图标
        FileAudio, // 音频图标
        FileImage, // 图片图标
        FileText, // 文档图标
        FileVideo, // 视频图标
        FolderDown, // 保存图标
        Forward, // 转发图标
        Layers, // AI 图标（AI 摘要卡片和操作栏）
        Mail, // 邮件图标（空状态占位）
        Reply, // 回复图标
        Star, // 星标图标
        Trash2, // 删除图标
        Paperclip, // 附件图标
    } from "lucide-svelte";

    // ==================== Context 引用 ====================

    // 写邮件模态框的 Context 键（与 +layout.svelte 中设置的键一致）
    // 使用 Symbol.for 创建全局唯一键，确保跨模块引用相同
    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal");
    // 从 Context 中获取写邮件模态框的引用函数
    // 返回值为 ComposeModal 实例或 undefined
    const getComposeModal =
        getContext<() => ComposeModal | undefined>(COMPOSE_MODAL_KEY);

    // ==================== 状态初始化 ====================

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);
    // 获取邮件状态实例（包含选中邮件、邮件列表等）
    const emailState = getEmailState();
    // 获取账号状态实例（用于从 account_id 反查账号邮箱）
    const accountState = getAccountState();

    // AI 摘要文本内容
    let aiSummary = $state("");
    // AI 摘要加载状态标志
    let isLoadingSummary = $state(false);
    // 多收件人列表是否展开
    let recipientsExpanded = $state(false);
    // 记录当前邮件 ID，用于切换邮件时重置展开状态
    let lastRecipientEmailId = $state<number | null>(null);

    // ==================== 工具函数 ====================

    /**
     * 获取发件人名称的首字母
     *
     * 用于在头像圆圈中显示发件人的首字母缩写。
     * 例如："张三" → "张"，"John" → "J"
     *
     * @param name - 发件人名称字符串
     * @returns 大写的首字母
     */
    function getInitials(name: string): string {
        return name.charAt(0).toUpperCase();
    }

    /**
     * 格式化邮件时间为完整日期时间字符串
     *
     * 将 Unix 时间戳转换为本地化的完整日期时间格式，
     * 包含年、月、日、时、分。
     * 例如："2024/01/15, 14:30"
     *
     * @param timestamp - Unix 时间戳（秒）
     * @returns 本地化的完整日期时间字符串
     */
    function formatFullDate(timestamp: number): string {
        return new Date(timestamp * 1000).toLocaleString(undefined, {
            year: "numeric",
            month: "2-digit",
            day: "2-digit",
            hour: "2-digit",
            minute: "2-digit",
        });
    }

    function splitEmailList(value: string | null | undefined): string[] {
        return (value ?? "")
            .split(/[;,]/)
            .map((item) => item.trim())
            .filter(Boolean);
    }

    const recipientList = $derived(
        splitEmailList(emailState.selectedEmail?.recipient_emails),
    );

    const currentAccountEmail = $derived(
        accountState.accounts.find(
            (account) => account.id === emailState.selectedEmail?.account_id,
        )?.email ?? "",
    );

    const recipientSummary = $derived.by(() => {
        if (recipientList.length === 0) return "";
        if (recipientList.length === 1) return recipientList[0]!;

        const accountEmail = currentAccountEmail.toLowerCase();
        const accountRecipient = recipientList.find(
            (recipient) => recipient.toLowerCase() === accountEmail,
        );
        const primaryRecipient = accountRecipient ?? recipientList[0]!;
        const otherRecipients = t.email.otherRecipients.replace(
            "{count}",
            String(recipientList.length - 1),
        );
        return `${primaryRecipient} ${otherRecipients}`;
    });

    $effect(() => {
        const emailId = emailState.selectedEmail?.id ?? null;
        if (emailId !== lastRecipientEmailId) {
            lastRecipientEmailId = emailId;
            recipientsExpanded = false;
        }
    });

    /**
     * 手动加载 AI 摘要
     *
     * 异步加载当前邮件的 AI 摘要内容。
     * 当前为模拟实现，使用 setTimeout 模拟网络请求延迟。
     * TODO: 替换为真实的后端 AI 摘要 API 调用
     */
    async function loadAISummary() {
        // 没有选中的邮件，不执行
        if (!emailState.selectedEmail) return;
        // 进入加载状态
        isLoadingSummary = true;
        try {
            // 模拟 800ms 网络延迟
            await new Promise((resolve) => setTimeout(resolve, 800));
            // 模拟 AI 摘要结果
            aiSummary =
                "• 邮件主要内容摘要\n• 需要关注的关键点\n• 建议的后续行动";
        } finally {
            // 无论成功或失败，都结束加载状态
            isLoadingSummary = false;
        }
    }

    /**
     * 切换邮件星标状态
     *
     * 将当前选中邮件的星标状态进行反转（已星标 → 取消，未星标 → 添加）。
     * 操作委托给 emailState 处理，确保全局状态一致。
     */
    async function handleToggleStar() {
        if (emailState.selectedEmail) {
            await emailState.toggleStar(emailState.selectedEmail.id);
        }
    }

    /**
     * 归档当前选中邮件
     *
     * 将当前邮件移出当前分类。操作委托给 emailState 处理。
     */
    async function handleArchive() {
        if (emailState.selectedEmail) {
            await emailState.archiveEmail(emailState.selectedEmail.id);
        }
    }

    /**
     * 删除当前选中邮件
     *
     * 第一阶段将当前邮件移至服务商 Trash 文件夹，不执行永久删除。
     * 操作委托给 emailState 处理。
     */
    async function handleDelete() {
        if (emailState.selectedEmail) {
            await emailState.deleteEmails([emailState.selectedEmail.id]);
        }
    }

    // ==================== AI 摘要自动加载 ====================

    // H-03: 使用 stale flag（递增 ID）防止 race condition
    // 当用户快速切换邮件时，旧的摘要请求可能在新请求之后返回，
    // 导致显示错误的摘要内容。通过递增 requestId 并在结果返回时比对，
    // 确保只有最新的请求结果会被使用。

    /**
     * 摘要请求的唯一递增 ID
     * 每次触发新的摘要加载时递增，用于标识最新的请求
     */
    let summaryRequestId = 0;

    // 当选中邮件变化时，自动重新加载 AI 摘要
    // $effect 会自动追踪 emailState.selectedEmail 的变化
    $effect(() => {
        // 获取当前选中的邮件
        const email = emailState.selectedEmail;
        if (email) {
            // 清空旧的摘要内容
            aiSummary = "";
            // 递增请求 ID，标记这是最新的请求
            const currentId = ++summaryRequestId;
            // 进入加载状态
            isLoadingSummary = true;
            // 模拟异步摘要请求
            new Promise((resolve) => setTimeout(resolve, 800)).then(() => {
                // 只有当 currentId 仍然是最新 ID 时才更新状态
                // 如果用户已经切换到其他邮件，summaryRequestId 会大于 currentId
                // 此时忽略过期的请求结果
                if (currentId === summaryRequestId) {
                    // 模拟 AI 摘要内容
                    aiSummary =
                        "• 邮件主要内容摘要\n• 需要关注的关键点\n• 建议的后续行动";
                    // 结束加载状态
                    isLoadingSummary = false;
                }
            });
        }
    });

    // ==================== AI 操作配置 ====================

    /**
     * AI 操作按钮列表
     *
     * 派生状态，根据当前语言自动更新按钮文本。
     * 包含以下操作：
     * - smartReply：智能回复（AI 生成回复建议）
     * - summary：邮件摘要（AI 总结邮件要点）
     * - translate：翻译（AI 翻译邮件内容）
     * - tasks：任务提取（AI 从邮件中提取待办事项）
     */
    const aiActions = $derived([
        { id: "reply", label: t.ai.smartReply },
        { id: "summary", label: t.ai.summary },
        { id: "translate", label: t.ai.translate },
        { id: "tasks", label: t.ai.tasks },
    ]);

    const sanitizedHtml = $derived(
        emailState.resolvedBodyHtml
            ? DOMPurify.sanitize(emailState.resolvedBodyHtml)
            : emailState.selectedEmail?.body_html
              ? DOMPurify.sanitize(emailState.selectedEmail.body_html)
              : "",
    );

    function formatFileSize(size: number): string {
        if (size < 1024) return `${size} B`;
        if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
        return `${(size / 1024 / 1024).toFixed(1)} MB`;
    }

    function isLargeAttachment(attachment: AttachmentDto): boolean {
        return attachment.size > 10 * 1024 * 1024;
    }

    function attachmentIcon(attachment: AttachmentDto) {
        const type = attachment.content_type;
        if (type.startsWith("image/")) return FileImage;
        if (type.startsWith("audio/")) return FileAudio;
        if (type.startsWith("video/")) return FileVideo;
        if (type.includes("zip") || type.includes("rar") || type.includes("7z")) {
            return FileArchive;
        }
        if (type.startsWith("text/") || type.includes("pdf")) return FileText;
        return File;
    }

    async function handleDownload(attachment: AttachmentDto) {
        if (isLargeAttachment(attachment)) {
            await handleSaveAs(attachment);
            return;
        }
        await emailState.downloadAttachment(attachment.id);
    }

    async function handleSaveAs(attachment: AttachmentDto) {
        const targetPath = await save({
            defaultPath: attachment.filename,
        });
        if (!targetPath) return;
        await emailState.saveAttachmentAs(attachment.id, targetPath);
    }

    async function handleOpen(attachment: AttachmentDto) {
        await emailState.openAttachment(attachment.id);
    }
</script>

<!-- ==================== 模板渲染 ====================-->

<!--
  邮件详情面板容器

  类名说明：
  - detail-panel：自定义面板样式
  - flex / flex-col：垂直方向弹性布局
  - h-full：占满父容器高度
  - flex-1：占据剩余空间（在主页面中与左侧 EmailList 分配空间）
  - bg-background：应用主题背景色
-->
<div
    data-testid={emailState.selectedEmail
        ? "email-detail"
        : "email-detail-empty"}
    class="detail-panel flex h-full flex-1 flex-col bg-background"
>
    {#if emailState.selectedEmail}
        <!-- ==================== 邮件详情内容 ==================== -->

        <!--
      顶部导航工具栏
      包含上/下封邮件的导航按钮
    -->
        <div
            class="flex items-center justify-between border-b border-border px-5 py-2"
        >
            <div class="flex items-center gap-1">
                <!-- 上一封邮件按钮 -->
                <button
                    class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    title={t.email.from}
                >
                    <ChevronUp size={20} />
                </button>
                <!-- 下一封邮件按钮 -->
                <button
                    class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    title={t.email.to}
                >
                    <ChevronDown size={20} />
                </button>
            </div>
        </div>

        <!--
      邮件主题区域
        显示邮件的完整主题行，字体较大、加粗
    -->
        <div class="border-b border-border px-5 py-4">
            <h2
                data-testid="email-subject"
                class="text-xl font-semibold text-foreground"
            >
                {emailState.selectedEmail.subject || "(No Subject)"}
            </h2>
        </div>

        <!--
      发件人信息区域
      包含：头像、发件人名称、邮箱地址、收件人、抄送、发送时间
    -->
        <div class="flex items-center gap-3 border-b border-border px-5 py-3">
            <!--
        发件人头像
        显示发件人名称的首字母，圆形背景
        bg-primary/15：主题色 15% 透明度背景
        text-primary：主题色文字
      -->
            <div
                class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-primary/15 text-sm font-semibold text-primary"
            >
                {getInitials(
                    emailState.selectedEmail.sender_name ||
                        emailState.selectedEmail.sender_email,
                )}
            </div>

            <!-- 发件人详情：名称、邮箱、收件人 -->
            <div class="min-w-0 flex-1">
                <!-- 第一行：发件人名称 + 邮箱地址 -->
                <div class="flex items-center gap-2">
                    <!-- 发件人显示名称 -->
                    <span
                        data-testid="email-sender"
                        class="text-sm font-medium text-foreground"
                    >
                        {emailState.selectedEmail.sender_name ||
                            emailState.selectedEmail.sender_email}
                    </span>
                    <!-- 发件人邮箱地址（尖括号包裹） -->
                    <span class="truncate text-xs text-muted-foreground">
                        &lt;{emailState.selectedEmail.sender_email}&gt;
                    </span>
                </div>
                <!-- 第二行：收件人 + 抄送信息 -->
                <div
                    class="mt-0.5 flex min-w-0 flex-wrap items-start gap-x-1 gap-y-1 text-xs text-muted-foreground"
                >
                    <span class="shrink-0">{t.email.to}:</span>
                    {#if recipientList.length > 1 && !recipientsExpanded}
                        <span class="min-w-0 break-all">
                            {recipientSummary}
                        </span>
                    {:else}
                        <span class="min-w-0 break-all">
                            {#each recipientList as recipient, index}
                                <span>{recipient}</span>{#if index < recipientList.length - 1}, {/if}
                            {/each}
                        </span>
                    {/if}

                    {#if recipientList.length > 1}
                        <button
                            type="button"
                            class="flex h-4 w-4 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                            aria-label={recipientsExpanded
                                ? t.email.collapseRecipients
                                : t.email.expandRecipients}
                            title={recipientsExpanded
                                ? t.email.collapseRecipients
                                : t.email.expandRecipients}
                            onclick={() =>
                                (recipientsExpanded = !recipientsExpanded)}
                        >
                            {#if recipientsExpanded}
                                <ChevronUp size={14} />
                            {:else}
                                <ChevronDown size={14} />
                            {/if}
                        </button>
                    {/if}

                    <!-- 如果有抄送，显示抄送信息 -->
                    {#if emailState.selectedEmail.cc_emails}
                        <span class="shrink-0">&nbsp;|&nbsp; {t.email.cc}:</span>
                        <span class="min-w-0 break-all">
                            {emailState.selectedEmail.cc_emails}
                        </span>
                    {/if}
                </div>
            </div>

            <!-- 发送时间（完整日期格式） -->
            <span class="shrink-0 text-xs text-muted-foreground">
                {formatFullDate(emailState.selectedEmail.sent_at)}
            </span>
        </div>

        <!--
      可滚动内容区域
      包含：AI 摘要卡片、邮件正文
      使用 flex-1 overflow-y-auto 实现独立滚动
    -->
        <div data-testid="email-body-scroller" class="flex-1 overflow-y-auto">
            <!-- ==================== AI 摘要卡片 ==================== -->

            <!--
        AI 摘要卡片
        仅在摘要正在加载或已有摘要内容时显示
        边框使用主题色 20% 透明度，背景使用主题色 5% 透明度
      -->
            {#if isLoadingSummary || aiSummary}
                <div
                    class="ai-card mx-5 mt-4 rounded-lg border border-primary/20 bg-primary/5"
                >
                    <!-- 卡片标题：AI 图标 + "摘要" 标签 -->
                    <div class="flex items-center gap-2 px-4 py-2.5">
                        <Layers size={16} class="text-primary" />
                        <span class="text-xs font-semibold text-primary"
                            >{t.ai.summary}</span
                        >
                    </div>

                    <!-- 卡片内容区域 -->
                    <div class="border-t border-primary/10 px-4 py-3">
                        {#if isLoadingSummary}
                            <!--
                加载中状态
                显示三行骨架屏动画，模拟摘要文本
              -->
                            <div class="space-y-2">
                                <div
                                    class="skeleton skeleton-text"
                                    style="width: 90%"
                                ></div>
                                <div
                                    class="skeleton skeleton-text"
                                    style="width: 75%"
                                ></div>
                                <div
                                    class="skeleton skeleton-text"
                                    style="width: 60%"
                                ></div>
                            </div>
                        {:else}
                            <!--
                摘要内容
                将 AI 摘要文本按换行符分割为列表项
                每行前面的 "• " 前缀会被移除后重新渲染为无序列表
              -->
                            <ul class="space-y-1">
                                {#each aiSummary.split("\n") as line, i (i)}
                                    <li class="text-sm text-foreground/80">
                                        {line.replace("• ", "")}
                                    </li>
                                {/each}
                            </ul>
                        {/if}
                    </div>
                </div>
            {/if}

            <!-- ==================== 邮件正文 ==================== -->

            <!--
        邮件正文渲染区域
        优先使用 HTML 格式渲染（富文本邮件），否则降级为纯文本
        HTML 内容经过 DOMPurify 净化，防止 XSS 攻击
      -->
            <div data-testid="email-body" class="px-5 py-4">
                {#if emailState.selectedEmail.body_html}
                    <!--
            HTML 格式邮件正文
            使用 {@html} 指令渲染原始 HTML
            DOMPurify.sanitize() 会移除 <script>、onerror 等危险标签和属性
          -->
                    {@html sanitizedHtml}
                {:else}
                    <!--
            纯文本格式邮件正文
            使用 <pre> 标签保留原始换行和空格
            whitespace-pre-wrap：保留空白字符但允许自动换行
          -->
                    <pre
                        class="whitespace-pre-wrap text-sm leading-relaxed text-foreground/90">{emailState
                            .selectedEmail.body_text || ""}</pre>
                {/if}
            </div>

        </div>

        <!-- ==================== 附件列表 ==================== -->

        <!--
      附件区域
      仅当邮件包含附件时显示，固定在正文滚动区下方、操作按钮栏上方
      高度由内容自然撑开；附件较多时仅附件区域内部滚动
    -->
        {#if emailState.selectedEmail.attachments.length > 0}
            <section
                data-testid="email-attachments-bar"
                class="shrink-0 border-t border-border px-5 py-3"
                aria-label={t.email.attachments}
            >
                <!-- 附件标题栏：回形针图标 + "附件" 文本 -->
                <div
                    class="mb-2 flex items-center gap-2 text-sm font-medium text-muted-foreground"
                >
                    <Paperclip size={18} />
                    <span>{t.email.attachments}</span>
                </div>

                <!-- 附件文件卡片列表：窄屏单列，中等宽度两列，宽屏三列 -->
                <div class="max-h-48 overflow-y-auto pr-1">
                    <div
                        class="grid grid-cols-1 gap-2 sm:grid-cols-2 xl:grid-cols-3"
                    >
                        {#each emailState.selectedEmail.attachments as attachment (attachment.id)}
                            {@const Icon = attachmentIcon(attachment)}
                            {@const operating = emailState.attachmentOperatingIds.has(
                                attachment.id,
                            )}
                            {@const large = isLargeAttachment(attachment)}
                            <div
                                class="flex min-w-0 items-center gap-3 rounded-md border border-border bg-glass px-3 py-2 transition-colors hover:border-primary hover:bg-glass-hover"
                            >
                                <div
                                    class="flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary"
                                >
                                    <Icon size={20} />
                                </div>
                                <div class="min-w-0 flex-1">
                                    <div
                                        class="truncate text-sm font-medium text-foreground"
                                    >
                                        {attachment.filename}
                                    </div>
                                    <div
                                        class="text-xs text-muted-foreground"
                                    >
                                        {formatFileSize(attachment.size)}
                                        ·
                                        {attachment.is_cached
                                            ? t.email.attachmentCached
                                            : t.email.attachmentNotDownloaded}
                                    </div>
                                    {#if emailState.attachmentErrors[attachment.id]}
                                        <div
                                            class="mt-1 text-xs text-destructive"
                                        >
                                            {emailState.attachmentErrors[
                                                attachment.id
                                            ]}
                                        </div>
                                    {/if}
                                </div>
                                <div class="flex shrink-0 items-center gap-1">
                                    <button
                                        type="button"
                                        class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                                        title={large
                                            ? t.email.attachmentSave
                                            : t.email.attachmentDownload}
                                        aria-label={large
                                            ? t.email.attachmentSave
                                            : t.email.attachmentDownload}
                                        disabled={operating}
                                        onclick={() =>
                                            handleDownload(attachment)}
                                    >
                                        {#if large}
                                            <FolderDown size={16} />
                                        {:else}
                                            <Download size={16} />
                                        {/if}
                                    </button>
                                    {#if !large}
                                        <button
                                            type="button"
                                            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                                            title={t.email.attachmentOpen}
                                            aria-label={t.email.attachmentOpen}
                                            disabled={operating}
                                            onclick={() =>
                                                handleOpen(attachment)}
                                        >
                                            <ExternalLink size={16} />
                                        </button>
                                        <button
                                            type="button"
                                            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                                            title={t.email.attachmentSaveAs}
                                            aria-label={t.email
                                                .attachmentSaveAs}
                                            disabled={operating}
                                            onclick={() =>
                                                handleSaveAs(attachment)}
                                        >
                                            <FolderDown size={16} />
                                        </button>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    </div>
                </div>
            </section>
        {/if}

        <!-- ==================== 邮件操作按钮栏 ==================== -->

        <!--
      操作按钮栏
      包含常用的邮件操作按钮：回复、转发、星标、归档、删除
      固定在邮件详情底部
    -->
        <div class="flex items-center gap-1 border-t border-border px-5 py-2">
            <!--
        回复按钮
        点击后打开写邮件模态框，预填充收件人和主题（Re: 前缀）
      -->
            <button
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.email.reply}
                onclick={() => {
                    // 获取写邮件模态框实例
                    const modal = getComposeModal?.();
                    // 确保有选中的邮件和模态框引用
                    if (emailState.selectedEmail && modal) {
                        // 调用模态框的 showReply 方法，传入回复所需的邮件信息
                        modal.showReply(
                            emailState.selectedEmail.sender_email, // 回复给发件人
                            emailState.selectedEmail.subject || "", // 原始主题
                            emailState.selectedEmail.body_text || "", // 原始正文（作为引用）
                        );
                    }
                }}
            >
                <Reply size={18} />
            </button>

            <!--
        转发按钮
        点击后打开写邮件模态框，预填充主题（Fwd: 前缀）和原文
      -->
            <button
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.email.forward}
                onclick={() => {
                    // 获取写邮件模态框实例
                    const modal = getComposeModal?.();
                    // 确保有选中的邮件和模态框引用
                    if (emailState.selectedEmail && modal) {
                        // 调用模态框的 showForward 方法，传入转发所需的邮件信息
                        modal.showForward(
                            emailState.selectedEmail.subject || "", // 原始主题
                            emailState.selectedEmail.body_text || "", // 原始正文（作为引用）
                        );
                    }
                }}
            >
                <Forward size={18} />
            </button>

            <!--
        星标切换按钮
        已星标时显示黄色实心星，未星标时显示空心星
        悬停时变红色（删除警告色）
      -->
            <button
                data-testid="email-star-button"
                data-starred={emailState.selectedEmail.is_starred
                    ? "true"
                    : "false"}
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-glass-hover {emailState
                    .selectedEmail.is_starred
                    ? 'text-yellow-400'
                    : 'text-muted-foreground hover:text-foreground'}"
                title={t.email.star}
                onclick={handleToggleStar}
                disabled={emailState.selectedEmail
                    ? emailState.operatingIds.has(emailState.selectedEmail.id)
                    : false}
            >
                <Star
                    size={18}
                    class={emailState.selectedEmail.is_starred
                        ? "fill-current"
                        : ""}
                />
            </button>

            <!-- 归档按钮 -->
            <button
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.email.archive}
                onclick={handleArchive}
                disabled={emailState.selectedEmail
                    ? emailState.operatingIds.has(emailState.selectedEmail.id)
                    : false}
            >
                <Archive size={18} />
            </button>

            <!--
        删除按钮
        悬停时变红色（destructive 警告色），提示危险操作
      -->
            <button
                data-testid="email-delete-button"
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-destructive"
                title={t.common.delete}
                onclick={handleDelete}
                disabled={emailState.selectedEmail
                    ? emailState.operatingIds.has(emailState.selectedEmail.id)
                    : false}
            >
                <Trash2 size={18} />
            </button>
        </div>

        <!-- ==================== AI 操作栏 ==================== -->

        <!--
      AI 功能操作栏
      包含 AI 提供商标签和多个 AI 功能按钮
      按钮包括：智能回复、摘要、翻译、任务提取
    -->
        <div class="flex items-center gap-3 border-t border-border px-5 py-2">
            <!-- AI 提供商标签 -->
            <div
                class="flex items-center gap-1.5 text-xs text-muted-foreground"
            >
                <Layers size={14} class="text-primary" />
                <span>AI {t.ai.provider}</span>
            </div>

            <!-- AI 功能按钮列表 -->
            <div class="flex items-center gap-1">
                {#each aiActions as action, i (i)}
                    <!--
            单个 AI 操作按钮
            悬停时显示主题色背景和文字
            TODO: 绑定实际的 AI 功能处理逻辑
          -->
                    <button
                        class="ai-action-btn flex items-center gap-1 rounded-md px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-primary/10 hover:text-primary"
                    >
                        <span>{action.label}</span>
                    </button>
                {/each}
            </div>
        </div>
    {:else}
        <!-- ==================== 空状态 ==================== -->

        <!--
      未选中邮件时的空状态视图
      显示邮件图标和提示文本，引导用户选择邮件
    -->
        <div class="flex h-full flex-col items-center justify-center py-12">
            <!-- 邮件图标（半透明，占位装饰） -->
            <Mail size={48} class="mb-4 opacity-30" strokeWidth={1} />
            <!-- 提示文本 -->
            <h3 class="text-lg text-foreground/70">
                {t.email.noEmailSelected}
            </h3>
        </div>
    {/if}
</div>
