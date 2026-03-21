<script setup lang="ts">
import { ref, computed, watch, onUnmounted, onMounted } from "vue";
import { useUIStore, useAccountStore, useEmailStore } from "@/stores";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    NModal,
    NForm,
    NFormItem,
    NInput,
    NInputNumber,
    NSelect,
    NButton,
    NSwitch,
    NAlert,
    NRadioGroup,
    NRadio,
    NProgress,
    NCard,
    NCollapse,
    NCollapseItem,
    NSpin,
} from "naive-ui";
import {
    extractAutoFillInfo,
    parseEmail,
    detectProviderFromEmail,
} from "@/utils/emailHelper";
import { AccountType, AuthType } from "@/types";

const uiStore = useUIStore();
const accountStore = useAccountStore();
const emailStore = useEmailStore();

const emit = defineEmits<{
    (e: "update:show", value: boolean): void;
    (e: "success"): void;
}>();

const providerOptions = [
    { label: "Gmail", value: "gmail" },
    { label: "Outlook / Hotmail", value: "outlook" },
    { label: "iCloud Mail", value: "icloud" },
    { label: "Yahoo Mail", value: "yahoo" },
    { label: "自定义 IMAP/SMTP", value: "imap" },
];

const show = computed(() => uiStore.modals.addAccount);

const form = ref({
    name: "",
    email: "",
    provider: "gmail",
    password: "",
    accountType: AccountType.Personal,
    authType: AuthType.Password,
    imapHost: "",
    imapPort: 993,
    imapSsl: true,
    smtpHost: "",
    smtpPort: 587,
    smtpSsl: true,
    color: "#7C3AED",
    // 企业邮箱配置
    enterpriseTenantId: "",
    enterpriseDomain: "",
});

const loading = ref(false);
const error = ref("");
const success = ref("");

// 自动判断相关状态
const isProviderManuallySet = ref(false); // 是否由用户手动设置的服务商
const isAuthTypeManuallySet = ref(false); // 是否由用户手动修改了认证方式
const lastValidEmail = ref(""); // 上一次有效的邮箱地址
const isInitializing = ref(true); // 是否正在初始化（避免初始化时触发watch）

// OAuth 相关状态
const waitingForOAuth = ref(false); // 等待 OAuth 授权完成
const oauthError = ref<string | null>(null); // OAuth 错误信息
const oauthSessionId = ref(""); // OAuth 会话 ID
let unlistenOAuthFlow: UnlistenFn | null = null; // OAuth 流程完成事件监听器

// 企业配置预览
const enterprisePreview = ref({
    imapHost: "",
    smtpHost: "",
});

// 更新企业配置预览
function updateEnterprisePreview() {
    if (form.value.enterpriseTenantId || form.value.enterpriseDomain) {
        // Microsoft 365 / Outlook
        if (
            form.value.provider === "outlook" ||
            form.value.enterpriseTenantId?.includes("onmicrosoft")
        ) {
            enterprisePreview.value = {
                imapHost: "outlook.office365.com",
                smtpHost: "smtp.office365.com",
            };
        }
        // Google Workspace / Gmail
        else if (
            form.value.provider === "gmail" ||
            form.value.enterpriseTenantId?.includes("gmail")
        ) {
            enterprisePreview.value = {
                imapHost: "imap.gmail.com",
                smtpHost: "smtp.gmail.com",
            };
        }
    } else {
        enterprisePreview.value = {
            imapHost: "",
            smtpHost: "",
        };
    }
}

// 进度状态
const syncProgress = ref({
    stage: "idle" as
        | "idle"
        | "authenticating"
        | "validating"
        | "syncing"
        | "completed"
        | "error",
    currentStep: 0,
    totalSteps: 3,
    message: "",
    percentage: 0,
});

// 事件监听器清理
let unlistenProgress: (() => void) | null = null;

const isCustom = computed(() => form.value.provider === "imap");
const canUseOAuth = computed(() => {
    return (
        form.value.provider === "outlook" ||
        form.value.provider === "hotmail" ||
        form.value.provider === "gmail"
    );
});
const isEnterprise = computed(
    () => form.value.accountType === AccountType.Enterprise,
);

