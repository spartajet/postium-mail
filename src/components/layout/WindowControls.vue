<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import {
    MinimizeOutlined,
    WebAssetOutlined,
    WebAssetOffOutlined,
    CloseOutlined,
} from "@vicons/material";

// i18n
const { t } = useI18n();

const isMaximized = ref(false);
const isReady = ref(false);

let appWindow: any = null;
let unlistenResized: (() => void) | null = null;

// 初始化 Tauri
async function initTauri() {
    try {
        // 直接尝试导入，不检查 __TAURI__
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        appWindow = getCurrentWindow();

        // 测试调用是否成功
        await appWindow.isMaximized();

        console.log("[WindowControls] ✅ Tauri initialized successfully");
        return true;
    } catch (error: any) {
        // 检查错误类型
        if (error?.message?.includes("__TAURI__")) {
            console.warn(
                "[WindowControls] ⚠️ Not running in Tauri environment",
            );
            console.warn(
                "[WindowControls] 💡 Use 'yarn tauri dev' instead of 'yarn dev'",
            );
        } else {
            console.error("[WindowControls] ❌ Tauri init error:", error);
        }
        return false;
    }
}

// 更新窗口状态
async function updateWindowState() {
    if (!appWindow) return;

    try {
        isMaximized.value = await appWindow.isMaximized();
    } catch (error) {
        console.error("[WindowControls] Failed to update state:", error);
    }
}

// 最小化窗口
async function minimizeWindow() {
    if (!appWindow) {
        console.warn("[WindowControls] Not initialized");
        return;
    }

    try {
        await appWindow.minimize();
        console.log("[WindowControls] ✅ Minimized");
    } catch (error) {
        console.error("[WindowControls] Minimize failed:", error);
    }
}

// 切换最大化
async function toggleMaximize() {
    if (!appWindow) {
        console.warn("[WindowControls] Not initialized");
        return;
    }

    try {
        if (isMaximized.value) {
            await appWindow.unmaximize();
        } else {
            await appWindow.maximize();
        }
        await updateWindowState();
        console.log("[WindowControls] ✅ Toggle maximize");
    } catch (error) {
        console.error("[WindowControls] Toggle failed:", error);
    }
}

// 关闭窗口
async function closeWindow() {
    if (!appWindow) {
        console.warn("[WindowControls] Not initialized");
        return;
    }

    try {
        await appWindow.close();
    } catch (error) {
        console.error("[WindowControls] Close failed:", error);
    }
}

// 初始化
onMounted(async () => {
    console.log("[WindowControls] Mounting...");
    const success = await initTauri();
    isReady.value = success;

    if (success && appWindow) {
        await updateWindowState();

        // 监听窗口变化
        try {
            appWindow
                .onResized(async () => {
                    await updateWindowState();
                })
                .then((unlisten: any) => {
                    unlistenResized = unlisten;
                });
        } catch (error) {
            console.error("[WindowControls] Failed to setup listener:", error);
        }
    }

    console.log("[WindowControls] Ready:", isReady.value);
});

onUnmounted(() => {
    if (unlistenResized) {
        unlistenResized();
    }
});
</script>

<template>
    <div class="window-controls">
        <!-- 最小化按钮 -->
        <button
            class="control-btn"
            @click="minimizeWindow"
            :title="t('window.minimize')"
        >
            <MinimizeOutlined :size="14" />
        </button>

        <!-- 最大化/还原按钮 -->
        <button
            class="control-btn"
            @click="toggleMaximize"
            :title="isMaximized ? t('window.restore') : t('window.maximize')"
        >
            <WebAssetOutlined v-if="!isMaximized" :size="14" />
            <WebAssetOffOutlined v-else :size="14" />
        </button>

        <!-- 关闭按钮 -->
        <button
            class="control-btn close-btn"
            @click="closeWindow"
            :title="t('window.close')"
        >
            <CloseOutlined :size="12" />
        </button>
    </div>
</template>

<style scoped>
.window-controls {
    position: fixed;
    top: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 40px;
    z-index: 9999;
}

.control-btn {
    width: 40px;
    height: 100%;
    padding: 0 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    cursor: pointer;
    transition: background 0.15s ease;
    color: var(--text-secondary);
    -webkit-app-region: no-drag;
}

.control-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.control-btn.close-btn:hover {
    background: #e81123;
    color: white;
}
</style>
