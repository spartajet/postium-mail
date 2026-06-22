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
    // 导入写邮件模态框类型定义（用于 Context 引用类型）
    import type ComposeModal from "./ComposeModal.svelte";
    // 导入 HTML 净化库，防止 XSS 攻击
    // 邮件正文可能包含恶意脚本，渲染前必须经过净化处理
    import DOMPurify from "dompurify";
    // 导入图标组件（Lucide 图标库）
    import {
        ChevronUp, // 上箭头（导航：上一封邮件）
        ChevronDown, // 下箭头（导航：下一封邮件）
        Layers, // AI 图标（AI 摘要卡片和操作栏）
        Paperclip, // 附件图标
        Reply, // 回复图标
        Forward, // 转发图标
        Star, // 星标图标
        Archive, // 归档图标
        Trash2, // 删除图标
        Mail, // 邮件图标（空状态占位）
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

    // AI 摘要文本内容
    let aiSummary = $state("");
    // AI 摘要加载状态标志
    let isLoadingSummary = $state(false);

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
    function handleToggleStar() {
        if (emailState.selectedEmail) {
            emailState.toggleStar(emailState.selectedEmail.id);
        }
    }

    /**
     * 删除当前选中邮件
     *
     * 将当前邮件移至废纸篓（或永久删除，取决于后端实现）。
     * 操作委托给 emailState 处理。
     */
    function handleDelete() {
        if (emailState.selectedEmail) {
            emailState.deleteEmails([emailState.selectedEmail.id]);
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

    // ==================== 附件相关函数 ====================

    /**
     * 根据文件扩展名获取对应的颜色
     *
     * 为不同类型的附件显示不同的标识颜色：
     * - PDF：红色 (#F40F02)
     * - 图片（jpg/png/gif 等）：紫色 (#9C27B0)
     * - 视频（mp4/avi 等）：橙色 (#FF9800)
     * - 音频（mp3/wav 等）：蓝色 (#2196F3)
     * - Word 文档：深蓝色 (#2B579A)
     * - Excel 表格：绿色 (#217346)
     * - PPT 演示：红棕色 (#D24726)
     * - 压缩文件：绿色 (#4CAF50)
     * - 其他：灰色 (#757575)
     *
     * @param filename - 文件名（含扩展名）
     * @returns 十六进制颜色值
     */
    function getFileExtensionColor(filename: string): string {
        // 提取文件扩展名（小写）
        const ext = filename.split(".").pop()?.toLowerCase() || "";
        // 文件扩展名 → 颜色映射表
        const colors: Record<string, string> = {
            pdf: "#F40F02", // PDF 文档 - 红色
            jpg: "#9C27B0",
            jpeg: "#9C27B0",
            png: "#9C27B0", // 图片 - 紫色
            gif: "#9C27B0",
            svg: "#9C27B0",
            webp: "#9C27B0",
            mp4: "#FF9800",
            avi: "#FF9800",
            mov: "#FF9800",
            mkv: "#FF9800", // 视频 - 橙色
            mp3: "#2196F3",
            wav: "#2196F3",
            flac: "#2196F3",
            aac: "#2196F3", // 音频 - 蓝色
            doc: "#2B579A",
            docx: "#2B579A", // Word 文档 - 深蓝
            xls: "#217346",
            xlsx: "#217346", // Excel 表格 - 绿色
            ppt: "#D24726",
            pptx: "#D24726", // PPT 演示 - 红棕
            txt: "#757575", // 纯文本 - 灰色
            zip: "#4CAF50",
            rar: "#4CAF50",
            "7z": "#4CAF50", // 压缩文件 - 绿色
        };
        // 返回对应颜色或默认灰色
        return colors[ext] || "#757575";
    }

    /**
     * 获取文件扩展名的大写标签
     *
     * 用于在附件图标中显示文件类型缩写。
     * 例如："document.pdf" → "PDF"，"image.png" → "PNG"
     *
     * @param filename - 文件名（含扩展名）
     * @returns 大写的扩展名字符串，无扩展名时返回 "FILE"
     */
    function getFileExtensionLabel(filename: string): string {
        return filename.split(".").pop()?.toUpperCase() || "FILE";
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
                <div class="mt-0.5 text-xs text-muted-foreground">
                    {t.email.to}: {emailState.selectedEmail.recipient_emails}
                    <!-- 如果有抄送，显示抄送信息 -->
                    {#if emailState.selectedEmail.cc_emails}
                        &nbsp;|&nbsp; {t.email.cc}: {emailState.selectedEmail
                            .cc_emails}
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
      包含：AI 摘要卡片、邮件正文、附件列表
      使用 flex-1 overflow-y-auto 实现独立滚动
    -->
        <div class="flex-1 overflow-y-auto">
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
                    {@html DOMPurify.sanitize(
                        emailState.selectedEmail.body_html,
                    )}
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

            <!-- ==================== 附件列表 ==================== -->

            <!--
        附件区域
        仅当邮件包含附件时显示 (has_attachments 为 true)
        包含附件标题栏和附件文件卡片列表
      -->
            {#if emailState.selectedEmail.has_attachments}
                <div class="border-t border-border px-5 py-4">
                    <!-- 附件标题栏：回形针图标 + "附件" 文本 -->
                    <div
                        class="mb-3 flex items-center gap-2 text-sm font-medium text-muted-foreground"
                    >
                        <Paperclip size={18} />
                        <span>{t.email.attachments}</span>
                    </div>

                    <!-- 附件文件卡片列表（flex-wrap 支持多行排列） -->
                    <div class="flex flex-wrap gap-3">
                        <!--
              单个附件卡片（当前为占位符，显示示例 PDF 附件）
              TODO: 改为遍历真实附件列表渲染

              结构说明：
              - 左侧：文件类型图标（彩色圆角方块，显示扩展名）
              - 右侧：文件名 + 文件大小
              - 悬停效果：边框变主题色
            -->
                        <div
                            class="attachment-item flex items-center gap-2 rounded-lg border border-border bg-glass px-3 py-2 transition-colors hover:border-primary hover:bg-glass-hover"
                        >
                            <!-- 文件类型图标（蓝色背景 + "PDF" 文字） -->
                            <div
                                class="flex h-8 w-8 items-center justify-center rounded bg-blue-500/10 text-xs font-bold text-blue-500"
                            >
                                PDF
                            </div>
                            <!-- 文件信息：名称 + 大小 -->
                            <div class="min-w-0">
                                <div
                                    class="truncate text-sm font-medium text-foreground"
                                >
                                    document.pdf
                                </div>
                                <div class="text-xs text-muted-foreground">
                                    2.4 MB
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            {/if}
        </div>

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
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md transition-colors hover:bg-glass-hover {emailState
                    .selectedEmail.is_starred
                    ? 'text-yellow-400'
                    : 'text-muted-foreground hover:text-foreground'}"
                title={t.email.star}
                onclick={handleToggleStar}
            >
                <Star
                    size={18}
                    class={emailState.selectedEmail.is_starred
                        ? "fill-current"
                        : ""}
                />
            </button>

            <!-- 归档按钮（功能占位） -->
            <button
                class="icon-btn-sm flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.email.archive}
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
