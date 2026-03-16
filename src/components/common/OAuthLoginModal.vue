ode<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    NModal,
    NSpin,
    NAlert,
    NInput,
    NButton,
    NCollapse,
    NCollapseItem,
} from "naive-ui";

const props = defineProps<{
    show: boolean;
    provider: "microsoft" | "google";
}>();

const emit = defineEmits<{
    (e: "update:show", value: boolean): void;
    (
        e: "success",
        token: {
            access_token: string;
            refresh_token: string;
            expires_at: number;
        },
    ): void;
}>();

const loading = ref(true);
const error = ref<string | null>(null);
const authUrl = ref("");
const manualCode = ref("");
const waitingForCallback = ref(false);

// 存储csrf_state用于验证
let csrfState: string = "";

// Tauri 事件监听器取消函数
let unlistenDeepLink: UnlistenFn | null = null;

// 监听 show 的变化
watch(
    () => props.show,
    async (newShow) => {
        console.log("[OAuthLoginModal] props.show 变化:", newShow);
        if (newShow) {
            await startOAuthFlow();
        } else {
            // 关闭时重置状态
            waitingForCallback.value = false;
            error.value = null;
            manualCode.value = "";
        }
    },
);

onMounted(async () => {
    console.log("[OAuthLoginModal] 组件挂载, props.show:", props.show);

    // 注册 Deep Link 事件监听器
    try {
        unlistenDeepLink = await listen<{
            code?: string;
            state?: string;
            error?: string;
            errorDescription?: string;
        }>("oauth-deep-link-callback", (event) => {
            console.log(
                "[OAuthLoginModal] 收到 Deep Link 回调:",
                event.payload,
            );
            handleDeepLinkCallback(event.payload);
        });
        console.log("[OAuthLoginModal] Deep Link 监听器注册成功");
    } catch (e) {
        console.error("[OAuthLoginModal] 注册 Deep Link 监听器失败:", e);
    }

    if (props.show) {
        await startOAuthFlow();
    }
});

// 处理 Deep Link 回调
function handleDeepLinkCallback(payload: {
    code?: string;
    state?: string;
    error?: string;
    errorDescription?: string;
}) {
    waitingForCallback.value = false;

    if (payload.error) {
        console.error("[OAuthLoginModal] Deep Link 返回错误:", payload.error);
        error.value = payload.errorDescription || payload.error;
        return;
    }

    if (payload.code) {
        console.log("[OAuthLoginModal] Deep Link 返回授权码，自动提交");
        manualCode.value = payload.code;
        exchangeCodeForToken(payload.code);
    } else {
        error.value = "未收到授权码，请重试";
    }
}

async function startOAuthFlow() {
    console.log("[OAuthLoginModal] startOAuthFlow 开始");
    try {
        loading.value = true;
        error.value = null;
        waitingForCallback.value = false;

        console.log(
            "[OAuthLoginModal] 调用 get_oauth_auth_url, provider:",
            props.provider,
        );
        // 1. 获取授权 URL 和 csrf_state
        const [authUrlStr, state] = await invoke<[string, string]>(
            "get_oauth_auth_url",
            {
                provider: props.provider,
            },
        );

        console.log(
            "[OAuthLoginModal] 授权URL:",
            authUrlStr.substring(0, 100) + "...",
        );
        console.log(
            "[OAuthLoginModal] CSRF State:",
            state.substring(0, 20) + "...",
        );

        // 存储csrf_state用于后续验证
        csrfState = state;
        authUrl.value = authUrlStr;

        console.log("[OAuthLoginModal] 在系统浏览器中打开授权页面");
        // 2. 在系统浏览器中打开授权页面
        await openUrl(authUrlStr);

        console.log("[OAuthLoginModal] 授权页面已在浏览器中打开");
        loading.value = false;
        waitingForCallback.value = true;
    } catch (e) {
        console.error("[OAuthLoginModal] startOAuthFlow 出错:", e);
        error.value = String(e);
        loading.value = false;
    }
}

async function exchangeCodeForToken(code: string) {
    console.log("[OAuthLoginModal] exchangeCodeForToken 开始");
    try {
        loading.value = true;

        // 注意：后端会自动从Microsoft Graph API获取用户email
        console.log("[OAuthLoginModal] 调用 exchange_oauth_code");
        const account = await invoke<{
            id: number;
            name: string;
            email: string;
            provider: string;
        }>("exchange_oauth_code", {
            provider: props.provider,
            code,
            csrfState: csrfState,
        });

        console.log("[OAuthLoginModal] 账号创建成功:", account);

        // 成功创建账号，触发成功事件
        emit("success", {
            access_token: "", // token已安全存储在后端，不需要传递到前端
            refresh_token: "",
            expires_at: 0,
        });
        emit("update:show", false);

        loading.value = false;
    } catch (e) {
        console.error("[OAuthLoginModal] exchangeCodeForToken 出错:", e);
        error.value = String(e);
        loading.value = false;
    }
}

