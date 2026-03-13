<script setup lang="ts">
import { useUIStore } from "@/stores";
import { useWindowControl } from "@/composables/useWindowControl";
import { useI18n } from "vue-i18n";

// Stores
const uiStore = useUIStore();

// i18n
const { t } = useI18n();

// Window control
const {
    isMaximized,
    isTauriEnv,
    minimizeWindow,
    toggleMaximize,
    closeWindow,
} = useWindowControl();
</script>

<template>
    <header
        class="title-bar"
        :data-theme="uiStore.appliedTheme"
    >
        <!-- 拖拽区域 - 只有这里可以拖拽 -->
        <div
            class="title-bar-drag-region"
            data-tauri-drag-region
            @dblclick="toggleMaximize"
        >
            <div class="title-bar-left">
                <div class="app-icon">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
                        />
                    </svg>
                </div>
                <h1 class="app-title">Postium Mail</h1>
            </div>
        </div>

        <!-- 窗口控制按钮区域 - 完全独立的区域，没有拖拽属性 -->
        <div class="title-bar-controls">
            <button
                class="control-btn minimize-btn"
                @click.stop="minimizeWindow"
                :disabled="!isTauriEnv"
                :title="t('window.minimize')"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path d="M19 13H5v-2h14v2z" />
                </svg>
            </button>

            <button
                class="control-btn maximize-btn"
                @click.stop="toggleMaximize"
                :disabled="!isTauriEnv"
                :title="isMaximized ? t('window.restore') : t('window.maximize')"
            >
                <svg
                    v-if="!isMaximized"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M4 4h16v16H4V4zm2 2v12h12V6H6z"
                    />
                </svg>
                <svg
                    v-else
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M4 4h12v12H4V4zm2 2v8h8V6H6zm9 0h3v3h-3V6zm0 4h3v3h-3v-3zm0 4h3v3h-3v-3z"
                    />
                </svg>
            </button>

            <button
                class="control-btn close-btn"
                @click.stop="closeWindow"
                :disabled="!isTauriEnv"
                :title="t('window.close')"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12 19 6.41z"
                    />
                </svg>
            </button>
        </div>
    </header>
</template>

<style scoped>
.title-bar {
    height: var(--title-bar-height, 40px);
    background: var(--bg-elevated);
    backdrop-filter: blur(var(--blur-md, 16px));
    -webkit-backdrop-filter: blur(var(--blur-md, 16px));
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    z-index: 1000;
    position: relative;
    user-select: none;
}

.title-bar-drag-region {
    display: flex;
    align-items: center;
    flex: 1;
    height: 100%;
    overflow: hidden;
    min-width: 0;
}

.title-bar-left {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-left: 16px;
}

.app-icon {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--primary);
    flex-shrink: 0;
}

.app-icon svg {
    width: 100%;
    height: 100%;
}

.app-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.title-bar-controls {
    display: flex;
    align-items: center;
    height: 100%;
    flex-shrink: 0;
    position: relative;
    z-index: 10;
}

.control-btn {
    width: 46px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    cursor: pointer;
    transition: all var(--transition-fast, 0.15s ease);
    color: var(--text-secondary);
    position: relative;
    z-index: 1;
}

.control-btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
}

.control-btn svg {
    width: 12px;
    height: 12px;
    pointer-events: none;
}

.control-btn:not(:disabled):hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.control-btn.close-btn:not(:disabled):hover {
    background: #E81123;
    color: white;
}

/* 确保按钮可以点击 */
.control-btn {
    pointer-events: auto !important;
}

@media (display-mode: browser) {
    .control-btn {
        width: 40px;
    }
}
</style>