// 监听 provider 变化，自动设置邮箱前缀
watch(
    () => form.value.provider,
    (newProvider, oldProvider) => {
        // 如果是初始化阶段，不标记为手动设置
        if (isInitializing.value) {
            return;
        }

        // 如果是程序自动触发的（由邮箱地址变化导致），不标记为手动设置
        if (lastValidEmail.value) {
            const detected = detectProviderFromEmail(lastValidEmail.value);
            if (detected === newProvider) {
                // 这是自动触发的，不标记为手动
                return;
            }
        }

        // 用户手动改变了服务商
        if (oldProvider && newProvider !== oldProvider) {
            isProviderManuallySet.value = true;

            // 清空自定义服务器配置（如果切换回已知服务商）
            if (newProvider !== "imap") {
                form.value.imapHost = "";
                form.value.smtpHost = "";
            }

            // 自动切换认证方式（Gmail/Outlook/Yahoo → oauth，其他 → password）
            const recommendedAuthType =
                newProvider === "gmail" ||
                newProvider === "outlook" ||
                newProvider === "yahoo"
                    ? AuthType.OAuth2
                    : AuthType.Password;
            if (form.value.authType !== recommendedAuthType) {
                form.value.authType = recommendedAuthType;
            }
        }

        // 原有的自动填充邮箱前缀逻辑保持不变
        if (!form.value.email.includes("@")) {
            switch (newProvider) {
                case "gmail":
                    form.value.email = "@gmail.com";
                    break;
                case "outlook":
                case "hotmail":
                    form.value.email = "@outlook.com";
                    break;
                case "icloud":
                    form.value.email = "@icloud.com";
                    break;
                case "yahoo":
                    form.value.email = "@yahoo.com";
                    break;
            }
        }
        console.log(
            "#################### [AddAccountModal] PROVIDER WATCHER END ####################\n",
        );
    },
);

// 监听邮箱地址变化，自动判断服务商和填充信息
watch(
    () => form.value.email,
    (newEmail) => {
        // 如果用户手动设置了服务商，不再自动判断
        if (isProviderManuallySet.value) {
            return;
        }

        // 如果用户手动修改了认证方式，不再自动更改认证方式
        const shouldUpdateAuthType = !isAuthTypeManuallySet.value;

        // 邮箱地址不完整，不触发
        if (!newEmail || !newEmail.includes("@")) {
            return;
        }

        const { isValid } = parseEmail(newEmail);
        if (!isValid) {
            return;
        }

        // 提取自动填充信息
        const autoFillInfo = extractAutoFillInfo(newEmail, form.value.name);

        // 自动设置服务商
        if (autoFillInfo.provider !== form.value.provider) {
            form.value.provider = autoFillInfo.provider;
        }

        // 自动设置认证方式（仅在用户未手动修改时）
        if (
            shouldUpdateAuthType &&
            autoFillInfo.authType !== form.value.authType
        ) {
            // 将字符串转换为 AuthType 枚举
            form.value.authType =
                autoFillInfo.authType === "oauth"
                    ? AuthType.OAuth2
                    : AuthType.Password;
        }

        // 自动填充账号名称（仅在名称为空时）
        if (!form.value.name.trim() && autoFillInfo.name) {
            form.value.name = autoFillInfo.name;
        }

        // 如果是自定义服务商，自动配置服务器
        if (autoFillInfo.isCustom && autoFillInfo.serverConfig) {
            Object.assign(form.value, {
                imapHost: autoFillInfo.serverConfig.imapHost,
                smtpHost: autoFillInfo.serverConfig.smtpHost,
                imapPort: autoFillInfo.serverConfig.imapPort,
                smtpPort: autoFillInfo.serverConfig.smtpPort,
                imapSsl: autoFillInfo.serverConfig.imapSsl,
                smtpSsl: autoFillInfo.serverConfig.smtpSsl,
            });
        }

        lastValidEmail.value = newEmail;
    },
);

// 监听 IMAP SSL 变化，自动切换端口
watch(
    () => form.value.imapSsl,
    (newSsl) => {
        // 只有当端口是默认值时才自动切换
        if (newSsl && form.value.imapPort === 143) {
            form.value.imapPort = 993;
        } else if (!newSsl && form.value.imapPort === 993) {
            form.value.imapPort = 143;
        }
    },
);

// 监听 SMTP SSL 变化，自动切换端口
watch(
    () => form.value.smtpSsl,
    (newSsl) => {
        if (
            newSsl &&
            (form.value.smtpPort === 25 || form.value.smtpPort === 587)
        ) {
            form.value.smtpPort = 465;
        } else if (!newSsl && form.value.smtpPort === 465) {
            form.value.smtpPort = 25;
        }
    },
);

