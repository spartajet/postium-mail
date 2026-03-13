<script setup lang="ts">
import { ref } from "vue";
import { useEmailStore, useUIStore } from "@/stores";
import { formatDistanceToNow } from "date-fns";
import { zhCN } from "date-fns/locale";

// Stores
const emailStore = useEmailStore();
const uiStore = useUIStore();

// 搜索关键词
const searchQuery = ref("");

// 格式化日期
function formatDate(date: Date): string {
    return formatDistanceToNow(new Date(date), {
        addSuffix: false,
        locale: zhCN,
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
    uiStore.showSuccess("邮件列表已刷新");
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
                <svg
                    class="search-icon"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"
                    />
                </svg>
                <input
                    v-model="searchQuery"
                    class="search-input"
                    placeholder="搜索邮件..."
                    @input="handleSearch"
                />
                <button
                    v-if="searchQuery"
                    class="icon-btn-sm"
                    @click="clearSearch"
                >
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                        />
                    </svg>
                </button>
                <kbd class="search-shortcut">⌘K</kbd>
            </div>

            <!-- 工具栏 -->
            <div class="list-toolbar">
                <button class="icon-btn-sm" title="刷新" @click="handleRefresh">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
                        />
                    </svg>
                </button>
                <button class="icon-btn-sm" title="过滤">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M10 18h4v-2h-4v2zM3 6v2h18V6H3zm3 7h12v-2H6v2z"
                        />
                    </svg>
                </button>
                <button class="icon-btn-sm" title="全选">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-9 14l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                        />
                    </svg>
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
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
                    />
                </svg>
                <h3>没有邮件</h3>
                <p>此文件夹中没有邮件</p>
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
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M16.5 6v11.5c0 2.21-1.79 4-4 4s-4-1.79-4-4V5c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5v10.5c0 .55-.45 1-1 1s-1-.45-1-1V6H10v9.5c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5V5c0-2.21-1.79-4-4-4S7 2.79 7 5v12.5c0 3.04 2.46 5.5 5.5 5.5s5.5-2.46 5.5-5.5V6h-1.5z"
                        />
                    </svg>
                    <span>{{ email.attachments.length }}</span>
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

.email-attachments {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--text-muted);
}

.email-attachments svg {
    width: 14px;
    height: 14px;
}
</style>
