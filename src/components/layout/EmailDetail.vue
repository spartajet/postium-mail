<script setup lang="ts">
import { ref, watch, computed } from "vue";
import { useEmailStore, useUIStore } from "@/stores";
import { useI18n } from "vue-i18n";
import { format } from "date-fns";
import { mockAISummary } from "@/mocks";
import { useDateLocale } from "@/composables/useDateLocale";
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
    AttachFileOutlined,
    DescriptionOutlined,
    AutoAwesomeOutlined,
    NotesOutlined,
    TranslateOutlined,
    PictureAsPdfOutlined,
    ImageOutlined,
    VideoFileOutlined,
    AudioFileOutlined,
    TextSnippetOutlined,
    TableChartOutlined,
    FolderZipOutlined,
    ArticleOutlined,
    CodeOutlined,
    InsertDriveFileOutlined,
} from "@vicons/material";
import type { Component } from "vue";

// Stores
const emailStore = useEmailStore();
const uiStore = useUIStore();

// i18n
const { t } = useI18n();

// 日期格式化
const { currentDateLocale } = useDateLocale();

// AI 摘要
const aiSummary = ref("");
const isLoadingSummary = ref(false);

// 格式化完整日期
function formatFullDate(date: Date): string {
    const locale = currentDateLocale.value;
    return format(new Date(date), "yyyy年MM月dd日 EEEE HH:mm", {
        locale,
    });
}

// 获取头像首字母
function getInitials(name: string): string {
    return name.charAt(0).toUpperCase();
}

// 文件类型图标和颜色映射（基于行业标准 - Gmail、Apple Mail、Outlook）
const fileTypeMap: Record<string, { icon: Component; color: string }> = {
    // PDF - 红色
    'pdf': { icon: PictureAsPdfOutlined, color: '#F40F02' },
    // 图片 - 紫色
    'jpg': { icon: ImageOutlined, color: '#9C27B0' },
    'jpeg': { icon: ImageOutlined, color: '#9C27B0' },
    'png': { icon: ImageOutlined, color: '#9C27B0' },
    'gif': { icon: ImageOutlined, color: '#9C27B0' },
    'bmp': { icon: ImageOutlined, color: '#9C27B0' },
    'svg': { icon: ImageOutlined, color: '#9C27B0' },
    'webp': { icon: ImageOutlined, color: '#9C27B0' },
    'ico': { icon: ImageOutlined, color: '#9C27B0' },
    // 视频 - 橙色
    'mp4': { icon: VideoFileOutlined, color: '#FF9800' },
    'avi': { icon: VideoFileOutlined, color: '#FF9800' },
    'mov': { icon: VideoFileOutlined, color: '#FF9800' },
    'wmv': { icon: VideoFileOutlined, color: '#FF9800' },
    'flv': { icon: VideoFileOutlined, color: '#FF9800' },
    'mkv': { icon: VideoFileOutlined, color: '#FF9800' },
    'webm': { icon: VideoFileOutlined, color: '#FF9800' },
    // 音频 - 蓝色
    'mp3': { icon: AudioFileOutlined, color: '#2196F3' },
    'wav': { icon: AudioFileOutlined, color: '#2196F3' },
    'flac': { icon: AudioFileOutlined, color: '#2196F3' },
    'aac': { icon: AudioFileOutlined, color: '#2196F3' },
    'ogg': { icon: AudioFileOutlined, color: '#2196F3' },
    'm4a': { icon: AudioFileOutlined, color: '#2196F3' },
    'wma': { icon: AudioFileOutlined, color: '#2196F3' },
    // Microsoft Office 文档
    'doc': { icon: ArticleOutlined, color: '#2B579A' },   // Word 蓝色
    'docx': { icon: ArticleOutlined, color: '#2B579A' },
    'xls': { icon: TableChartOutlined, color: '#217346' }, // Excel 绿色
    'xlsx': { icon: TableChartOutlined, color: '#217346' },
    'ppt': { icon: ArticleOutlined, color: '#D24726' },   // PowerPoint 橙红色
    'pptx': { icon: ArticleOutlined, color: '#D24726' },
    // 其他文档
    'txt': { icon: TextSnippetOutlined, color: '#757575' },
    'rtf': { icon: TextSnippetOutlined, color: '#757575' },
    'odt': { icon: TextSnippetOutlined, color: '#757575' },
    'ods': { icon: TableChartOutlined, color: '#4CAF50' },
    'odp': { icon: ArticleOutlined, color: '#F57C00' },
    // 压缩包 - 绿色
    'zip': { icon: FolderZipOutlined, color: '#4CAF50' },
    'rar': { icon: FolderZipOutlined, color: '#4CAF50' },
    '7z': { icon: FolderZipOutlined, color: '#4CAF50' },
    'tar': { icon: FolderZipOutlined, color: '#4CAF50' },
    'gz': { icon: FolderZipOutlined, color: '#4CAF50' },
    // 代码 - 紫灰色
    'js': { icon: CodeOutlined, color: '#7E57C2' },
    'ts': { icon: CodeOutlined, color: '#7E57C2' },
    'html': { icon: CodeOutlined, color: '#7E57C2' },
    'css': { icon: CodeOutlined, color: '#7E57C2' },
    'json': { icon: CodeOutlined, color: '#7E57C2' },
    'xml': { icon: CodeOutlined, color: '#7E57C2' },
    'py': { icon: CodeOutlined, color: '#7E57C2' },
    'java': { icon: CodeOutlined, color: '#7E57C2' },
    'cpp': { icon: CodeOutlined, color: '#7E57C2' },
    'c': { icon: CodeOutlined, color: '#7E57C2' },
    'go': { icon: CodeOutlined, color: '#7E57C2' },
    'rs': { icon: CodeOutlined, color: '#7E57C2' },
    'php': { icon: CodeOutlined, color: '#7E57C2' },
    // 其他
    'exe': { icon: InsertDriveFileOutlined, color: '#607D8B' },
    'msi': { icon: InsertDriveFileOutlined, color: '#607D8B' },
};