// 监听 show 变化，重置表单
watch(show, (newShow) => {
    if (!newShow) {
        error.value = "";
        success.value = "";
        // 重置 OAuth 状态
        waitingForOAuth.value = false;
        oauthError.value = null;
        oauthSessionId.value = "";
        // 重置自动判断状态
        isProviderManuallySet.value = false;
        isAuthTypeManuallySet.value = false;
        lastValidEmail.value = "";
        // 重置进度状态
        syncProgress.value = {
            stage: "idle",
            currentStep: 0,
            totalSteps: 3,
            message: "",
            percentage: 0,
        };
        // 清理事件监听器
        if (unlistenProgress) {
            unlistenProgress();
            unlistenProgress = null;
        }
    } else {
        // 打开时，重置为默认状态
        isProviderManuallySet.value = false;
        isAuthTypeManuallySet.value = false;
        lastValidEmail.value = "";
        isInitializing.value = true;

        // 延迟将 isInitializing 设置为 false，确保初始化完成
        setTimeout(() => {
            isInitializing.value = false;
        }, 100);
    }
});

// 组件销毁时清理监听器
onUnmounted(() => {
    if (unlistenProgress) {
        unlistenProgress();
        unlistenProgress = null;
    }
});

// 处理邮箱输入框失焦，更新服务器配置
function handleEmailBlur() {
    const email = form.value.email;

    // 邮箱地址不完整，不触发
    if (!email || !email.includes("@")) {
        return;
    }

    const { isValid } = parseEmail(email);
    if (!isValid) {
        return;
    }

    // 提取自动填充信息
    const autoFillInfo = extractAutoFillInfo(email, form.value.name);

    // 只更新服务器配置，不更新服务商和认证方式
    if (autoFillInfo.isCustom && autoFillInfo.serverConfig) {
        // 只有当当前是自定义服务商，或者服务器配置为空时才更新
        if (
            form.value.provider === "imap" ||
            !form.value.imapHost ||
            !form.value.smtpHost
        ) {
            Object.assign(form.value, {
                imapHost: autoFillInfo.serverConfig.imapHost,
                smtpHost: autoFillInfo.serverConfig.smtpHost,
                imapPort: autoFillInfo.serverConfig.imapPort,
                smtpPort: autoFillInfo.serverConfig.smtpPort,
                imapSsl: autoFillInfo.serverConfig.imapSsl,
                smtpSsl: autoFillInfo.serverConfig.smtpSsl,
            });
        }
    }
}

async function handleSubmit() {
    console.log("[AddAccountModal] handleSubmit 开始");
    console.log("[AddAccountModal] 表单数据:", {
        name: form.value.name,
        email: form.value.email,
        provider: form.value.provider,
        authType: form.value.authType,
        waitingForOAuth: waitingForOAuth.value,
    });

    error.value = "";
    success.value = "";

    try {
        // === 前端职责：基本格式校验 ===

        // 1. 验证账号名称
        if (!form.value.name.trim()) {
            error.value = "请输入账号名称";
            console.log("[AddAccountModal] 验证失败：账号名称为空");
            return;
        }

        // 2. 验证邮箱地址格式
        if (!form.value.email.trim() || form.value.email.startsWith("@")) {
            error.value = "请输入完整的邮箱地址";
            console.log("[AddAccountModal] 验证失败：邮箱地址不完整");
            return;
        }

        // 3. 邮箱格式基本校验（前端只检查是否包含 @ 和 .）
        const emailRegex = /^[^@\s]+@[^@\s]+\.[^@\s]+$/;
        if (!emailRegex.test(form.value.email)) {
            error.value = "请输入有效的邮箱地址";
            console.log("[AddAccountModal] 验证失败：邮箱格式无效");
            return;
        }

        // 4. OAuth 模式：检查是否正在等待授权
        if (form.value.authType === AuthType.OAuth2 && waitingForOAuth.value) {
            error.value = "请先完成 OAuth 授权或等待授权完成";
            console.log("[AddAccountModal] 验证失败：OAuth 授权进行中");
            return;
        }

        // 5. 密码模式：检查密码是否填写
        if (
            form.value.authType === AuthType.Password &&
            !form.value.password.trim()
        ) {
            error.value = "请输入密码";
            console.log("[AddAccountModal] 验证失败：密码为空");
            return;
        }

        console.log("[AddAccountModal] 前端格式校验通过");

        // === 调用后端创建账号（所有业务逻辑在后端处理） ===
        // 后端会通过 AuthManager 统一处理：
        // - OAuth token 验证
        // - 密码验证（IMAP 连接测试）
        // - 服务商配置获取
        // - 账号创建
        await createAccountAndSync();
    } catch (e: any) {
        console.error("[AddAccountModal] handleSubmit 全局错误:", e);
        error.value = `操作失败：${e?.message || String(e)}`;
        syncProgress.value.stage = "idle";
        loading.value = false;
    }
}

