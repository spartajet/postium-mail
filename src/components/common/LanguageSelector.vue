<script setup lang="ts">
import { computed } from "vue";
import { useUIStore } from "@/stores";
import { availableLanguages, getLanguageFlag } from "@/locales";
import { NDropdown } from "naive-ui";

const uiStore = useUIStore();

// 当前语言配置
const currentLanguage = computed(() => {
    return availableLanguages.find(
        lang => lang.code === uiStore.currentLocale
    );
});

// 当前语言国旗
const currentLanguageFlag = computed(() => {
    return getLanguageFlag(uiStore.currentLocale);
});

// 语言选项
const languageOptions = computed(() => {
    return availableLanguages.map(lang => ({
        label: lang.name,
        key: lang.code,
        flag: lang.flag,
        disabled: lang.code === uiStore.currentLocale,
    }));
});

// 处理语言切换
function handleLanguageChange(languageCode: string) {
    uiStore.setLanguage(languageCode as any);
}
</script>

<template>
    <n-dropdown
        trigger="click"
        :options="languageOptions"
        @select="handleLanguageChange"
        placement="bottom-end"
    >
        <button class="language-selector-btn" :title="currentLanguage?.name">
            <span class="language-flag">{{ currentLanguageFlag }}</span>
        </button>
    </n-dropdown>
</template>

<style scoped>
.language-selector-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px 8px;
    border: none;
    background: transparent;
    cursor: pointer;
    transition: background 0.15s ease;
    color: var(--text-secondary);
}

.language-selector-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.language-flag {
    font-size: 14px;
    line-height: 1;
}

:deep(.n-dropdown-option) {
    border-radius: 4px;
    margin: 2px 0;
}

:deep(.n-dropdown-option:hover) {
    background-color: rgba(255, 255, 255, 0.1) !important;
}

[data-theme="light"] :deep(.n-dropdown-option:hover) {
    background-color: rgba(0, 0, 0, 0.06) !important;
}
</style>
