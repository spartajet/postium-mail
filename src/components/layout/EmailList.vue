<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from "vue";
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
    ExpandMoreOutlined,
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

// 邮件列表容器的 ref
const emailListContainer = ref<HTMLElement | null>(null);
const loadMoreTrigger = ref<HTMLElement | null>(null);

// Intersection Observer 用于无限滚动
let observer: IntersectionObserver | null = null;

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

// 设置无限滚动监听
function setupInfiniteScroll() {
    // 先清理旧的 observer
    cleanupInfiniteScroll();

    // 等待 DOM 更新
    setTimeout(() => {
        if (!loadMoreTrigger.value) {
            console.log('[EmailList] loadMoreTrigger 未找到，稍后重试');
            return;
        }

        console.log('[EmailList] 设置无限滚动监听');

        // 创建 Intersection Observer
        observer = new IntersectionObserver(
            (entries) => {
                entries.forEach((entry) => {
                    if (entry.isIntersecting && !emailStore.isLoadingMore && emailStore.hasMore) {
                        console.log('[EmailList] 触发加载更多');
                        emailStore.loadMore();
                    }
                });
            },
            {
                root: emailListContainer.value,
                rootMargin: '200px', // 提前200px触发加载
                threshold: 0.1
            }
        );

        // 开始观察触发器元素
        observer.observe(loadMoreTrigger.value);
    }, 100);
}

// 清理 Intersection Observer
function cleanupInfiniteScroll() {
    if (observer) {
        observer.disconnect();
        observer = null;
    }
}

// 组件挂载时设置无限滚动
onMounted(() => {
    setupInfiniteScroll();
});

// 组件卸载时清理
onUnmounted(() => {
    cleanupInfiniteScroll();
});

// 监听文件夹变化，重置无限滚动
watch(() => emailStore.currentFolder, () => {
    console.log('[EmailList] 文件夹变化，重置无限滚动');
    cleanupInfiniteScroll();
    // 延迟设置，确保 DOM 更新完成
    setTimeout(() => {
        setupInfiniteScroll();
    }, 100);
});

// 监听邮件列表变化，重新设置触发器
watch(() => emailStore.filteredEmails, () => {
    console.log('[EmailList] 邮件列表变化，重新设置触发器');
    setTimeout(() => {
        setupInfiniteScroll();
    }, 100);
});
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
        <div ref="emailListContainer" class="email-list">
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
            <template v-else>
                <div
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

                <!-- 加载更多指示器 -->
                <div ref="loadMoreTrigger" class="load-more-trigger">
                    <div v-if="emailStore.isLoadingMore" class="loading-more">
                        <div class="loading-spinner"></div>
                        <span>加载中...</span>
                    </div>
                    <div v-else-if="!emailStore.hasMore" class="no-more">
                        <span>没有更多邮件了</span>
                    </div>
                    <div v-else class="load-more-hint">
                        <ExpandMoreOutlined :size="20" />
                        <span>向下滑动加载更多</span>
                    </div>
                </div>
            </template>
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

/* 加载更多指示器 */
.load-more-trigger {
    padding: 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 14px;
    min-height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
}

.loading-more {
    display: flex;
    align-items: center;
    gap: 12px;
}

.loading-spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border-color);
    border-top-color: var(--primary-color);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

.no-more {
    color: var(--text-muted);
    font-size: 13px;
}

.load-more-hint {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    opacity: 0.5;
    transition: opacity 0.2s;
}

.load-more-hint:hover {
    opacity: 0.8;
}
</style>