async function createAccountAndSync() {
    console.log("[AddAccountModal] createAccountAndSync 开始");
    loading.value = true;

    try {
        syncProgress.value = {
            stage: "syncing",
            currentStep: 2,
            totalSteps: 3,
            message: "正在创建账号...",
            percentage: 40,
        };

        const accountData: any = {
            name: form.value.name,
            email: form.value.email,
            provider: form.value.provider,
            color: form.value.color,
            accountType: form.value.accountType,
            authType: form.value.authType,
        };

        console.log("[AddAccountModal] 准备账号数据:", {
            ...accountData,
            authType: form.value.authType,
            isCustom: isCustom.value,
        });

        // 注意：OAuth 模式下，账号已由后端自动创建
        // 前端只处理密码认证模式
        if (form.value.authType === AuthType.OAuth2) {
            error.value = "OAuth 模式下账号已自动创建，请刷新账号列表查看";
            console.log("[AddAccountModal] OAuth 模式，跳过手动创建");
            return;
        }

        // 密码认证模式
        console.log("[AddAccountModal] 使用密码认证");
        accountData.password = form.value.password;

        if (isCustom.value) {
            console.log("[AddAccountModal] 自定义服务器配置");
            accountData.imap_host = form.value.imapHost;
            accountData.imap_port = form.value.imapPort;
            accountData.imap_ssl = form.value.imapSsl;
            accountData.smtp_host = form.value.smtpHost;
            accountData.smtp_port = form.value.smtpPort;
            accountData.smtp_ssl = form.value.smtpSsl;
        }

        // 企业邮箱配置
        if (isEnterprise.value) {
            console.log("[AddAccountModal] 企业邮箱配置");
            accountData.enterprise_tenant_id =
                form.value.enterpriseTenantId || null;
            accountData.enterprise_domain = form.value.enterpriseDomain || null;
        }

        // 1. 检查邮箱是否已存在于本地列表
        const existingAccount = accountStore.accounts.find(
            (a) => a.email === form.value.email,
        );
        if (existingAccount) {
            throw new Error(
                `邮箱地址 ${form.value.email} 已存在于账号列表中（账号名：${existingAccount.name}）。请先在设置中删除旧账号，或使用不同的邮箱地址。`,
            );
        }

        // 2. 创建账号
        console.log("[AddAccountModal] 调用 add_account");
        const accountResult = (await invoke("add_account", {
            account: accountData,
        })) as { id: number };
        const accountId = accountResult.id;

        console.log("[AddAccountModal] 账号创建成功, ID:", accountId);

        // 3. 刷新账号列表（从后端获取最新数据，避免重复添加）
        await accountStore.fetchAccounts();

        // 3.1 切换到新创建的账号
        accountStore.selectAccountById(String(accountId));

        // 3.2 重置加载状态
        loading.value = false;

        // 4. 立即关闭对话框并返回成功
        syncProgress.value = {
            stage: "idle",
            currentStep: 3,
            totalSteps: 3,
            message: "账号已添加，正在后台同步...",
            percentage: 100,
        };

        // 5. 注册同步进度监听器（用于状态栏显示）
        console.log("[AddAccountModal] 注册同步进度监听器（用于状态栏）");
        await listenToSyncProgress(accountId);

        // 6. 关闭对话框（通过 uiStore 而不是 emit）
        uiStore.closeAddAccountModal();

        // 7. 触发后台同步（不等待完成）
        console.log("[AddAccountModal] 触发后台同步, accountId:", accountId);
        invoke("sync_account_with_progress", { accountId })
            .then(async () => {
                console.log("[AddAccountModal] 后台同步完成");
                // 确保当前账号是正确的
                console.log(
                    "[AddAccountModal] 当前账号:",
                    accountStore.currentAccount?.email,
                    accountStore.currentAccount?.id,
                );
                // 刷新邮件列表
                await emailStore.fetchEmails();
                console.log(
                    "[AddAccountModal] 邮件列表已刷新, 邮件数量:",
                    emailStore.emails.length,
                );
            })
            .catch((error) => {
                console.error("[AddAccountModal] 后台同步失败:", error);
            });

        console.log(
            "[AddAccountModal] 账号添加成功，对话框已关闭，同步在后台进行",
        );
    } catch (e: any) {
        console.error("[AddAccountModal] createAccountAndSync 出错:", e);
        console.error("[AddAccountModal] 错误详情:", {
            message: e?.message,
            stack: e?.stack,
            string: String(e),
            json: JSON.stringify(e),
        });

        syncProgress.value = {
            stage: "error",
            currentStep: 0,
            totalSteps: 3,
            message: "",
            percentage: 0,
        };

        // 提供更详细的错误信息
        let errorMessage = "创建账号失败";
        if (typeof e === "string") {
            errorMessage = e;
        } else if (e?.message) {
            errorMessage = e.message;
        } else if (e?.toString) {
            errorMessage = e.toString();
        }

        error.value = errorMessage;
        loading.value = false;
    }
}

