<script lang="ts">
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import { getAccountState } from "$lib/stores/account.svelte";
    import { commands } from "$lib/bindings";

    import { openUrl } from "@tauri-apps/plugin-opener";
    import {
        X,
        CircleCheck,
        Globe,
        ChevronRight,
        ChevronLeft,
        Loader2,
        ExternalLink,
        Shield,
    } from "lucide-svelte";
    import type { ProviderInfo } from "$lib/bindings";

    const i18n = getI18nState();
    const t = $derived(i18n.t);
    const accountStore = getAccountState();

    let open = $state(false);
    let step = $state<"select" | "credentials" | "done">("select");
    let selectedProvider = $state<ProviderInfo | null>(null);
    let isManual = $state(false);
    let providers = $state<ProviderInfo[]>([]);
    let loadingProviders = $state(true);

    // 表单字段
    let email = $state("");
    let displayName = $state("");
    let password = $state("");
    let authType = $state("Password");
    // 手动配置字段
    let imapHost = $state("");
    let imapPort = $state(993);
    let imapSslMode = $state("Tls");
    let smtpHost = $state("");
    let smtpPort = $state(465);
    let smtpSslMode = $state("Tls");

    let detecting = $state(false);
    let providerDetected = $state(false);
    let detectedProviderName = $state("");
    let submitting = $state(false);
    let error = $state("");

    // OAuth2 状态
    let oauthState = $state("");
    let oauthPolling = $state(false);
    let oauthError = $state("");

    // 认证模式判断
    let useOAuth2 = $derived(
        selectedProvider !== null &&
            selectedProvider.auth_type.includes("OAuth2"),
    );
    let showPasswordField = $derived(!useOAuth2 || authType === "Password");

    // SSL 模式选项
    const sslModes = [
        { value: "Tls", label: "SSL/TLS" },
        { value: "StartTls", label: "STARTTLS" },
        { value: "None", label: "无加密" },
    ];

    // 根据默认 SSL 端口映射
    const imapDefaultPorts: Record<string, number> = {
        Tls: 993,
        StartTls: 143,
        None: 143,
    };
    const smtpDefaultPorts: Record<string, number> = {
        Tls: 465,
        StartTls: 587,
        None: 25,
    };

    // 加载服务商列表
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

    function selectProvider(provider: ProviderInfo) {
        selectedProvider = provider;
        isManual = false;
        authType = provider.auth_type.includes("OAuth2")
            ? "OAuth2"
            : "Password";
        step = "credentials";
        error = "";
        oauthError = "";
    }

    function selectManual() {
        selectedProvider = null;
        isManual = true;
        authType = "Password";
        step = "credentials";
        error = "";
        oauthError = "";
    }

    async function detectProvider() {
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

    // ─── OAuth2 流程 ───

    async function startOAuth2Flow() {
        if (!selectedProvider || !email) return;
        oauthError = "";
        oauthPolling = true;

        try {
            const result = await commands.startOauth2(
                selectedProvider.id,
                email,
                displayName || null,
            );
            console.log(result);
            if (result.status === "error") {
                oauthError = result.error.message as string;
                oauthPolling = false;
                return;
            }

            const { url, state } = result.data;
            oauthState = state;

            // 在默认浏览器中打开授权 URL
            await openUrl(url);

            // 开始轮询
            pollOAuth2Status(state);
        } catch (e: unknown) {
            oauthError = String(e);
            oauthPolling = false;
        }
    }

    async function pollOAuth2Status(state: string) {
        let attempts = 0;
        const maxAttempts = 300; // 5 分钟, 每秒一次

        const interval = setInterval(async () => {
            attempts++;
            if (attempts >= maxAttempts) {
                clearInterval(interval);
                oauthPolling = false;
                oauthError = "OAuth2 授权超时，请重试";
                return;
            }

            try {
                const result = await commands.pollOauth2(state);

                if (result.status === "error") {
                    // invoke 本身出错，继续轮询
                    return;
                }

                const pollResult = result.data;

                // Rust 枚举序列化: "Pending" | { Completed: OAuth2CompletedInfo } | { Error: string }
                if (pollResult === "Pending") {
                    // 还在等待，继续轮询
                    return;
                }

                if (
                    typeof pollResult === "object" &&
                    "Completed" in pollResult
                ) {
                    clearInterval(interval);
                    oauthPolling = false;
                    // 后端已自动创建账号，刷新列表并跳转到完成页
                    await accountStore.loadAccounts();
                    step = "done";
                } else if (
                    typeof pollResult === "object" &&
                    "Error" in pollResult
                ) {
                    clearInterval(interval);
                    oauthPolling = false;
                    oauthError = pollResult.Error;
                }
            } catch {
                // poll command not yet available or other transient error - continue
            }
        }, 1000);
    }

    // ─── 密码模式提交 ───

    async function handleSubmit(e: Event) {
        e.preventDefault();
        error = "";
        submitting = true;
        try {
            const result = await commands.createAccount({
                name: email.split("@")[0] || email,
                email,
                display_name: displayName || null,
                provider: selectedProvider?.id || "custom",
                auth_type: authType,
                password,
                imap_host: isManual ? imapHost || null : null,
                imap_port: isManual ? imapPort : null,
                imap_ssl_mode: isManual ? imapSslMode : null,
                smtp_host: isManual ? smtpHost || null : null,
                smtp_port: isManual ? smtpPort : null,
                smtp_ssl_mode: isManual ? smtpSslMode : null,
                color: selectedProvider?.color || null,
                account_type: "personal",
            });
            if (result.status === "ok") {
                step = "done";
                await accountStore.loadAccounts();
            } else {
                error = result.error.message as string;
            }
        } catch (e: unknown) {
            error = String(e);
        }
        submitting = false;
    }

    // SSL 模式变更时自动更新端口
    function onImapSslModeChange(mode: string) {
        imapSslMode = mode;
        imapPort = imapDefaultPorts[mode] ?? 993;
    }

    function onSmtpSslModeChange(mode: string) {
        smtpSslMode = mode;
        smtpPort = smtpDefaultPorts[mode] ?? 465;
    }

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
        oauthState = "";
        oauthPolling = false;
        oauthError = "";
    }

    export function show() {
        open = true;
    }
