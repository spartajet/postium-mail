<script setup lang="ts">
import { ref, computed } from "vue";
import { useEmailStore, useUIStore } from "@/stores";
import { useI18n } from "vue-i18n";
import { formatDistanceToNow } from "date-fns";
import {
    SearchOutlined,
    CloseOutlined,
    RefreshOutlined,
    FilterListOutlined,
    CheckBoxOutlineBlankOutlined,
    EmailOutlined,
    AttachFileOutlined,
} from "@vicons/material";
import { useDateLocale } from "@/composables/useDateLocale";

// Stores
const emailStore = useEmailStore();
const uiStore = useUIStore();

// i18n
const { t } = useI18n();

// 日期格式化
const { currentDateLocale } = useDateLocale();

// 搜索关键词
const searchQuery = ref("");

// 格式化日期
function formatDate(date: Date): string {
    return formatDistanceToNow(new Date(date), {
        addSuffix: false,
        locale: currentDateLocale.value,
    });
}

// 处理搜索
function handleSearch() {
    emailStore.setSearchQuery(searchQuery.value);
}

// 选择邮件
function handleEmailClick(email: any) {
    emailStore.selectEmail(email);
}

// 刷新邮件列表
async function handleRefresh() {
    await emailStore.fetchEmails();
    uiStore.showSuccess(t('email.refreshSuccess'));
}

// 清除搜索
function clearSearch() {
    searchQuery.value = "";
    emailStore.setSearchQuery("");
}
</script>

<template>
    <div class="list-panel">
        <!-- 搜索栏 -->
        <div class="list-header">
            <div class="search-container">
                <SearchOutlined class="search-icon" :size="18" />
                <input
                    v-model="searchQuery"
                    class="search-input"
                    :placeholder="t('email.searchPlaceholder')"
                    @input="handleSearch"
                />
                <button
                    v-if="searchQuery"
                    class="icon-btn-sm"
                    @click="clearSearch"
                >
                    <CloseOutlined :size="16" />
                </button>
                <kbd class="search-shortcut">⌘K</kbd>
            </div>

            <!-- 工具栏 -->
            <div class="list-toolbar">
                <button class="icon-btn-sm" :title="t('common.refresh')" @click="handleRefresh">
                    <RefreshOutlined :size="18" />
                </button>
                <button class="icon-btn-sm" :title="t('common.filter')">
                    <FilterListOutlined :size="18" />
                </button>
                <button class="icon-btn-sm" :title="t('common.selectAll')">
                    <CheckBoxOutlineBlankOutlined :size="18" />
                </button>
            </div>
        </div>

        <!-- 邮件列表 -->
        <div class="email-list">
            <!-- 加载状态 -->
            <div v-if="emailStore.isLoading" class="loading-state">
                <div class="skeleton skeleton-text" style="width: 80%"></div>
                <div class="skeleton skeleton-text" style="width: 60%"></div>
                <div class="skeleton skeleton-text" style="width: 90%"></div>
            </div>

            <!-- 空状态 -->
            <div
                v-else-if="emailStore.filteredEmails.length === 0"
                class="empty-state"
            >
                <EmailOutlined :size="48" />
                <h3>{{ t('email.noEmails') }}</h3>
                <p>{{ t('email.noEmails') }}</p>
            </div>

            <!-- 邮件项列表 -->
            <div
                v-else
                v-for="email in emailStore.filteredEmails"
                :key="email.id"
                :class="[
                    'email-item',
                    {
                        active: emailStore.currentEmail?.id === email.id,
                        unread: email.unread,
                    },
                ]"
                @click="handleEmailClick(email)"
            >
                <div class="email-item-header">
                    <span class="email-sender">{{ email.sender }}</span>
                    <span class="email-time">{{ formatDate(email.date) }}</span>
                </div>
                <div class="email-subject">{{ email.subject }}</div>
                <div class="email-preview">{{ email.preview }}</div>
                <!-- 元信息行：标签和附件 -->
                <div
                    v-if="
                        email.labels.length > 0 || email.attachments.length > 0
                    "
                    class="email-meta"
                >
                    <div v-if="email.labels.length > 0" class="email-labels">
                        <span
                            v-for="label in email.labels.slice(0, 2)"
                            :key="label"
                            :class="['email-label', label]"
                        >
                            {{ label }}
                        </span>
                    </div>
                    <div
                        v-if="email.attachments.length > 0"
                        class="email-attachments"
                    >
                        <AttachFileOutlined
                            class="attachment-icon"
                            :size="12"
                        />
                        <span>{{ email.attachments.length }}</span>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.loading-state {
    padding: 16px;
}

.loading-state .skeleton {
    margin-bottom: 12px;
}

.email-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 8px;
    gap: 8px;
}

.email-labels {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
}

.email-attachments {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
}

.email-attachments .attachment-icon {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
}
</style>