async function listenToSyncProgress(accountId: number) {
    const eventName = `sync-progress-${accountId}`;

    // 保存 unlisten 函数以便后续清理
    unlistenProgress = await listen(eventName, (event: any) => {
        const progress = event.payload as {
            stage: string;
            current: number;
            total: number;
            message: string;
        };

        // 计算：如果 total 为 0，显示不确定进度（50%）
        const hasTotal = progress.total > 0;
        const percentage = hasTotal
            ? Math.floor((progress.current / progress.total) * 100)
            : 50; // 不确定进度时显示 50%
        const currentStep = hasTotal
            ? Math.floor((progress.current / progress.total) * 3) + 1
            : 2;

        syncProgress.value = {
            stage: "syncing",
            currentStep,
            totalSteps: 3,
            message: progress.message,
            percentage,
        };

        if (progress.stage === "completed") {
            // 同步完成
            syncProgress.value.stage = "completed";
            syncProgress.value.percentage = 100;
            syncProgress.value.message = "同步完成！";

            setTimeout(async () => {
                // 刷新账号列表
                await accountStore.fetchAccounts();
                // 刷新邮件列表
                await emailStore.fetchEmails();
                emit("success");
                uiStore.closeAddAccountModal();
                resetForm();
                loading.value = false;
                if (unlistenProgress) {
                    unlistenProgress();
                    unlistenProgress = null;
                }
            }, 1000);
        } else if (progress.stage === "error") {
            // 同步出错
            syncProgress.value.stage = "error";
            error.value = progress.message;
            loading.value = false;
            if (unlistenProgress) {
                unlistenProgress();
                unlistenProgress = null;
            }
        }
    });
}

// 辅助函数：获取阶段标题
function getStageTitle(stage: string): string {
    switch (stage) {
        case "authenticating":
            return "正在验证授权";
        case "validating":
            return "正在验证连接";
        case "syncing":
            return "正在同步";
        case "completed":
            return "完成";
        case "error":
            return "失败";
        default:
            return "准备中";
    }
}

function resetForm() {
    form.value = {
        name: "",
        email: "",
        provider: "gmail",
        accountType: AccountType.Personal,
        authType: AuthType.Password,
        password: "",
        imapHost: "",
        imapPort: 993,
        imapSsl: true,
        smtpHost: "",
        smtpPort: 587,
        smtpSsl: true,
        color: "#7C3AED",
        // 企业邮箱配置
        enterpriseTenantId: "",
        enterpriseDomain: "",
    };
    success.value = "";
    error.value = "";
    // 重置 OAuth 状态
    waitingForOAuth.value = false;
    oauthError.value = null;
    oauthSessionId.value = "";
    // 重置进度状态
    syncProgress.value = {
        stage: "idle",
        currentStep: 0,
        totalSteps: 3,
        message: "",
        percentage: 0,
    };
    // 重置自动判断相关状态
    isProviderManuallySet.value = false;
    isAuthTypeManuallySet.value = false;
    lastValidEmail.value = "";
}

// 启动 OAuth 授权流程
async function startOAuthLogin() {
    console.log("[AddAccountModal] startOAuthLogin 被调用");
    console.log("[AddAccountModal] 邮箱:", form.value.email);

    if (!form.value.email) {
        error.value = "请先输入邮箱地址";
        return;
    }

    try {
        waitingForOAuth.value = true;
        oauthError.value = null;
        loading.value = true;

        console.log("[AddAccountModal] 调用 start_oauth_flow");

        // 调用后端启动 OAuth 流程
        const result = await invoke<{
            session_id: string;
            auth_url: string;
        }>("start_oauth_flow", {
            email: form.value.email,
        });

        console.log("[AddAccountModal] OAuth 流程已启动");
        console.log(
            "[AddAccountModal] 会话ID:",
            result.session_id.substring(0, 20) + "...",
        );
        console.log(
            "[AddAccountModal] 授权URL:",
            result.auth_url.substring(0, 100) + "...",
        );

        oauthSessionId.value = result.session_id;

        // 浏览器已自动打开授权页面
        loading.value = false;
    } catch (e) {
        console.error("[AddAccountModal] 启动 OAuth 流程失败:", e);
        oauthError.value = String(e);
        waitingForOAuth.value = false;
        loading.value = false;
    }
}

