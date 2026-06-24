<!--
  Postium Mail - 添加账号模态框组件
  AddAccountModal.svelte

  本组件是邮箱账号添加的核心模态框，支持多种邮箱服务商的账号配置。

  ==================== 功能说明 ====================
  1. 三步式添加流程：
     - 第一步：选择邮箱服务商（Gmail、Outlook等）或手动配置
     - 第二步：输入账号凭据（OAuth2授权 或 密码登录）
     - 第三步：显示添加成功结果
  2. OAuth2 授权流程：在默认浏览器中打开授权页面，轮询等待授权完成
  3. 密码认证流程：支持自动检测邮箱服务商、手动配置 IMAP/SMTP
  4. SSL/TLS 加密模式配置，自动推荐端口

  ==================== 组件状态 ====================
  - open: 模态框是否打开
  - step: 当前步骤（"select" | "credentials" | "done"）
  - selectedProvider: 选中的服务商信息
  - isManual: 是否为手动配置模式
  - providers: 可用的服务商列表
  - email/displayName/password: 表单字段
  - imapHost/imapPort/imapSslMode: IMAP 配置
  - smtpHost/smtpPort/smtpSslMode: SMTP 配置
  - oauthState/oauthPolling/oauthError: OAuth2 流程状态

  ==================== 交互说明 ====================
  - 点击服务商卡片：进入凭据输入步骤
  - 点击"其他"按钮：进入手动配置模式
  - OAuth2 模式：点击按钮打开浏览器授权，轮询等待结果
  - 密码模式：填写邮箱、密码等信息后提交
  - 点击遮罩层或 ESC 键：关闭模态框
  - 点击关闭按钮：关闭并重置所有状态

  ==================== 技术实现 ====================
  使用 Svelte 5 的 Runes API：
  - $state: 定义响应式状态
  - $derived: 定义派生状态（如 useOAuth2）
  - $effect: 副作用（如自动加载服务商列表）

  通过 Tauri commands 与后端 Rust 通信：
  - commands.listProviders(): 获取服务商列表
  - commands.detectProvider(): 自动检测邮箱服务商
  - commands.startOauth2(): 启动 OAuth2 授权
  - commands.pollOauth2(): 轮询 OAuth2 授权状态
  - commands.createAccount(): 创建邮箱账号
