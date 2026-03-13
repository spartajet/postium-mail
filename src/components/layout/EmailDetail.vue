<script setup lang="ts">
import { ref, watch, h } from "vue";
import { useEmailStore, useUIStore } from "@/stores";
import { format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { mockAISummary } from "@/mocks";
import {
    EmailOutlined,
    KeyboardArrowUpOutlined,
    KeyboardArrowDownOutlined,
    ReplyOutlined,
    ForwardOutlined,
    StarOutlined,
    StarBorderOutlined,
    ArchiveOutlined,
    DeleteOutlined,
    CheckCircleOutlined,
    AttachFileOutlined,
    DescriptionOutlined,
    AutoAwesomeOutlined,
    NotesOutlined,
    TranslateOutlined
} from "@vicons/material";

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
                <EmailOutlined :size="48" />
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
                        <KeyboardArrowUpOutlined :size="20" />
                    </button>
                    <button
                        class="icon-btn"
                        :disabled="!emailStore.hasNextEmail"
                        @click="handleNavigate('next')"
                        title="下一封"
                    >
                        <KeyboardArrowDownOutlined :size="20" />
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
                        <AutoAwesomeOutlined :size="16" />
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
                    <AttachFileOutlined :size="18" />
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
                            <DescriptionOutlined :size="24" />
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

            <!-- 操作按钮栏 -->
            <div class="detail-actions">
                <button class="icon-btn" @click="handleReply" title="回复">
                    <ReplyOutlined :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="handleForward"
                    title="转发"
                >
                    <ForwardOutlined :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="handleToggleStar"
                    title="星标"
                >
                    <StarOutlined
                        v-if="emailStore.currentEmail.starred"
                        :size="18"
                        style="color: #f59e0b"
                    />
                    <StarBorderOutlined v-else :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="handleArchive"
                    title="归档"
                >
                    <ArchiveOutlined :size="18" />
                </button>
                <button
                    class="icon-btn danger"
                    @click="handleDelete"
                    title="删除"
                >
                    <DeleteOutlined :size="18" />
                </button>
            </div>

            <!-- AI 操作栏 -->
            <div class="ai-actions">
                <div class="ai-actions-label">
                    <AutoAwesomeOutlined :size="16" />
                    <span>AI 助手</span>
                </div>
                <div class="ai-actions-buttons">
                    <button
                        v-for="action in aiActions"
                        :key="action.id"
                        class="ai-action-btn"
                        @click="handleAIAction(action.id)"
                    >
                        <ReplyOutlined v-if="action.icon === 'reply'" :size="16" />
                        <NotesOutlined v-else-if="action.icon === 'summary'" :size="16" />
                        <TranslateOutlined v-else-if="action.icon === 'translate'" :size="16" />
                        <NotesOutlined v-else :size="16" />
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

.detail-actions {
    padding: 12px 20px;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    gap: 4px;
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