// 处理 OAuth 流程完成事件
async function handleOAuthFlowComplete(payload: {
    session_id: string;
    status: string;
    account?: {
        id: number;
        name: string;
        email: string;
        provider: string;
    };
    error?: string;
}) {
    console.log("[AddAccountModal] OAuth 流程完成:", payload);

    waitingForOAuth.value = false;
    loading.value = false;

    if (payload.status === "success" && payload.account) {
        console.log("[AddAccountModal] OAuth 授权成功:", payload.account);

        // 自动填充表单
        form.value.name = payload.account.name;
        form.value.email = payload.account.email;
        form.value.provider = payload.account.provider;

        // 显示成功消息
        success.value = "OAuth 授权成功！账号已自动创建";
        oauthError.value = null;

        // 刷新账号列表
        await accountStore.fetchAccounts();

        // 关闭弹窗
        setTimeout(() => {
            uiStore.closeAddAccountModal();
            resetForm();
        }, 1500);
    } else {
        console.error("[AddAccountModal] OAuth 授权失败:", payload.error);
        oauthError.value = payload.error || "授权失败";
        error.value = payload.error || "OAuth 授权失败";
    }
}

// 组件挂载时注册 OAuth 流程完成事件监听器
onMounted(async () => {
    console.log("[AddAccountModal] 组件挂载，注册 OAuth 流程完成监听器");

    try {
        unlistenOAuthFlow = await listen<{
            session_id: string;
            status: string;
            account?: {
                id: number;
                name: string;
                email: string;
                provider: string;
            };
            error?: string;
        }>("oauth-flow-complete", (event) => {
            console.log(
                "[AddAccountModal] 收到 OAuth 流程完成事件:",
                event.payload,
            );
            handleOAuthFlowComplete(event.payload);
        });
        console.log("[AddAccountModal] OAuth 流程完成监听器注册成功");
    } catch (e) {
        console.error("[AddAccountModal] 注册 OAuth 流程完成监听器失败:", e);
    }
});

// 组件卸载时清理事件监听器
onUnmounted(() => {
    // 清理 OAuth 流程完成事件监听器
    if (unlistenOAuthFlow) {
        unlistenOAuthFlow();
        console.log("[AddAccountModal] OAuth 流程完成监听器已清理");
    }

    // 清理同步进度事件监听器
    if (unlistenProgress) {
        unlistenProgress();
        console.log("[AddAccountModal] 同步进度监听器已清理");
    }
});
</script>