-->
<script lang="ts">
    // 导入国际化状态管理，用于多语言支持
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入账户状态管理，用于刷新账户列表
    import { getAccountState } from "$lib/stores/account.svelte";
    // 导入同步状态管理，用于首次同步新建账号
    import { getSyncState } from "$lib/stores/sync.svelte";
    // 导入 Tauri 后端命令，用于与服务端通信
    import { commands } from "$lib/bindings";

    // 导入 Tauri 的 URL 打开插件，用于在默认浏览器中打开 OAuth2 授权页面
    import { openUrl } from "@tauri-apps/plugin-opener";
    // 导入图标组件
    import {
        X, // 关闭按钮图标
        CircleCheck, // 成功完成图标
        Globe, // 全球/互联网图标（用于手动配置）
        ChevronRight, // 右箭头（步骤指示器）
        ChevronLeft, // 左箭头（返回按钮）
        Loader2, // 加载旋转图标
        ExternalLink, // 外部链接图标（用于 OAuth2 登录按钮）
        Shield, // 盾牌图标（用于密码登录按钮）
    } from "lucide-svelte";
    // 导入服务商信息类型定义
    import type { ProviderInfo } from "$lib/bindings";

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);
    // 获取账户状态实例
    const accountStore = getAccountState();
    // 获取同步状态实例
    const syncStore = getSyncState();

    // ─── 模态框基础状态 ───

    // 模态框是否打开（外部通过 show() 方法控制）
    let open = $state(false);
    // 当前步骤：选择服务商 → 输入凭据 → 完成
    let step = $state<"select" | "credentials" | "done">("select");
    // 选中的邮箱服务商信息，null 表示手动配置模式
    let selectedProvider = $state<ProviderInfo | null>(null);
    // 是否为手动配置模式（未选择预设服务商）
    let isManual = $state(false);
    // 可用的邮箱服务商列表
    let providers = $state<ProviderInfo[]>([]);
    // 是否正在加载服务商列表
    let loadingProviders = $state(true);

    // ─── 表单字段状态 ───

    // 用户邮箱地址
    let email = $state("");
    // 用户显示名称（可选）
    let displayName = $state("");
    // 邮箱密码（密码认证模式）
    let password = $state("");
    // 认证类型："OAuth2" 或 "Password"
    let authType = $state("Password");

    // ─── 手动配置字段（IMAP/SMTP） ───

    // IMAP 服务器地址
    let imapHost = $state("");
    // IMAP 服务器端口，默认 993（SSL/TLS）
    let imapPort = $state(993);
    // IMAP 加密模式，默认 "Tls"（SSL/TLS）
    let imapSslMode = $state("Tls");
    // SMTP 服务器地址
    let smtpHost = $state("");
    // SMTP 服务器端口，默认 465（SSL/TLS）
    let smtpPort = $state(465);
    // SMTP 加密模式，默认 "Tls"（SSL/TLS）
    let smtpSslMode = $state("Tls");

    // ─── 服务商检测状态 ───

    // 是否正在检测邮箱服务商
    let detecting = $state(false);
    // 是否成功检测到服务商
    let providerDetected = $state(false);
    // 检测到的服务商名称
    let detectedProviderName = $state("");

    // ─── 提交状态 ───

    // 是否正在提交表单
    let submitting = $state(false);
    // 错误信息
    let error = $state("");
    // 当前提交阶段：空闲、验证中、同步中、完成
    let phase = $state<"idle" | "validating" | "syncing" | "done">("idle");
    // 首次同步错误信息
    let syncError = $state("");

    // ─── OAuth2 授权流程状态 ───

    // OAuth2 授权 state 参数（用于防止 CSRF 攻击）
    let oauthState = $state("");
    // 是否正在轮询 OAuth2 授权状态
    let oauthPolling = $state(false);
    // OAuth2 授权错误信息
    let oauthError = $state("");

    // ─── 派生状态 ───

    // 判断是否使用 OAuth2 模式
    // 当选中的服务商支持 OAuth2 认证时为 true
    let useOAuth2 = $derived(
        selectedProvider !== null &&
            selectedProvider.auth_type.includes("OAuth2"),
    );

    // 判断是否显示密码输入字段
    // 非 OAuth2 模式时始终显示；OAuth2 模式下切换到密码登录时显示
    let showPasswordField = $derived(!useOAuth2 || authType === "Password");

    // ─── 常量配置 ───

    // SSL/TLS 加密模式选项
    const sslModes = [
        { value: "Tls", label: "SSL/TLS" }, // 直接 TLS 加密连接
        { value: "StartTls", label: "STARTTLS" }, // 先明文连接再升级为加密
        { value: "None", label: "无加密" }, // 无加密（不推荐）
    ];

    // IMAP 默认端口映射：根据加密模式推荐端口
    const imapDefaultPorts: Record<string, number> = {
        Tls: 993, // SSL/TLS 加密端口
        StartTls: 143, // STARTTLS 端口
        None: 143, // 无加密端口
    };

    // SMTP 默认端口映射：根据加密模式推荐端口
    const smtpDefaultPorts: Record<string, number> = {
        Tls: 465, // SSL/TLS 加密端口
        StartTls: 587, // STARTTLS 端口（最常用）
        None: 25, // 无加密端口
    };

    // ─── 副作用 ───

    // 当模态框打开时自动加载服务商列表
    $effect(() => {
        if (open && providers.length === 0) {
            loadingProviders = true;
            commands.listProviders().then((result) => {
                if (result.status === "ok") {
                    providers = result.data;
                }
                loadingProviders = false;
            });
        }
    });

    // ─── 交互函数 ───

    /**
     * 选择预设服务商
     * 点击服务商卡片时调用，自动设置认证类型并进入凭据步骤
     * @param provider - 选中的服务商信息
     */
    function selectProvider(provider: ProviderInfo) {
        selectedProvider = provider;
        isManual = false;
        // 如果服务商支持 OAuth2，默认使用 OAuth2；否则使用密码认证
        authType = provider.auth_type.includes("OAuth2")
            ? "OAuth2"
            : "Password";
        step = "credentials";
        error = "";
        syncError = "";
        phase = "idle";
        oauthError = "";
    }

    /**
     * 选择手动配置模式
     * 点击"其他"按钮时调用，进入手动 IMAP/SMTP 配置
     */
    function selectManual() {
        selectedProvider = null;
        isManual = true;
        authType = "Password";
        step = "credentials";
        error = "";
        syncError = "";
        phase = "idle";
        oauthError = "";
    }

    /**
     * 自动检测邮箱服务商
     * 根据用户输入的邮箱地址，尝试识别所属服务商
     * 例如：user@gmail.com → 检测到 Gmail
     */
    async function detectProvider() {
        // 邮箱格式不完整则跳过检测
        if (!email.includes("@")) return;
        detecting = true;
        try {
            const result = await commands.detectProvider(email);
            if (result.status === "ok" && result.data.detected) {
                providerDetected = true;
                detectedProviderName = result.data.provider_name || "";
            } else {
                providerDetected = false;
            }
        } catch {
            providerDetected = false;
        }
        detecting = false;
    }

    // ─── OAuth2 授权流程 ───

    /**
     * 启动 OAuth2 授权流程
     * 1. 调用后端获取授权 URL
     * 2. 在默认浏览器中打开授权页面
     * 3. 开始轮询等待授权完成
     */
    async function startOAuth2Flow() {
        if (!selectedProvider || !email) return;
        oauthError = "";
        syncError = "";
        phase = "idle";
        oauthPolling = true;

        try {
            // 向后端请求 OAuth2 授权 URL
            const result = await commands.startOauth2(
                selectedProvider.id,
                email,
                displayName || null,
            );

            if (result.status === "error") {
                oauthError = result.error.message as string;
                oauthPolling = false;
                return;
            }

            const { url, state } = result.data;
            // 保存 state 参数，用于后续轮询验证
            oauthState = state;

            // 在默认浏览器中打开授权 URL
            await openUrl(url);

            // 开始轮询授权状态
            pollOAuth2Status(state);
        } catch (e: unknown) {
            oauthError = String(e);
            oauthPolling = false;
        }
    }

    async function syncCreatedAccount(accountId: number) {
        phase = "syncing";
        syncError = "";
        accountStore.setActive(accountId);

        await syncStore.syncAccount(accountId);

        if (syncStore.error) {
            syncError = syncStore.error;
        }
    }

    function findAccountIdByEmail(targetEmail: string) {
        return (
            accountStore.accounts.find(
                (account) => account.email === targetEmail,
            )?.id ?? null
        );
    }

    /**
     * 轮询 OAuth2 授权状态
     * 每秒向后端查询一次授权结果，持续最多 5 分钟
     *
     * 授权状态有三种：
     * - "Pending": 用户尚未完成授权，继续轮询
     * - { Completed: ... }: 授权成功，后端已自动创建账号
     * - { Error: string }: 授权失败
     *
     * @param state - OAuth2 授权 state 参数（唯一标识本次授权请求）
     */
    async function pollOAuth2Status(state: string) {
        let attempts = 0;
        const maxAttempts = 300; // 最多 300 次（5 分钟，每秒一次）

        const interval = setInterval(async () => {
            attempts++;

            // 超过最大尝试次数，停止轮询并报错
            if (attempts >= maxAttempts) {
                clearInterval(interval);
                oauthPolling = false;
                oauthError = "OAuth2 授权超时，请重试";
                return;
            }

            try {
                const result = await commands.pollOauth2(state);

                // invoke 本身出错，继续轮询
                if (result.status === "error") {
                    return;
                }

                const pollResult = result.data;

                // Rust 枚举序列化格式: "Pending" | { Completed: OAuth2CompletedInfo } | { Error: string }

                // 状态一：仍在等待用户授权
                if (pollResult === "Pending") {
                    return;
                }

                // 状态二：授权成功完成
                if (
                    typeof pollResult === "object" &&
                    "Completed" in pollResult
                ) {
                    clearInterval(interval);
                    oauthPolling = false;
                    // 后端已自动创建账号，刷新账户列表并跳转到完成页
                    await accountStore.loadAccounts();
                    const accountId = findAccountIdByEmail(email);
                    if (accountId !== null) {
                        await syncCreatedAccount(accountId);
                    } else {
                        syncError = "账号已添加，但未能定位新账号执行首次同步";
                        phase = "done";
                    }
                    phase = "done";
                    step = "done";
                }
                // 状态三：授权失败
                else if (
                    typeof pollResult === "object" &&
                    "Error" in pollResult
                ) {
                    clearInterval(interval);
                    oauthPolling = false;
                    oauthError = pollResult.Error;
                }
            } catch {
                // 轮询命令暂不可用或其他临时错误 - 继续轮询
            }
        }, 1000); // 每秒轮询一次
    }

    // ─── 密码认证提交 ───

    /**
     * 提交账号创建请求（密码认证模式）
     * 收集所有表单数据，调用后端创建账号命令
     * @param e - 表单提交事件
     */
    async function handleSubmit(e: Event) {
        e.preventDefault();
        error = "";
        syncError = "";
        submitting = true;
        phase = "validating";
        try {
            const result = await commands.createAccount({
                // 用户名：取邮箱 @ 前的部分
                name: email.split("@")[0] || email,
                // 邮箱地址
                email,
                // 显示名称（可选）
                display_name: displayName || null,
                // 服务商 ID，手动配置时使用 "custom"
                provider: selectedProvider?.id || "custom",
                // 认证类型
                auth_type: authType,
                // 密码
                password,
                // IMAP 配置（仅手动模式时提交）
                imap_host: isManual ? imapHost || null : null,
                imap_port: isManual ? imapPort : null,
                imap_ssl_mode: isManual ? imapSslMode : null,
                // SMTP 配置（仅手动模式时提交）
                smtp_host: isManual ? smtpHost || null : null,
                smtp_port: isManual ? smtpPort : null,
                smtp_ssl_mode: isManual ? smtpSslMode : null,
                // 标签颜色
                color: selectedProvider?.color || null,
                // 账号类型
                account_type: "personal",
            });

            if (result.status === "ok") {
                // 创建成功，跳转到完成步骤
                await accountStore.loadAccounts();
                await syncCreatedAccount(result.data.id);
                phase = "done";
                step = "done";
            } else {
                // 创建失败，显示错误信息
                error = result.error.message as string;
                phase = "idle";
            }
        } catch (e: unknown) {
            error = String(e);
            phase = "idle";
        }
        submitting = false;
    }

    /**
     * IMAP SSL 模式变更处理
     * 自动更新 IMAP 端口为对应加密模式的推荐端口
     * @param mode - 新的加密模式
     */
    function onImapSslModeChange(mode: string) {
        imapSslMode = mode;
        imapPort = imapDefaultPorts[mode] ?? 993;
    }

    /**
     * SMTP SSL 模式变更处理
     * 自动更新 SMTP 端口为对应加密模式的推荐端口
     * @param mode - 新的加密模式
     */
    function onSmtpSslModeChange(mode: string) {
        smtpSslMode = mode;
        smtpPort = smtpDefaultPorts[mode] ?? 465;
    }

    /**
     * 关闭模态框并重置所有状态
     * 将所有状态恢复到初始值，确保下次打开时是干净的状态
     */
    function close() {
        open = false;
        step = "select";
        selectedProvider = null;
        isManual = false;
        email = "";
        displayName = "";
        password = "";
        authType = "Password";
        imapHost = "";
        imapPort = 993;
        imapSslMode = "Tls";
        smtpHost = "";
        smtpPort = 465;
        smtpSslMode = "Tls";
        detecting = false;
        providerDetected = false;
        detectedProviderName = "";
        submitting = false;
        error = "";
        phase = "idle";
        syncError = "";
        oauthState = "";
        oauthPolling = false;
        oauthError = "";
    }

    /**
     * 打开模态框
     * 此方法导出给父组件调用（如设置页面的"添加账号"按钮）
     */
    export function show() {
        open = true;
    }
