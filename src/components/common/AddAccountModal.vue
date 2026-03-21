<script setup lang="ts">
import { ref, computed, watch, onUnmounted, onMounted } from "vue";
import { useUIStore, useAccountStore } from "@/stores";
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
    NCollapse,
    NCollapseItem,
} from "naive-ui";
import {
    extractAutoFillInfo,
    parseEmail,
    detectProviderFromEmail,
} from "@/utils/emailHelper";
import {
    AccountType,
    AuthType,
    type AuthInfo,
    type AuthResponse,
} from "@/types";

const uiStore = useUIStore();
const accountStore = useAccountStore();

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
    provider: "imap",
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

// 按钮文本：根据认证类型显示不同的验证按钮文本
const verifyButtonText = computed(() => {
    if (form.value.authType === AuthType.OAuth2) {
        return "验证OAuth";
    }
    return "验证密码";
});

// 是否可以提交验证
const canSubmit = computed(() => {
    // 基本验证
    if (!form.value.name.trim() || !form.value.email.trim()) {
        return false;
    }
    if (form.value.email.startsWith("@")) {
        return false;
    }
    // 密码认证需要填写密码
    if (
        form.value.authType === AuthType.Password &&
        !form.value.password.trim()
    ) {
        return false;
    }
    // OAuth 等待中不能重复提交
    if (waitingForOAuth.value) {
        return false;
    }
    return true;
});

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

// 统一认证处理函数
async function handleVerify() {
    console.log("[AddAccountModal] handleVerify 开始");
    console.log("[AddAccountModal] 认证类型:", form.value.authType);

    error.value = "";
    success.value = "";
    loading.value = true;

    try {
        // 基本格式校验
        if (!form.value.name.trim()) {
            error.value = "请输入账号名称";
            loading.value = false;
            return;
        }

        if (!form.value.email.trim() || form.value.email.startsWith("@")) {
            error.value = "请输入完整的邮箱地址";
            loading.value = false;
            return;
        }

        const emailRegex = /^[^@\s]+@[^@\s]+\.[^@\s]+$/;
        if (!emailRegex.test(form.value.email)) {
            error.value = "请输入有效的邮箱地址";
            loading.value = false;
            return;
        }

        // 根据 authType 构建 AuthInfo
        let authInfo: AuthInfo;

        if (form.value.authType === AuthType.OAuth2) {
            // OAuth 认证
            authInfo = {
                type: "OauthConfig",
                email: form.value.email,
                name: form.value.name || undefined,
                color: form.value.color,
            };
        } else {
            // 密码认证
            if (!form.value.password.trim()) {
                error.value = "请输入密码";
                loading.value = false;
                return;
            }

            // 始终传递 IMAP 和 SMTP 配置
            // - 如果是自定义服务器，使用用户配置的值
            // - 如果是标准提供商，传递表单值（后端会根据空值自动检测提供商配置）
            authInfo = {
                type: "ImapSmtpConfig",
                email: form.value.email,
                password: form.value.password,
                name: form.value.name || undefined,
                color: form.value.color,
                imapConfig: {
                    host: form.value.imapHost,
                    port: form.value.imapPort,
                    ssl: form.value.imapSsl,
                },
                smtpConfig: {
                    host: form.value.smtpHost,
                    port: form.value.smtpPort,
                    ssl: form.value.smtpSsl,
                },
            };
        }

        console.log("[AddAccountModal] 调用 start_auth_command");
        console.log("[AddAccountModal] authInfo:", JSON.stringify(authInfo, null, 2));
        const response = await invoke<AuthResponse>("start_auth_command", {
            authInfo,
        });

        console.log("[AddAccountModal] 认证响应:", response);

        // 处理响应
        if (response.status === "Pending") {
            // OAuth 需要等待授权
            waitingForOAuth.value = true;
            oauthSessionId.value = response.sessionId;
            loading.value = false;
            // 浏览器已自动打开授权页面
        } else if (response.status === "Success") {
            // 认证成功，账号已创建
            await handleAuthSuccess(response.account);
        } else if (response.status === "Error") {
            // 认证失败
            error.value = response.message;
            loading.value = false;
        }
    } catch (e: any) {
        console.error("[AddAccountModal] 认证失败:", e);
        error.value = e?.message || String(e) || "认证失败，请重试";
        loading.value = false;
    }
}

// 处理认证成功
async function handleAuthSuccess(account: any) {
    console.log("[AddAccountModal] 认证成功:", account);

    // 刷新账号列表
    await accountStore.fetchAccounts();

    // 设置为当前账号
    accountStore.selectAccountById(String(account.id));

    // 显示成功消息
    success.value = "账号添加成功！";

    // 关闭弹窗
    setTimeout(() => {
        uiStore.closeAddAccountModal();
        resetForm();
    }, 1000);
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
    // 重置自动判断相关状态
    isProviderManuallySet.value = false;
    isAuthTypeManuallySet.value = false;
    lastValidEmail.value = "";
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

        // 刷新账号列表
        await accountStore.fetchAccounts();

        // 设置为当前账号
        accountStore.selectAccountById(String(payload.account.id));

        // 显示成功消息并关闭弹窗
        success.value = "OAuth 授权成功！账号已添加";

        setTimeout(() => {
            uiStore.closeAddAccountModal();
            resetForm();
        }, 1000);
    } else {
        console.error("[AddAccountModal] OAuth 授权失败:", payload.error);
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
        <NForm>
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

            <!-- OAuth 等待提示 -->
            <NAlert v-if="waitingForOAuth" type="info" title="等待授权">
                已在浏览器中打开授权页面，请完成授权...
            </NAlert>

            <!-- 操作按钮 -->
            <div class="form-actions">
                <NButton
                    @click="uiStore.closeAddAccountModal"
                    :disabled="loading"
                >
                    取消
                </NButton>
                <NButton
                    type="primary"
                    :loading="loading"
                    :disabled="!canSubmit"
                    @click="handleVerify"
                >
                    {{ verifyButtonText }}
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
</style>
