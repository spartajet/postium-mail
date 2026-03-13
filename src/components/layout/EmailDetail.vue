<script setup lang="ts">
import { ref, watch } from "vue";
import { useEmailStore, useUIStore } from "@/stores";
import { format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { mockAISummary } from "@/mocks";

// Stores
const emailStore = useEmailStore();
const uiStore = useUIStore();

// AI 摘要
const aiSummary = ref("");
const isLoadingSummary = ref(false);

// 格式化完整日期
function formatFullDate(date: Date): string {
    return format(new Date(date), "yyyy年MM月dd日 EEEE HH:mm", {
        locale: zhCN,
    });
}

// 获取头像首字母
function getInitials(name: string): string {
    return name.charAt(0).toUpperCase();
}

// 加载 AI 摘要
async function loadAISummary() {
    if (!emailStore.currentEmail) return;

    isLoadingSummary.value = true;
    try {
        // 模拟 AI 处理延迟
        await new Promise((resolve) => setTimeout(resolve, 800));
        aiSummary.value = mockAISummary(emailStore.currentEmail);
    } finally {
        isLoadingSummary.value = false;
    }
}

// 监听当前邮件变化
watch(
    () => emailStore.currentEmail,
    (newEmail) => {
        if (newEmail) {
            aiSummary.value = "";
            loadAISummary();
        }
    },
);

// 回复邮件
function handleReply() {
    uiStore.openComposeModal();
    // TODO: 设置回复数据
}

// 转发邮件
function handleForward() {
    uiStore.openComposeModal();
    // TODO: 设置转发数据
}

// 切换星标
function handleToggleStar() {
    if (emailStore.currentEmail) {
        emailStore.toggleStar(emailStore.currentEmail.id);
        const action = emailStore.currentEmail.starred
            ? "已添加星标"
            : "已移除星标";
        uiStore.showSuccess(action);
    }
}

// 删除邮件
function handleDelete() {
    if (emailStore.currentEmail) {
        emailStore.deleteEmail(emailStore.currentEmail.id);
        uiStore.showSuccess("邮件已移至垃圾箱");
    }
}

// 归档邮件
function handleArchive() {
    if (emailStore.currentEmail) {
        emailStore.moveEmail(emailStore.currentEmail.id, "trash");
        uiStore.showSuccess("邮件已归档");
    }
}

// 导航邮件
function handleNavigate(direction: "prev" | "next") {
    if (direction === "prev") {
        emailStore.navigatePrevious();
    } else {
        emailStore.navigateNext();
    }
}

// AI 操作
function handleAIAction(action: string) {
    switch (action) {
        case "reply":
            uiStore.showInfo("正在生成智能回复...");
            break;
        case "summary":
            loadAISummary();
            break;
        case "translate":
            uiStore.showInfo("正在翻译邮件...");
            break;
        case "tasks":
            uiStore.showInfo("正在提取任务...");
            break;
        default:
            uiStore.showInfo(`执行操作: ${action}`);
    }
}

// 标签颜色映射（保留用于将来扩展）
// const labelColors: Record<string, string> = {
//   urgent: '#EF4444',
//   work: '#3B82F6',
//   personal: '#10B981',
//   finance: '#F59E0B',
//   travel: '#8B5CF6',
//   newsletter: '#06B6D4',
// }

// AI 操作列表
const aiActions = [
    { id: "reply", label: "智能回复", icon: "reply" },
    { id: "summary", label: "总结", icon: "summary" },
    { id: "translate", label: "翻译", icon: "translate" },
    { id: "tasks", label: "提取任务", icon: "tasks" },
];
</script>

<template>
    <div class="detail-panel">
        <!-- 空状态 -->
        <div v-if="!emailStore.currentEmail" class="empty-state">
            <div class="empty-icon">
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
            <h3>选择一封邮件</h3>
            <p>从左侧列表中选择邮件查看详情</p>
        </div>

        <!-- 邮件详情 -->
        <div v-else class="email-detail">
            <!-- 顶部工具栏 -->
            <div class="detail-header">
                <div class="detail-nav">
                    <button
                        class="icon-btn"
                        :disabled="!emailStore.hasPreviousEmail"
                        @click="handleNavigate('prev')"
                        title="上一封"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M7.41 15.41L12 10.83l4.59 4.58L18 14l-6-6-6 6z"
                            />
                        </svg>
                    </button>
                    <button
                        class="icon-btn"
                        :disabled="!emailStore.hasNextEmail"
                        @click="handleNavigate('next')"
                        title="下一封"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z"
                            />
                        </svg>
                    </button>
                </div>

                <div class="detail-actions">
                    <button class="icon-btn" @click="handleReply" title="回复">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z"
                            />
                        </svg>
                    </button>
                    <button
                        class="icon-btn"
                        @click="handleForward"
                        title="转发"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M14 9V5l7 7-7 7v-4.1c-5 0-8.5 1.6-11 5.1 1-5 4-10 11-11z"
                            />
                        </svg>
                    </button>
                    <button
                        class="icon-btn"
                        @click="handleToggleStar"
                        title="星标"
                    >
                        <svg
                            v-if="emailStore.currentEmail.starred"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                            style="color: #f59e0b"
                        >
                            <path
                                d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
                            />
                        </svg>
                        <svg
                            v-else
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
                            />
                        </svg>
                    </button>
                    <button
                        class="icon-btn"
                        @click="handleArchive"
                        title="归档"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M20.54 5.23l-1.39-1.68C18.88 3.21 18.47 3 18 3H6c-.47 0-.88.21-1.16.55L3.46 5.23C3.17 5.57 3 6.02 3 6.5V19c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V6.5c0-.48-.17-.93-.46-1.27zM12 17.5L6.5 12H10v-2h4v2h3.5L12 17.5zM5.12 5l.81-1h12l.94 1H5.12z"
                            />
                        </svg>
                    </button>
                    <button
                        class="icon-btn danger"
                        @click="handleDelete"
                        title="删除"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                            />
                        </svg>
                    </button>
                </div>
            </div>

            <!-- 邮件主题 -->
            <h2 class="detail-subject">
                {{ emailStore.currentEmail.subject }}
            </h2>

            <!-- 邮件元信息 -->
            <div class="detail-meta">
                <div class="sender-info">
                    <div
                        class="sender-avatar"
                        :style="{
                            backgroundColor:
                                emailStore.currentEmail.accountId || '#7C3AED',
                        }"
                    >
                        {{ getInitials(emailStore.currentEmail.sender) }}
                    </div>
                    <div class="sender-details">
                        <div class="sender-name">
                            {{ emailStore.currentEmail.sender }}
                        </div>
                        <div class="sender-email">
                            &lt;{{ emailStore.currentEmail.senderEmail }}&gt;
                        </div>
                    </div>
                </div>
                <div class="email-meta-info">
                    <div class="email-date">
                        {{ formatFullDate(emailStore.currentEmail.date) }}
                    </div>
                    <div class="email-to">
                        收件人: {{ emailStore.currentEmail.recipient }}
                    </div>
                </div>
            </div>

            <!-- AI 摘要卡片 -->
            <div v-if="isLoadingSummary || aiSummary" class="ai-card">
                <div class="ai-card-header">
                    <div class="ai-badge">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                            />
                        </svg>
                        <span>AI 摘要</span>
                    </div>
                </div>
                <div class="ai-card-content">
                    <div
                        v-if="isLoadingSummary"
                        class="skeleton skeleton-text"
                        style="width: 90%"
                    ></div>
                    <div v-else>
                        <ul>
                            <li
                                v-for="(line, index) in aiSummary.split('\n')"
                                :key="index"
                            >
                                {{ line.replace("• ", "") }}
                            </li>
                        </ul>
                    </div>
                </div>
            </div>

            <!-- 邮件正文 -->
            <div
                class="detail-body"
                v-html="emailStore.currentEmail.body"
            ></div>

            <!-- 附件区域 -->
            <div
                v-if="emailStore.currentEmail.attachments.length > 0"
                class="attachments-section"
            >
                <div class="attachments-header">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M16.5 6v11.5c0 2.21-1.79 4-4 4s-4-1.79-4-4V5c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5v10.5c0 .55-.45 1-1 1s-1-.45-1-1V6H10v9.5c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5V5c0-2.21-1.79-4-4-4S7 2.79 7 5v12.5c0 3.04 2.46 5.5 5.5 5.5s5.5-2.46 5.5-5.5V6h-1.5z"
                        />
                    </svg>
                    <span
                        >附件 ({{
                            emailStore.currentEmail.attachments.length
                        }})</span
                    >
                </div>
                <div class="attachments-list">
                    <div
                        v-for="(attachment, index) in emailStore.currentEmail
                            .attachments"
                        :key="index"
                        class="attachment-item"
                    >
                        <div class="attachment-icon">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"
                                />
                            </svg>
                        </div>
                        <div class="attachment-info">
                            <div class="attachment-name">
                                {{ attachment.name }}
                            </div>
                            <div class="attachment-size">
                                {{ attachment.size }}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- AI 操作栏 -->
            <div class="ai-actions">
                <div class="ai-actions-label">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                        />
                    </svg>
                    <span>AI 助手</span>
                </div>
                <div class="ai-actions-buttons">
                    <button
                        v-for="action in aiActions"
                        :key="action.id"
                        class="ai-action-btn"
                        @click="handleAIAction(action.id)"
                    >
                        <svg
                            v-if="action.icon === 'reply'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M10 9V5l-7 7 7 7v-4.1c5 0 8.5 1.6 11 5.1-1-5-4-10-11-11z"
                            />
                        </svg>
                        <svg
                            v-else-if="action.icon === 'summary'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-5 14H7v-2h7v2zm3-4H7v-2h10v2zm0-4H7V7h10v2z"
                            />
                        </svg>
                        <svg
                            v-else-if="action.icon === 'translate'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12.87 15.07l-2.54-2.51.03-.03c1.74-1.94 2.98-4.17 3.71-6.53H17V4h-7V2H8v2H1v1.99h11.17C11.5 7.92 10.44 9.75 9 11.35 8.07 10.32 7.3 9.19 6.69 8h-2c.73 1.63 1.73 3.17 2.98 4.56l-5.09 5.02L4 19l5-5 3.11 3.11.76-2.04zM18.5 10h-2L12 22h2l1.12-3h4.75L21 22h2l-4.5-12zm-2.62 7l1.62-4.33L19.12 17h-3.24z"
                            />
                        </svg>
                        <svg
                            v-else
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-5 14H7v-2h7v2zm3-4H7v-2h10v2zm0-4H7V7h10v2z"
                            />
                        </svg>
                        <span>{{ action.label }}</span>
                    </button>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped>
.email-detail {
    display: flex;
    flex-direction: column;
    height: 100%;
}

.skeleton-text {
    height: 14px;
    margin-bottom: 8px;
    background: linear-gradient(
        90deg,
        var(--bg-glass) 25%,
        var(--bg-glass-hover) 50%,
        var(--bg-glass) 75%
    );
    background-size: 200% 100%;
    animation: skeleton-loading 1.5s infinite;
    border-radius: 4px;
}

@keyframes skeleton-loading {
    0% {
        background-position: 200% 0;
    }
    100% {
        background-position: -200% 0;
    }
}
</style>