<template>
    <NModal
        :show="show"
        @update:show="uiStore.closeAddAccountModal"
        preset="card"
        title="添加邮箱账号"
        :style="{ width: '500px' }"
        :mask-closable="!loading"
        :close-on-esc="!loading"
    >
        <NForm @submit.prevent="handleSubmit">
            <!-- 账号名称 -->
            <NFormItem label="账号名称" path="name" :show-require-mark="true">
                <NInput
                    v-model:value="form.name"
                    placeholder="工作邮箱"
                    :disabled="loading"
                />
            </NFormItem>

            <!-- 邮箱地址 -->
            <NFormItem label="邮箱地址" path="email" :show-require-mark="true">
                <NInput
                    v-model:value="form.email"
                    placeholder="user@example.com"
                    :disabled="loading"
                    @blur="handleEmailBlur"
                />
            </NFormItem>

            <!-- 服务商 -->
            <NFormItem
                label="邮箱服务商"
                path="provider"
                :show-require-mark="true"
            >
                <NSelect
                    v-model:value="form.provider"
                    :options="providerOptions"
                    :disabled="loading"
                />
            </NFormItem>

            <!-- 账号类型 -->
            <NFormItem label="账号类型">
                <NRadioGroup
                    v-model:value="form.accountType"
                    :disabled="loading"
                >
                    <NRadio :value="AccountType.Personal">个人邮箱</NRadio>
                    <NRadio :value="AccountType.Enterprise">企业邮箱</NRadio>
                </NRadioGroup>
            </NFormItem>

            <!-- 认证方式 -->
            <NFormItem label="认证方式" v-if="canUseOAuth">
                <NRadioGroup v-model:value="form.authType" :disabled="loading">
                    <NRadio :value="AuthType.Password">密码登录</NRadio>
                    <NRadio :value="AuthType.OAuth2">OAuth 2.0 授权</NRadio>
                </NRadioGroup>
            </NFormItem>

            <!-- OAuth 登录按钮 -->
            <template v-if="canUseOAuth && form.authType === AuthType.OAuth2">
                <!-- OAuth 等待状态 -->
                <template v-if="waitingForOAuth">
                    <NFormItem>
                        <div class="oauth-waiting">
                            <NSpin :size="24" />
                            <div class="oauth-waiting-text">
                                <p class="oauth-title">等待授权完成...</p>
                                <p class="oauth-desc">
                                    请在浏览器中完成账号登录。授权完成后将自动返回应用。
                                </p>
                            </div>
                        </div>
                    </NFormItem>

                    <!-- OAuth 错误提示 -->
                    <NAlert v-if="oauthError" type="error">
                        {{ oauthError }}
                        <template #header>授权失败</template>
                        <div class="retry-link">
                            <a @click="startOAuthLogin">点击重试</a>
                        </div>
                    </NAlert>
                </template>

                <!-- OAuth 授权按钮 -->
                <template v-else>
                    <NFormItem>
                        <NButton
                            type="primary"
                            @click="startOAuthLogin"
                            :disabled="loading || !form.email"
                            block
                        >
                            使用
                            {{
                                form.provider === "gmail"
                                    ? "Google"
                                    : "Microsoft"
                            }}
                            账号授权
                        </NButton>
                    </NFormItem>

                    <!-- OAuth 成功提示 -->
                    <NAlert
                        v-if="success && success.includes('OAuth')"
                        type="success"
                        :title="success"
                    >
                    </NAlert>
                </template>
            </template>

            <!-- 密码输入 -->
            <NFormItem
                v-if="form.authType === AuthType.Password"
                label="密码"
                path="password"
                :show-require-mark="true"
            >
                <NInput
                    v-model:value="form.password"
                    type="password"
                    placeholder="邮箱密码或应用专用密码"
                    show-password-on="click"
                    :disabled="loading"
                />
            </NFormItem>

            <!-- 自定义 IMAP/SMTP 配置 -->
            <template v-if="isCustom">
                <NFormItem label="IMAP 服务器">
                    <NInput
                        v-model:value="form.imapHost"
                        placeholder="imap.example.com"
                        :disabled="loading"
                        style="flex: 1"
                    />
                </NFormItem>

                <NFormItem label="IMAP 端口和加密">
                    <div
                        style="
                            display: flex;
                            gap: 12px;
                            align-items: center;
                            width: 100%;
                        "
                    >
                        <NInputNumber
                            v-model:value="form.imapPort"
                            :min="1"
                            :max="65535"
                            placeholder="993"
                            :disabled="loading"
                            style="flex: 1"
                        />
                        <NSwitch
                            v-model:value="form.imapSsl"
                            :disabled="loading"
                        >
                            <template #checked>SSL</template>
                            <template #unchecked>非加密</template>
                        </NSwitch>
                        <span
                            style="color: var(--text-tertiary); font-size: 12px"
                        >
                            {{ form.imapSsl ? "993" : "143" }}
                        </span>
                    </div>
                </NFormItem>

                <NFormItem label="SMTP 服务器">
                    <NInput
                        v-model:value="form.smtpHost"
                        placeholder="smtp.example.com"
                        :disabled="loading"
                        style="flex: 1"
                    />
                </NFormItem>

                <NFormItem label="SMTP 端口和加密">
                    <div
                        style="
                            display: flex;
                            gap: 12px;
                            align-items: center;
                            width: 100%;
                        "
                    >
                        <NInputNumber
                            v-model:value="form.smtpPort"
                            :min="1"
                            :max="65535"
                            placeholder="465"
                            :disabled="loading"
                            style="flex: 1"
                        />
                        <NSwitch
                            v-model:value="form.smtpSsl"
                            :disabled="loading"
                        >
                            <template #checked>SSL</template>
                            <template #unchecked>非加密</template>
                        </NSwitch>
                        <span
                            style="color: var(--text-tertiary); font-size: 12px"
                        >
                            {{ form.smtpSsl ? "465" : "25" }}
                        </span>
                    </div>
                </NFormItem>
            </template>

            <!-- 企业邮箱配置 -->
            <NCollapse v-if="isEnterprise">
                <NCollapseItem title="企业邮箱配置" name="enterprise">
                    <NFormItem label="说明">
                        <NAlert type="info" title="企业邮箱配置说明">
                            如果您的组织使用 Microsoft 365 或 Google
                            Workspace，请填写以下信息以获得更好的配置体验。
                        </NAlert>
                    </NFormItem>

                    <NFormItem label="租户 ID / 域名">
                        <NInput
                            v-model:value="form.enterpriseTenantId"
                            placeholder="例如: contoso.onmicrosoft.com 或 example.com"
                            @input="updateEnterprisePreview"
                        />
                    </NFormItem>

                    <NFormItem label="域名（可选）">
                        <NInput
                            v-model:value="form.enterpriseDomain"
                            placeholder="例如: example.com"
                        />
                    </NFormItem>

                    <!-- 配置预览 -->
                    <NFormItem
                        v-if="enterprisePreview.imapHost"
                        label="配置预览"
                    >
                        <div class="enterprise-preview">
                            <p>
                                <strong>IMAP 服务器:</strong>
                                {{ enterprisePreview.imapHost }}
                            </p>
                            <p>
                                <strong>SMTP 服务器:</strong>
                                {{ enterprisePreview.smtpHost }}
                            </p>
                        </div>
                    </NFormItem>
                </NCollapseItem>
            </NCollapse>

            <!-- 颜色标签 -->
            <NFormItem label="颜色标签">
                <div class="color-options">
                    <div
                        v-for="color in [
                            '#7C3AED',
                            '#F43F5E',
                            '#10B981',
                            '#F59E0B',
                            '#3B82F6',
                            '#8B5CF6',
                            '#06B6D4',
                        ]"
                        :key="color"
                        class="color-option"
                        :class="{ active: form.color === color }"
                        :style="{ backgroundColor: color }"
                        @click="form.color = color"
                    />
                </div>
            </NFormItem>

            <!-- 提示信息 -->
            <NAlert
                v-if="success"
                type="success"
                :title="success"
                closable
                @close="success = ''"
            />
            <div v-if="error" class="error-message">
                {{ error }}
            </div>

            <!-- 进度显示 -->
            <div v-if="syncProgress.stage !== 'idle'" class="sync-progress">
                <NCard :bordered="false" class="progress-card">
                    <div class="progress-header">
                        <span class="progress-title">{{
                            getStageTitle(syncProgress.stage)
                        }}</span>
                        <span class="progress-percentage"
                            >{{ syncProgress.percentage }}%</span
                        >
                    </div>

                    <NProgress
                        type="line"
                        :percentage="syncProgress.percentage"
                        :processing="syncProgress.stage === 'syncing'"
                        :style="{ marginBottom: '12px' }"
                    />

                    <div class="progress-message">
                        {{ syncProgress.message }}
                    </div>

                    <div
                        v-if="syncProgress.stage === 'syncing'"
                        class="progress-steps"
                    >
                        <div
                            v-for="(step, index) in [
                                '验证账号',
                                '创建记录',
                                '同步邮件',
                            ]"
                            :key="step"
                            class="progress-step"
                            :class="{
                                active: index + 1 === syncProgress.currentStep,
                            }"
                        >
                            {{ step }}
                        </div>
                    </div>
                </NCard>
            </div>

            <!-- 操作按钮 -->
            <div class="form-actions">
                <NButton
                    @click="uiStore.closeAddAccountModal"
                    :disabled="loading || waitingForOAuth"
                >
                    取消
                </NButton>
                <!-- OAuth 模式下不显示添加按钮，因为账号会自动创建 -->
                <NButton
                    v-if="form.authType !== AuthType.OAuth2"
                    type="primary"
                    attr-type="submit"
                    :loading="loading"
                >
                    添加
                </NButton>
            </div>
        </NForm>
    </NModal>