async function submitCode() {
    console.log("[OAuthLoginModal] submitCode 被调用");
    if (!manualCode.value.trim()) {
        error.value = "请输入授权码";
        return;
    }

    await exchangeCodeForToken(manualCode.value.trim());
}

async function openAuthPage() {
    if (authUrl.value) {
        console.log("[OAuthLoginModal] 重新打开授权页面");
        await openUrl(authUrl.value);
    }
}

async function retryOAuth() {
    error.value = null;
    manualCode.value = "";
    await startOAuthFlow();
}

onUnmounted(() => {
    // 清理事件监听
    if (unlistenDeepLink) {
        unlistenDeepLink();
        console.log("[OAuthLoginModal] Deep Link 监听器已清理");
    }
});
</script>

<template>
    <NModal
        :show="show"
        @update:show="emit('update:show', $event)"
        preset="card"
        :title="provider === 'microsoft' ? 'Microsoft 登录' : 'Google 登录'"
        :style="{ width: '500px' }"
    >
        <div class="oauth-login">
            <NSpin :show="loading">
                <div class="content">
                    <h3>
                        {{ provider === "microsoft" ? "Microsoft" : "Google" }}
                        账号授权
                    </h3>

                    <NAlert v-if="error" type="error" class="mb-4">
                        <template #header>授权失败</template>
                        {{ error }}
                        <div class="retry-link">
                            <a @click="retryOAuth">点击重试</a>
                        </div>
                    </NAlert>

                    <!-- 等待回调状态 -->
                    <div
                        v-if="!error && waitingForCallback"
                        class="waiting-state"
                    >
                        <div class="waiting-icon">
                            <NSpin :size="40" />
                        </div>
                        <p class="waiting-title">等待授权完成...</p>
                        <p class="waiting-text">
                            请在浏览器中完成
                            {{
                                provider === "microsoft"
                                    ? "Microsoft"
                                    : "Google"
                            }}
                            账号登录。 授权完成后将自动返回应用。
                        </p>
                        <p class="waiting-hint">
                            如果浏览器没有自动打开，<a @click="openAuthPage"
                                >点击这里重新打开</a
                            >
                        </p>
                    </div>

                    <!-- 初始加载状态 -->
                    <div
                        v-if="!error && !waitingForCallback && !authUrl"
                        class="initial-state"
                    >
                        <NSpin :size="32" />
                        <p>正在准备授权...</p>
                    </div>

                    <!-- 手动输入折叠面板（备用方案） -->
                    <NCollapse v-if="!error && authUrl" class="manual-collapse">
                        <NCollapseItem
                            title="手动输入授权码（备用）"
                            name="manual"
                        >
                            <div class="manual-content">
                                <p class="manual-hint">
                                    如果自动回调失败，请在浏览器授权完成后，从
                                    URL 中复制
                                    <code>code</code> 参数的值并粘贴到下方：
                                </p>
                                <div class="code-input-section">
                                    <NInput
                                        v-model:value="manualCode"
                                        type="textarea"
                                        placeholder="请粘贴授权码（code 参数的值）"
                                        :autosize="{ minRows: 2, maxRows: 4 }"
                                        class="code-input"
                                    />
                                    <NButton
                                        type="primary"
                                        @click="submitCode"
                                        :disabled="!manualCode.trim()"
                                        class="submit-button"
                                    >
                                        提交授权码
                                    </NButton>
                                </div>
                            </div>
                        </NCollapseItem>
                    </NCollapse>
                </div>
            </NSpin>
        </div>
    </NModal>
</template>

<style scoped>
.oauth-login {
    padding: 20px 0;
}

.content {
    text-align: center;
}

.content h3 {
    margin-bottom: 16px;
    color: var(--text-color-1);
}

.initial-state {
    padding: 40px 20px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
}

.initial-state p {
    color: var(--text-color-2);
}

.waiting-state {
    padding: 30px 20px;
}

.waiting-icon {
    margin-bottom: 20px;
}

.waiting-title {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-color-1);
    margin-bottom: 12px;
}

.waiting-text {
    color: var(--text-color-2);
    line-height: 1.6;
    margin-bottom: 16px;
}

.waiting-hint {
    font-size: 13px;
    color: var(--text-color-3);
}

.waiting-hint a,
.retry-link a {
    color: var(--primary-color);
    cursor: pointer;
    text-decoration: underline;
}

.waiting-hint a:hover,
.retry-link a:hover {
    color: var(--primary-color-hover);
}

.retry-link {
    margin-top: 8px;
}

.manual-collapse {
    margin-top: 20px;
    text-align: left;
}

.manual-content {
    padding: 8px 0;
}

.manual-hint {
    font-size: 13px;
    color: var(--text-color-3);
    margin-bottom: 12px;
    line-height: 1.5;
}

.manual-hint code {
    background: var(--code-color);
    padding: 2px 6px;
    border-radius: 4px;
    font-family: monospace;
}

.code-input-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.code-input {
    width: 100%;
}

.submit-button {
    width: 100%;
    height: 40px;
}

.mb-4 {
    margin-bottom: 16px;
}
</style>