</script>

{#if open}
    <!-- 模态框遮罩层：点击遮罩关闭，支持 ESC 键关闭 -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
        onclick={(e) => e.target === e.currentTarget && close()}
        onkeydown={(e) => e.key === "Escape" && close()}
    >
        <!-- 模态框主体：固定宽度，圆角卡片样式 -->
        <div
            data-testid="add-account-modal"
            class="w-full max-w-lg rounded-xl border border-border bg-card shadow-2xl"
        >
            <!-- 模态框头部：标题 + 步骤指示器 + 关闭按钮 -->
            <div
                class="flex items-center justify-between border-b border-border px-6 py-4"
            >
                <div>
                    <!-- 标题 -->
                    <h2 class="text-lg font-semibold text-foreground">
                        {t.account.add}
                    </h2>
                    <!-- 步骤指示器：显示当前所在步骤，高亮当前步骤 -->
                    <div
                        class="mt-2 flex items-center gap-2 text-xs text-muted-foreground"
                    >
                        <span
                            class={step === "select"
                                ? "font-medium text-primary"
                                : ""}>1. {t.account.selectProvider}</span
                        >
                        <ChevronRight size={12} />
                        <span
                            class={step === "credentials"
                                ? "font-medium text-primary"
                                : ""}>2. {t.account.step2}</span
                        >
                        <ChevronRight size={12} />
                        <span
                            class={step === "done"
                                ? "font-medium text-primary"
                                : ""}>3. {t.account.step3}</span
                        >
                    </div>
                </div>
                <!-- 关闭按钮 -->
                <button
                    data-testid="add-account-close-button"
                    class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                    aria-label="Close"
                >
                    <X size={16} />
                </button>
            </div>

            <!-- ─── 第一步：选择邮箱服务商 ─── -->
            {#if step === "select"}
                <div class="p-6">
                    <!-- 提示文字 -->
                    <p class="mb-4 text-sm text-muted-foreground">
                        {t.account.selectProviderHint}
                    </p>

                    {#if loadingProviders}
                        <!-- 加载中状态：显示旋转加载图标 -->
                        <div class="flex items-center justify-center py-8">
                            <Loader2
                                size={24}
                                class="animate-spin text-muted-foreground"
                            />
                        </div>
                    {:else}
                        <!-- 服务商选择网格：4 列布局 -->
                        <div class="grid grid-cols-4 gap-3">
                            {#each providers as provider (provider.id)}
                                <!-- 服务商卡片：显示首字母头像和名称 -->
                                <button
                                    class="flex flex-col items-center gap-2 rounded-lg border border-border p-4 transition-all hover:border-primary/50 hover:bg-glass-hover"
                                    onclick={() => selectProvider(provider)}
                                >
                                    <!-- 服务商图标：首字母圆形背景 -->
                                    <div
                                        class="flex h-10 w-10 items-center justify-center rounded-full text-base font-bold text-white"
                                        style="background: {provider.color ||
                                            '#6366f1'}"
                                    >
                                        {provider.name.charAt(0)}
                                    </div>
                                    <!-- 服务商名称 -->
                                    <span
                                        class="text-xs font-medium text-foreground"
                                        >{provider.name}</span
                                    >
                                </button>
                            {/each}

                            <!-- "其他"按钮：进入手动配置模式 -->
                            <button
                                data-testid="add-account-provider-other-button"
                                class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border p-4 transition-all hover:border-primary/50 hover:bg-glass-hover"
                                onclick={selectManual}
                            >
                                <!-- 虚线圆形背景 + 地球图标 -->
                                <div
                                    class="flex h-10 w-10 items-center justify-center rounded-full bg-glass-active"
                                >
                                    <Globe
                                        size={20}
                                        class="text-muted-foreground"
                                    />
                                </div>
                                <!-- "其他"文字 -->
                                <span
                                    class="text-xs font-medium text-muted-foreground"
                                    >{t.account.other}</span
                                >
                            </button>
                        </div>
                    {/if}
                </div>

                <!-- ─── 第二步：输入账号凭据 ─── -->
            {:else if step === "credentials"}
                <!-- 可滚动内容区域，限制最大高度 -->
                <div class="max-h-[60vh] overflow-y-auto p-6">
                    <!-- 已选服务商信息展示 -->
                    {#if selectedProvider}
                        <!-- 服务商信息卡片：显示选中的服务商名称和支持的认证方式 -->
                        <div
                            class="mb-4 flex items-center gap-3 rounded-lg bg-glass px-4 py-3"
                        >
                            <!-- 服务商图标 -->
                            <div
                                class="flex h-8 w-8 items-center justify-center rounded-full text-sm font-bold text-white"
                                style="background: {selectedProvider.color ||
                                    '#6366f1'}"
                            >
                                {selectedProvider.name.charAt(0)}
                            </div>
                            <div>
                                <!-- 服务商名称 -->
                                <div
                                    class="text-sm font-medium text-foreground"
                                >
                                    {selectedProvider.name}
                                </div>
                                <!-- 支持的认证方式（如 OAuth2/Password） -->
                                <div class="text-xs text-muted-foreground">
                                    {#each selectedProvider.auth_type as at, i}
                                        {#if i > 0}/{/if}{at}
                                    {/each}
                                </div>
                            </div>
                        </div>
                    {:else}
                        <!-- 手动配置模式提示 -->
                        <div
                            class="mb-4 rounded-lg border border-dashed border-border px-4 py-3 text-sm text-muted-foreground"
                        >
                            {t.account.otherHint}
                        </div>
                    {/if}

                    <!-- ─── OAuth2 认证模式 ─── -->
                    {#if useOAuth2}
                        <div class="space-y-4">
                            <!-- 邮箱输入 -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.email}</label
                                >
                                <input
                                    data-testid="add-account-email-input"
                                    type="email"
                                    bind:value={email}
                                    onchange={detectProvider}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder="you@example.com"
                                />
                                <!-- 服务商自动检测状态提示 -->
                                {#if detecting}
                                    <p
                                        class="mt-1 text-xs text-muted-foreground"
                                    >
                                        {t.account.detecting}
                                    </p>
                                {:else if providerDetected && detectedProviderName}
                                    <!-- 检测成功提示 -->
                                    <p class="mt-1 text-xs text-green-500">
                                        {t.account.detected}: {detectedProviderName}
                                    </p>
                                {/if}
                            </div>

                            <!-- 显示名称输入 -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.displayName}</label
                                >
                                <input
                                    data-testid="add-account-display-name-input"
                                    type="text"
                                    bind:value={displayName}
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder={t.account.displayName}
                                />
                            </div>

                            <!-- OAuth2 登录按钮区域 -->
                            <div class="space-y-3">
                                <!-- OAuth2 授权按钮：点击后打开浏览器进行授权 -->
                                <button
                                    type="button"
                                    disabled={oauthPolling || !email}
                                    onclick={startOAuth2Flow}
                                    class="compose-btn flex w-full items-center justify-center gap-2 rounded-md px-4 py-3 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                >
                                    {#if oauthPolling}
                                        <!-- 轮询中状态：显示加载图标 -->
                                        <Loader2
                                            size={16}
                                            class="animate-spin"
                                        />
                                        <span>等待授权中...</span>
                                    {:else}
                                        <!-- 默认状态：显示外部链接图标 -->
                                        <ExternalLink size={16} />
                                        <span>使用 OAuth2 登录</span>
                                    {/if}
                                </button>

                                <!-- 轮询提示信息 -->
                                {#if oauthPolling}
                                    <p
                                        class="text-center text-xs text-muted-foreground"
                                    >
                                        已在浏览器中打开授权页面，完成授权后自动继续
                                    </p>
                                {/if}

                                <!-- OAuth2 错误信息 -->
                                {#if oauthError}
                                    <p class="text-sm text-destructive">
                                        {oauthError}
                                    </p>
                                {/if}

                                <!-- 分隔线："或" -->
                                <div
                                    class="relative flex items-center justify-center"
                                >
                                    <div
                                        class="absolute inset-0 flex items-center"
                                    >
                                        <div
                                            class="w-full border-t border-border"
                                        ></div>
                                    </div>
                                    <span
                                        class="relative bg-card px-3 text-xs text-muted-foreground"
                                        >或</span
                                    >
                                </div>

                                <!-- 备选方案：切换到密码登录模式 -->
                                <button
                                    type="button"
                                    class="flex w-full items-center justify-center gap-2 rounded-md border border-border px-4 py-2.5 text-sm text-foreground transition-colors hover:bg-glass-hover"
                                    onclick={() => {
                                        authType = "Password";
                                    }}
                                >
                                    <Shield size={14} />
                                    <span>使用密码登录</span>
                                </button>
                            </div>

                            <!-- 密码输入表单（OAuth2 模式下切换到密码登录时显示） -->
                            {#if showPasswordField && authType === "Password"}
                                <form onsubmit={handleSubmit} class="space-y-4">
                                    <!-- 密码输入 -->
                                    <div>
                                        <!-- svelte-ignore a11y_label_has_associated_control -->
                                        <label
                                            class="mb-1 block text-sm font-medium text-foreground"
                                        >
                                            {t.account.password}
                                        </label>
                                        <input
                                            type="password"
                                            bind:value={password}
                                            required
                                            class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                        />
                                    </div>
                                    <!-- 错误提示 -->
                                    {#if error}
                                        <p class="text-sm text-destructive">
                                            {error}
                                        </p>
                                    {/if}
                                    <!-- 确认提交按钮 -->
                                    <button
                                        type="submit"
                                        disabled={submitting ||
                                            phase === "syncing"}
                                        class="compose-btn flex w-full items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                    >
                                        {#if submitting || phase === "syncing"}
                                            <Loader2
                                                size={16}
                                                class="animate-spin"
                                            />
                                        {/if}
                                        {#if phase === "validating"}
                                            正在验证...
                                        {:else if phase === "syncing"}
                                            正在同步...
                                        {:else}
                                            {t.common.confirm}
                                        {/if}
                                    </button>
                                </form>
                            {/if}
                        </div>

                        <!-- ─── 密码认证模式 / 手动配置 ─── -->
                    {:else}
                        <form onsubmit={handleSubmit} class="space-y-4">
                            <!-- 邮箱输入（带服务商自动检测） -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.email}</label
                                >
                                <input
                                    data-testid="add-account-email-input"
                                    type="email"
                                    bind:value={email}
                                    onchange={detectProvider}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder="you@example.com"
                                />
                                <!-- 服务商检测状态 -->
                                {#if detecting}
                                    <p
                                        class="mt-1 text-xs text-muted-foreground"
                                    >
                                        {t.account.detecting}
                                    </p>
                                {:else if providerDetected && detectedProviderName}
                                    <p class="mt-1 text-xs text-green-500">
                                        {t.account.detected}: {detectedProviderName}
                                    </p>
                                {/if}
                            </div>

                            <!-- 显示名称输入 -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.displayName}</label
                                >
                                <input
                                    data-testid="add-account-display-name-input"
                                    type="text"
                                    bind:value={displayName}
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder={t.account.displayName}
                                />
                            </div>

                            <!-- 密码输入 -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.password}</label
                                >
                                <input
                                    data-testid="add-account-password-input"
                                    type="password"
                                    bind:value={password}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                />
                            </div>

                            <!-- 手动配置区域：IMAP/SMTP 服务器设置（仅手动模式显示） -->
                            {#if isManual}
                                <div
                                    class="space-y-3 rounded-lg border border-border bg-glass/50 p-4"
                                >
                                    <h3
                                        class="text-sm font-medium text-foreground"
                                    >
                                        {t.account.manualConfig}
                                    </h3>

                                    <!-- IMAP 服务器配置行 -->
                                    <div class="grid grid-cols-5 gap-2">
                                        <!-- IMAP 主机地址 -->
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.imapHost}
                                            </label>
                                            <input
                                                data-testid="add-account-imap-host-input"
                                                type="text"
                                                bind:value={imapHost}
                                                required
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                placeholder="imap.example.com"
                                            />
                                        </div>
                                        <!-- IMAP 端口号 -->
                                        <div>
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.imapPort}
                                            </label>
                                            <input
                                                data-testid="add-account-imap-port-input"
                                                type="number"
                                                bind:value={imapPort}
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                            />
                                        </div>
                                        <!-- IMAP 加密模式选择 -->
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                IMAP 加密
                                            </label>
                                            <select
                                                data-testid="add-account-imap-ssl-select"
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                value={imapSslMode}
                                                onchange={(e) =>
                                                    onImapSslModeChange(
                                                        e.currentTarget.value,
                                                    )}
                                            >
                                                {#each sslModes as mode}
                                                    <option value={mode.value}
                                                        >{mode.label}</option
                                                    >
                                                {/each}
                                            </select>
                                        </div>
                                    </div>

                                    <!-- SMTP 服务器配置行 -->
                                    <div class="grid grid-cols-5 gap-2">
                                        <!-- SMTP 主机地址 -->
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.smtpHost}
                                            </label>
                                            <input
                                                data-testid="add-account-smtp-host-input"
                                                type="text"
                                                bind:value={smtpHost}
                                                required
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                placeholder="smtp.example.com"
                                            />
                                        </div>
                                        <!-- SMTP 端口号 -->
                                        <div>
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.smtpPort}
                                            </label>
                                            <input
                                                data-testid="add-account-smtp-port-input"
                                                type="number"
                                                bind:value={smtpPort}
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                            />
                                        </div>
                                        <!-- SMTP 加密模式选择 -->
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                SMTP 加密
                                            </label>
                                            <select
                                                data-testid="add-account-smtp-ssl-select"
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                value={smtpSslMode}
                                                onchange={(e) =>
                                                    onSmtpSslModeChange(
                                                        e.currentTarget.value,
                                                    )}
                                            >
                                                {#each sslModes as mode}
                                                    <option value={mode.value}
                                                        >{mode.label}</option
                                                    >
                                                {/each}
                                            </select>
                                        </div>
                                    </div>
                                </div>
                            {/if}

                            <!-- 错误信息提示 -->
                            {#if error}
                                <p
                                    data-testid="add-account-error"
                                    class="text-sm text-destructive"
                                >
                                    {error}
                                </p>
                            {/if}

                            <!-- 操作按钮区域 -->
                            <div class="flex justify-between pt-2">
                                <!-- 返回上一步按钮 -->
                                <button
                                    data-testid="add-account-back-button"
                                    type="button"
                                    class="flex items-center gap-1 rounded-md border border-border px-4 py-2 text-sm text-foreground transition-colors hover:bg-glass-hover"
                                    onclick={() => {
                                        step = "select";
                                        error = "";
                                        syncError = "";
                                        phase = "idle";
                                    }}
                                >
                                    <ChevronLeft size={16} />
                                    {t.account.step1}
                                </button>
                                <!-- 确认提交按钮 -->
                                <button
                                    data-testid="add-account-submit-button"
                                    type="submit"
                                    disabled={submitting ||
                                        phase === "syncing"}
                                    class="compose-btn flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                >
                                    {#if submitting || phase === "syncing"}
                                        <Loader2
                                            size={16}
                                            class="animate-spin"
                                        />
                                    {/if}
                                    {#if phase === "validating"}
                                        正在验证...
                                    {:else if phase === "syncing"}
                                        正在同步...
                                    {:else}
                                        {t.common.confirm}
                                    {/if}
                                </button>
                            </div>
                        </form>
                    {/if}

                    <!-- OAuth2 模式的返回按钮（独立于表单之外） -->
                    {#if useOAuth2}
                        <div class="flex justify-start pt-4">
                            <button
                                type="button"
                                class="flex items-center gap-1 rounded-md border border-border px-4 py-2 text-sm text-foreground transition-colors hover:bg-glass-hover"
                                onclick={() => {
                                    step = "select";
                                    error = "";
                                    syncError = "";
                                    phase = "idle";
                                    oauthError = "";
                                    oauthPolling = false;
                                }}
                            >
                                <ChevronLeft size={16} />
                                {t.account.step1}
                            </button>
                        </div>
                    {/if}
                </div>

                <!-- ─── 第三步：添加成功 ─── -->
            {:else if step === "done"}
                <div data-testid="add-account-done" class="p-8 text-center">
                    <!-- 成功图标 -->
                    <CircleCheck size={48} class="mx-auto text-green-500" />
                    <!-- 显示添加的邮箱地址 -->
                    <p class="mt-3 text-sm font-medium text-foreground">
                        {email}
                    </p>
                    <!-- 成功提示文字 -->
                    {#if syncError}
                        <p class="mt-1 text-xs text-destructive">
                            账号已添加，但首次同步失败：{syncError}
                        </p>
                    {:else}
                        <p class="mt-1 text-xs text-muted-foreground">
                            账号已添加并完成首次同步
                        </p>
                    {/if}
                    <!-- 操作按钮 -->
                    <div class="mt-6 flex items-center justify-center gap-3">
                        <!-- 继续添加另一个账号 -->
                        <button
                            class="rounded-md border border-border px-4 py-2 text-sm text-foreground transition-colors hover:bg-glass-hover"
                            onclick={() => {
                                step = "select";
                                email = "";
                                displayName = "";
                                password = "";
                                selectedProvider = null;
                                isManual = false;
                                error = "";
                                phase = "idle";
                                syncError = "";
                                oauthState = "";
                                oauthPolling = false;
                                oauthError = "";
                            }}
                        >
                            {t.account.addAnother}
                        </button>
                        <!-- 关闭模态框 -->
                        <button
                            data-testid="add-account-done-close-button"
                            class="compose-btn rounded-md px-4 py-2 text-sm font-medium text-white"
                            onclick={close}
                        >
                            {t.common.close}
                        </button>
                    </div>
                </div>
            {/if}
        </div>
    </div>
{/if}

<!-- 样式定义 -->
<style>
    /* 主操作按钮样式：使用主色到辅色的渐变背景 */
    .compose-btn {
        background: linear-gradient(
            135deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
    }
</style>