</script>

{#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
        onclick={(e) => e.target === e.currentTarget && close()}
        onkeydown={(e) => e.key === "Escape" && close()}
    >
        <div
            class="w-full max-w-lg rounded-xl border border-border bg-card shadow-2xl"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between border-b border-border px-6 py-4"
            >
                <div>
                    <h2 class="text-lg font-semibold text-foreground">
                        {t.account.add}
                    </h2>
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
                <button
                    class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                    aria-label="Close"
                >
                    <X size={16} />
                </button>
            </div>

            <!-- Step 1: 选择服务商 -->
            {#if step === "select"}
                <div class="p-6">
                    <p class="mb-4 text-sm text-muted-foreground">
                        {t.account.selectProviderHint}
                    </p>

                    {#if loadingProviders}
                        <div class="flex items-center justify-center py-8">
                            <Loader2
                                size={24}
                                class="animate-spin text-muted-foreground"
                            />
                        </div>
                    {:else}
                        <div class="grid grid-cols-4 gap-3">
                            {#each providers as provider (provider.id)}
                                <button
                                    class="flex flex-col items-center gap-2 rounded-lg border border-border p-4 transition-all hover:border-primary/50 hover:bg-glass-hover"
                                    onclick={() => selectProvider(provider)}
                                >
                                    <div
                                        class="flex h-10 w-10 items-center justify-center rounded-full text-base font-bold text-white"
                                        style="background: {provider.color ||
                                            '#6366f1'}"
                                    >
                                        {provider.name.charAt(0)}
                                    </div>
                                    <span
                                        class="text-xs font-medium text-foreground"
                                        >{provider.name}</span
                                    >
                                </button>
                            {/each}

                            <!-- 其他 / 手动配置 -->
                            <button
                                class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border p-4 transition-all hover:border-primary/50 hover:bg-glass-hover"
                                onclick={selectManual}
                            >
                                <div
                                    class="flex h-10 w-10 items-center justify-center rounded-full bg-glass-active"
                                >
                                    <Globe
                                        size={20}
                                        class="text-muted-foreground"
                                    />
                                </div>
                                <span
                                    class="text-xs font-medium text-muted-foreground"
                                    >{t.account.other}</span
                                >
                            </button>
                        </div>
                    {/if}
                </div>

                <!-- Step 2: 输入凭据 -->
            {:else if step === "credentials"}
                <div class="max-h-[60vh] overflow-y-auto p-6">
                    <!-- 已选服务商信息 -->
                    {#if selectedProvider}
                        <div
                            class="mb-4 flex items-center gap-3 rounded-lg bg-glass px-4 py-3"
                        >
                            <div
                                class="flex h-8 w-8 items-center justify-center rounded-full text-sm font-bold text-white"
                                style="background: {selectedProvider.color ||
                                    '#6366f1'}"
                            >
                                {selectedProvider.name.charAt(0)}
                            </div>
                            <div>
                                <div
                                    class="text-sm font-medium text-foreground"
                                >
                                    {selectedProvider.name}
                                </div>
                                <div class="text-xs text-muted-foreground">
                                    {#each selectedProvider.auth_type as at, i}
                                        {#if i > 0}/{/if}{at}
                                    {/each}
                                </div>
                            </div>
                        </div>
                    {:else}
                        <div
                            class="mb-4 rounded-lg border border-dashed border-border px-4 py-3 text-sm text-muted-foreground"
                        >
                            {t.account.otherHint}
                        </div>
                    {/if}

                    <!-- OAuth2 模式 -->
                    {#if useOAuth2}
                        <div class="space-y-4">
                            <!-- Email -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.email}</label
                                >
                                <input
                                    type="email"
                                    bind:value={email}
                                    onchange={detectProvider}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder="you@example.com"
                                />
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

                            <!-- Display Name -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.displayName}</label
                                >
                                <input
                                    type="text"
                                    bind:value={displayName}
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder={t.account.displayName}
                                />
                            </div>

                            <!-- OAuth2 登录按钮 -->
                            <div class="space-y-3">
                                <button
                                    type="button"
                                    disabled={oauthPolling || !email}
                                    onclick={startOAuth2Flow}
                                    class="compose-btn flex w-full items-center justify-center gap-2 rounded-md px-4 py-3 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                >
                                    {#if oauthPolling}
                                        <Loader2
                                            size={16}
                                            class="animate-spin"
                                        />
                                        <span>等待授权中...</span>
                                    {:else}
                                        <ExternalLink size={16} />
                                        <span>使用 OAuth2 登录</span>
                                    {/if}
                                </button>

                                {#if oauthPolling}
                                    <p
                                        class="text-center text-xs text-muted-foreground"
                                    >
                                        已在浏览器中打开授权页面，完成授权后自动继续
                                    </p>
                                {/if}

                                {#if oauthError}
                                    <p class="text-sm text-destructive">
                                        {oauthError}
                                    </p>
                                {/if}

                                <!-- 备选: 使用密码登录 -->
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

                            <!-- 密码输入（备选模式） -->
                            {#if showPasswordField && authType === "Password"}
                                <form onsubmit={handleSubmit} class="space-y-4">
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
                                    {#if error}
                                        <p class="text-sm text-destructive">
                                            {error}
                                        </p>
                                    {/if}
                                    <button
                                        type="submit"
                                        disabled={submitting}
                                        class="compose-btn flex w-full items-center justify-center gap-2 rounded-md px-4 py-2.5 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                    >
                                        {#if submitting}
                                            <Loader2
                                                size={16}
                                                class="animate-spin"
                                            />
                                        {/if}
                                        {t.common.confirm}
                                    </button>
                                </form>
                            {/if}
                        </div>

                        <!-- 密码模式 / 手动配置 -->
                    {:else}
                        <form onsubmit={handleSubmit} class="space-y-4">
                            <!-- Email -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.email}</label
                                >
                                <input
                                    type="email"
                                    bind:value={email}
                                    onchange={detectProvider}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder="you@example.com"
                                />
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

                            <!-- Display Name -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.displayName}</label
                                >
                                <input
                                    type="text"
                                    bind:value={displayName}
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                    placeholder={t.account.displayName}
                                />
                            </div>

                            <!-- Password -->
                            <div>
                                <!-- svelte-ignore a11y_label_has_associated_control -->
                                <label
                                    class="mb-1 block text-sm font-medium text-foreground"
                                    >{t.account.password}</label
                                >
                                <input
                                    type="password"
                                    bind:value={password}
                                    required
                                    class="w-full rounded-md border border-border bg-glass px-3 py-2 text-sm text-foreground outline-none transition-colors focus:border-primary focus:ring-2 focus:ring-primary/20"
                                />
                            </div>

                            <!-- 手动配置: IMAP/SMTP + SSL 模式 -->
                            {#if isManual}
                                <div
                                    class="space-y-3 rounded-lg border border-border bg-glass/50 p-4"
                                >
                                    <h3
                                        class="text-sm font-medium text-foreground"
                                    >
                                        {t.account.manualConfig}
                                    </h3>

                                    <!-- IMAP 配置 -->
                                    <div class="grid grid-cols-5 gap-2">
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.imapHost}
                                            </label>
                                            <input
                                                type="text"
                                                bind:value={imapHost}
                                                required
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                placeholder="imap.example.com"
                                            />
                                        </div>
                                        <div>
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.imapPort}
                                            </label>
                                            <input
                                                type="number"
                                                bind:value={imapPort}
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                            />
                                        </div>
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                IMAP 加密
                                            </label>
                                            <select
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

                                    <!-- SMTP 配置 -->
                                    <div class="grid grid-cols-5 gap-2">
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.smtpHost}
                                            </label>
                                            <input
                                                type="text"
                                                bind:value={smtpHost}
                                                required
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                                placeholder="smtp.example.com"
                                            />
                                        </div>
                                        <div>
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                {t.account.smtpPort}
                                            </label>
                                            <input
                                                type="number"
                                                bind:value={smtpPort}
                                                class="w-full rounded-md border border-border bg-glass px-3 py-1.5 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/20"
                                            />
                                        </div>
                                        <div class="col-span-2">
                                            <!-- svelte-ignore a11y_label_has_associated_control -->
                                            <label
                                                class="mb-1 block text-xs text-muted-foreground"
                                            >
                                                SMTP 加密
                                            </label>
                                            <select
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

                            <!-- Error -->
                            {#if error}
                                <p class="text-sm text-destructive">{error}</p>
                            {/if}

                            <!-- Actions -->
                            <div class="flex justify-between pt-2">
                                <button
                                    type="button"
                                    class="flex items-center gap-1 rounded-md border border-border px-4 py-2 text-sm text-foreground transition-colors hover:bg-glass-hover"
                                    onclick={() => {
                                        step = "select";
                                        error = "";
                                    }}
                                >
                                    <ChevronLeft size={16} />
                                    {t.account.step1}
                                </button>
                                <button
                                    type="submit"
                                    disabled={submitting}
                                    class="compose-btn flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium text-white transition-colors disabled:opacity-50"
                                >
                                    {#if submitting}
                                        <Loader2
                                            size={16}
                                            class="animate-spin"
                                        />
                                    {/if}
                                    {t.common.confirm}
                                </button>
                            </div>
                        </form>
                    {/if}

                    <!-- OAuth2 模式的返回按钮 -->
                    {#if useOAuth2}
                        <div class="flex justify-start pt-4">
                            <button
                                type="button"
                                class="flex items-center gap-1 rounded-md border border-border px-4 py-2 text-sm text-foreground transition-colors hover:bg-glass-hover"
                                onclick={() => {
                                    step = "select";
                                    error = "";
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

                <!-- Step 3: 完成 -->
            {:else if step === "done"}
                <div class="p-8 text-center">
                    <CircleCheck size={48} class="mx-auto text-green-500" />
                    <p class="mt-3 text-sm font-medium text-foreground">
                        {email}
                    </p>
                    <p class="mt-1 text-xs text-muted-foreground">
                        {t.account.testSuccess}
                    </p>
                    <div class="mt-6 flex items-center justify-center gap-3">
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
                                oauthState = "";
                                oauthPolling = false;
                                oauthError = "";
                            }}
                        >
                            {t.account.addAnother}
                        </button>
                        <button
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

<style>
    .compose-btn {
        background: linear-gradient(
            135deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
    }
</style>