// 获取附件图标
function getAttachmentIcon(filename: string): Component {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    return fileTypeMap[ext]?.icon || DescriptionOutlined;
}

// 获取附件颜色
function getAttachmentColor(filename: string): string {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    return fileTypeMap[ext]?.color || '#757575';
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
            ? t('email.starred')
            : t('email.archive');
        uiStore.showSuccess(action);
    }
}

// 删除邮件
function handleDelete() {
    if (emailStore.currentEmail) {
        emailStore.deleteEmail(emailStore.currentEmail.id);
        uiStore.showSuccess(t('email.trash'));
    }
}

// 归档邮件
function handleArchive() {
    if (emailStore.currentEmail) {
        emailStore.moveEmail(emailStore.currentEmail.id, "trash");
        uiStore.showSuccess(t('email.archive'));
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
            uiStore.showInfo(t('ai.generate'));
            break;
        case "summary":
            loadAISummary();
            break;
        case "translate":
            uiStore.showInfo(t('ai.translate'));
            break;
        case "tasks":
            uiStore.showInfo(t('ai.tasks'));
            break;
        default:
            uiStore.showInfo(`${t('common.operations')}: ${action}`);
    }
}

// AI 操作列表
const aiActions = computed(() => [
    { id: "reply", label: t('ai.smartReply'), icon: "reply" },
    { id: "summary", label: t('ai.summary'), icon: "summary" },
    { id: "translate", label: t('ai.translate'), icon: "translate" },
    { id: "tasks", label: t('ai.tasks'), icon: "tasks" },
]);
</script>

<template>
    <div class="detail-panel">
        <!-- 空状态 -->
        <div v-if="!emailStore.currentEmail" class="empty-state">
            <div class="empty-icon">
                <EmailOutlined :size="48" />
            </div>
            <h3>{{ t('email.noEmails') }}</h3>
            <p>{{ t('email.noEmails') }}</p>
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
                        :title="t('email.from')"
                    >
                        <KeyboardArrowUpOutlined :size="20" />
                    </button>
                    <button
                        class="icon-btn"
                        :disabled="!emailStore.hasNextEmail"
                        @click="handleNavigate('next')"
                        :title="t('email.to')"
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
                        {{ t('email.to') }}: {{ emailStore.currentEmail.recipient }}
                    </div>
                </div>
            </div>

            <!-- AI 摘要卡片 -->
            <div v-if="isLoadingSummary || aiSummary" class="ai-card">
                <div class="ai-card-header">
                    <div class="ai-badge">
                        <AutoAwesomeOutlined :size="16" />
                        <span>{{ t('ai.summary') }}</span>
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
                    <span>{{ t('email.attachments') }} ({{ emailStore.currentEmail.attachments.length }})</span>
                </div>
                <div class="attachments-list">
                    <div
                        v-for="(attachment, index) in emailStore.currentEmail
                            .attachments"
                        :key="index"
                        class="attachment-item"
                    >
                        <div class="attachment-icon" :style="{ color: getAttachmentColor(attachment.name) }">
                            <component :is="getAttachmentIcon(attachment.name)" :size="24" />
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
                <button class="icon-btn" @click="handleReply" :title="t('email.reply')">
                    <ReplyOutlined :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="handleForward"
                    :title="t('email.forward')"
                >
                    <ForwardOutlined :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="handleToggleStar"
                    :title="t('email.starred')"
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
                    :title="t('email.archive')"
                >
                    <ArchiveOutlined :size="18" />
                </button>
                <button
                    class="icon-btn danger"
                    @click="handleDelete"
                    :title="t('common.delete')"
                >
                    <DeleteOutlined :size="18" />
                </button>
            </div>

            <!-- AI 操作栏 -->
            <div class="ai-actions">
                <div class="ai-actions-label">
                    <AutoAwesomeOutlined :size="16" />
                    <span>AI {{ t('ai.provider') }}</span>
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

/* 附件区域样式 */
.attachments-section {
    padding: 16px 20px;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
}

.attachments-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-secondary);
}

.attachments-list {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
}

.attachment-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-glass);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s ease;
    min-width: 0;
    flex-shrink: 0;
}

.attachment-item:hover {
    background: var(--bg-glass-hover);
    border-color: var(--primary-color);
}

.attachment-icon {
    flex-shrink: 0;
    color: var(--text-muted);
}

.attachment-info {
    min-width: 0;
    flex: 1;
    overflow: hidden;
}

.attachment-name {
    font-size: $font-size-base;
    font-weight: $font-weight-medium;
    color: var(--text-primary);
    word-break: break-word;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.attachment-size {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
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