</template>

<style scoped>
.color-options {
    display: flex;
    gap: 8px;
}

.color-option {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    cursor: pointer;
    border: 2px solid transparent;
    transition: all 0.15s ease;
}

.color-option:hover {
    transform: scale(1.1);
}

.color-option.active {
    border-color: var(--text-primary);
    box-shadow: 0 0 0 2px var(--bg-elevated);
}

.form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 24px;
}

.error-message {
    padding: 12px;
    background: rgba(244, 63, 78, 0.1);
    border: 1px solid rgba(244, 63, 78, 0.3);
    border-radius: var(--radius-md);
    color: var(--error);
    margin-bottom: 16px;
}

.sync-progress {
    margin: 16px 0;
}

.progress-card {
    background: var(--bg-secondary);
}

.progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.progress-title {
    font-weight: 500;
    font-size: 14px;
}

.progress-percentage {
    font-size: 14px;
    color: var(--text-secondary);
}

.progress-message {
    font-size: 13px;
    color: var(--text-secondary);
    margin-top: 8px;
}

.progress-steps {
    display: flex;
    gap: 8px;
    margin-top: 12px;
}

.progress-step {
    font-size: 12px;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--bg-tertiary);
    color: var(--text-tertiary);
    transition: all 0.2s;
}

.progress-step.active {
    background: var(--primary-bg);
    color: var(--primary-fg);
}

.enterprise-preview {
    padding: 12px;
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
    font-size: 13px;
}

.enterprise-preview p {
    margin: 4px 0;
}

.enterprise-preview strong {
    color: var(--text-primary);
}

/* OAuth 等待状态样式 */
.oauth-waiting {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px;
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
}

.oauth-waiting-text {
    flex: 1;
    text-align: left;
}

.oauth-title {
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 4px;
}

.oauth-desc {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
    margin: 0;
}

.retry-link {
    margin-top: 8px;
}

.retry-link a {
    color: var(--primary-color);
    cursor: pointer;
    text-decoration: underline;
}

.retry-link a:hover {
    color: var(--primary-color-hover);
}
</style>
