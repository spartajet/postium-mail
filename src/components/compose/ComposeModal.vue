<script setup lang="ts">
import { ref, computed } from "vue";
import { useUIStore, useEmailStore, useAccountStore } from "@/stores";
import { useI18n } from "vue-i18n";
import type { ComposeForm } from "@/types";
import { mockAICompose, delay } from "@/mocks";
import {
    EditOutlined,
    MinimizeOutlined,
    WebAssetOutlined,
    CloseOutlined,
    ExpandMoreOutlined,
    AutoAwesomeOutlined,
    SendOutlined,
    FormatBoldOutlined,
    FormatItalicOutlined,
    FormatUnderlinedOutlined,
    StrikethroughSOutlined,
    FormatListBulletedOutlined,
    FormatListNumberedOutlined,
    LinkOutlined,
    ImageOutlined,
    AttachFileOutlined,
    RefreshOutlined
} from "@vicons/material";

// Stores
const uiStore = useUIStore();
const emailStore = useEmailStore();
const accountStore = useAccountStore();

// i18n
const { t } = useI18n();

// 表单数据
const form = ref<ComposeForm>({
    to: "",
    cc: "",
    bcc: "",
    subject: "",
    body: "",
});

// 显示抄送/密送
const showCcBcc = ref(false);

// AI 面板折叠状态
const aiPanelCollapsed = ref(true);

// AI 提示输入
const aiPrompt = ref("");

// AI 加载状态
const isAILoading = ref(false);

// 发送中
const isSending = ref(false);

// 快捷操作
const quickActions = computed(() => [
    { id: "generate", label: t('ai.generate'), icon: "create" },
    { id: "improve", label: t('ai.improve'), icon: "improve" },
    { id: "shorten", label: t('ai.shorten'), icon: "shorten" },
    { id: "formal", label: t('ai.formal'), icon: "formal" },
]);

// 关闭模态框
function handleClose() {
    uiStore.closeComposeModal();
    resetForm();
}

// 重置表单
function resetForm() {
    form.value = {
        to: "",
        cc: "",
        bcc: "",
        subject: "",
        body: "",
    };
    showCcBcc.value = false;
    aiPanelCollapsed.value = true;
    aiPrompt.value = "";
}

// 发送邮件
async function handleSend() {
    if (!form.value.to || !form.value.subject) {
        uiStore.showError(t('email.to') + t('email.subject'));
        return;
    }

    isSending.value = true;
    try {
        await delay(500);

        // 添加到已发送
        emailStore.addEmail({
            id: `email-${Date.now()}`,
            sender: "我",
            senderEmail: accountStore.currentAccount?.email || "me@postium.com",
            recipient: form.value.to,
            subject: form.value.subject,
            preview: form.value.body.slice(0, 100),
            body: form.value.body,
            date: new Date(),
            unread: false,
            starred: false,
            labels: [],
            folder: "sent",
            attachments: [],
            accountId: accountStore.currentAccount?.id || "acc-1",
        });

        uiStore.showSuccess(t('email.sent'));
        handleClose();
    } catch (error) {
        uiStore.showError(t('toast.error'));
    } finally {
        isSending.value = false;
    }
}

// 保存草稿
async function handleSaveDraft() {
    await delay(300);

    emailStore.addEmail({
        id: `email-${Date.now()}`,
        sender: "我",
        senderEmail: accountStore.currentAccount?.email || "me@postium.com",
        recipient: form.value.to,
        subject: form.value.subject || `(${t('email.noEmails')})`,
        preview: form.value.body.slice(0, 100),
        body: form.value.body,
        date: new Date(),
        unread: false,
        starred: false,
        labels: [],
        folder: "drafts",
        attachments: [],
        accountId: accountStore.currentAccount?.id || "acc-1",
    });

    uiStore.showSuccess(t('email.drafts'));
    handleClose();
}

// 切换 AI 面板
function toggleAIPanel() {
    aiPanelCollapsed.value = !aiPanelCollapsed.value;
}

// 处理 AI 快捷操作
async function handleAIQuickAction(actionId: string) {
    isAILoading.value = true;
    try {
        await delay(800);
        const result = mockAICompose(actionId as any, form.value.body);
        form.value.body = result;
        uiStore.showSuccess(t('ai.generate'));
    } finally {
        isAILoading.value = false;
    }
}

// 处理 AI 提示
async function handleAIPrompt() {
    if (!aiPrompt.value.trim()) return;

    isAILoading.value = true;
    try {
        await delay(1000);
        const result = mockAICompose("generate", aiPrompt.value);
        form.value.body = result;
        aiPrompt.value = "";
        uiStore.showSuccess(t('ai.generate'));
    } finally {
        isAILoading.value = false;
    }
}

// 编辑器工具栏操作
function execCommand(command: string, value?: string) {
    document.execCommand(command, false, value);
}

// 插入链接
function handleInsertLink() {
    const url = window.prompt(t('email.subject'));
    if (url) {
        execCommand("createLink", url);
    }
}

// 插入图片
function handleInsertImage() {
    const url = window.prompt(t('email.subject'));
    if (url) {
        execCommand("insertImage", url);
    }
}
</script>

<template>
    <Teleport to="body">
        <Transition name="modal">
            <div
                v-if="uiStore.modals.compose"
                class="modal-overlay show"
                @click.self="handleClose"
            >
                <div class="modal compose-window">
                    <!-- 头部 -->
                    <div class="modal-header">
                        <h3>
                            <EditOutlined class="modal-icon" :size="20" />
                            {{ t('email.compose') }}
                        </h3>
                        <div class="modal-controls">
                            <button class="icon-btn-sm" :title="t('common.operations')">
                                <MinimizeOutlined :size="16" />
                            </button>
                            <button class="icon-btn-sm" :title="t('common.operations')">
                                <WebAssetOutlined :size="16" />
                            </button>
                            <button
                                class="icon-btn-sm"
                                @click="handleClose"
                                :title="t('common.close')"
                            >
                                <CloseOutlined :size="16" />
                            </button>
                        </div>
                    </div>

                    <!-- 表单字段 -->
                    <div class="compose-fields">
                        <div class="compose-field">
                            <label>{{ t('email.to') }}</label>
                            <input
                                v-model="form.to"
                                type="email"
                                :placeholder="t('email.to')"
                            />
                            <button
                                class="icon-btn-sm"
                                @click="showCcBcc = !showCcBcc"
                                :title="t('email.cc')"
                            >
                                <ExpandMoreOutlined :size="18" />
                            </button>
                        </div>

                        <template v-if="showCcBcc">
                            <div class="compose-field">
                                <label>{{ t('email.cc') }}</label>
                                <input
                                    v-model="form.cc"
                                    type="email"
                                    :placeholder="t('email.cc')"
                                />
                            </div>
                            <div class="compose-field">
                                <label>{{ t('email.bcc') }}</label>
                                <input
                                    v-model="form.bcc"
                                    type="email"
                                    :placeholder="t('email.bcc')"
                                />
                            </div>
                        </template>

                        <div class="compose-field">
                            <label>{{ t('email.subject') }}</label>
                            <input
                                v-model="form.subject"
                                type="text"
                                :placeholder="t('email.subject')"
                            />
                        </div>
                    </div>

                    <!-- AI 写作助手面板 -->
                    <div class="ai-compose-panel">
                        <div class="ai-compose-header" @click="toggleAIPanel">
                            <div class="ai-badge">
                                <AutoAwesomeOutlined :size="16" />
                                <span>AI {{ t('ai.provider') }}</span>
                            </div>
                            <ExpandMoreOutlined
                                class="chevron"
                                :class="{ collapsed: aiPanelCollapsed }"
                                :size="20"
                            />
                        </div>

                        <div
                            class="ai-compose-body"
                            :class="{ collapsed: aiPanelCollapsed }"
                        >
                            <!-- 快捷操作 -->
                            <div class="ai-quick-actions">
                                <button
                                    v-for="action in quickActions"
                                    :key="action.id"
                                    class="ai-quick-btn"
                                    :disabled="isAILoading"
                                    @click="handleAIQuickAction(action.id)"
                                >
                                    <EditOutlined v-if="action.icon === 'create'" :size="16" />
                                    <AutoAwesomeOutlined v-else-if="action.icon === 'improve'" :size="16" />
                                    <MinimizeOutlined v-else-if="action.icon === 'shorten'" :size="16" />
                                    <RefreshOutlined v-else :size="16" />
                                    <span>{{ action.label }}</span>
                                </button>
                            </div>

                            <!-- 自定义提示 -->
                            <div class="ai-prompt-box">
                                <input
                                    v-model="aiPrompt"
                                    type="text"
                                    :placeholder="t('ai.generate')"
                                    @keyup.enter="handleAIPrompt"
                                />
                                <button
                                    class="ai-send-btn"
                                    :disabled="isAILoading || !aiPrompt.trim()"
                                    @click="handleAIPrompt"
                                >
                                    <SendOutlined :size="18" />
                                </button>
                            </div>
                        </div>
                    </div>

                    <!-- 编辑器工具栏 -->
                    <div class="editor-toolbar">
                        <button
                            class="toolbar-btn"
                            :title="t('editor.bold')"
                            @click="execCommand('bold')"
                        >
                            <FormatBoldOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            :title="t('editor.italic')"
                            @click="execCommand('italic')"
                        >
                            <FormatItalicOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            :title="t('editor.underline')"
                            @click="execCommand('underline')"
                        >
                            <FormatUnderlinedOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            :title="t('editor.strikethrough')"
                            @click="execCommand('strikeThrough')"
                        >
                            <StrikethroughSOutlined :size="18" />
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            class="toolbar-btn"
                            :title="t('editor.unorderedList')"
                            @click="execCommand('insertUnorderedList')"
                        >
                            <FormatListBulletedOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            :title="t('editor.orderedList')"
                            @click="execCommand('insertOrderedList')"
                        >
                            <FormatListNumberedOutlined :size="18" />
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            class="toolbar-btn"
                            :title="t('editor.insertLink')"
                            @click="handleInsertLink"
                        >
                            <LinkOutlined :size="18" />
                        </button>

                        <button
                            class="toolbar-btn"
                            :title="t('editor.insertImage')"
                            @click="handleInsertImage"
                        >
                            <ImageOutlined :size="18" />
                        </button>

                        <button
                            class="toolbar-btn"
                            :title="t('editor.addAttachment')"
                        >
                            <AttachFileOutlined :size="18" />
                        </button>
                    </div>

                    <!-- 编辑区域 -->
                    <div
                        class="modal-body"
                        style="padding: 0; flex: 1; overflow: hidden"
                    >
                        <div
                            class="compose-editor"
                            contenteditable="true"
                            :data-placeholder="t('email.editorPlaceholder')"
                            v-html="form.body"
                            @input="
                                form.body = (
                                    $event.target as HTMLElement
                                ).innerHTML
                            "
                        ></div>
                    </div>

                    <!-- 底部操作栏 -->
                    <div class="modal-footer">
                        <button
                            class="btn btn-primary"
                            :disabled="isSending"
                            @click="handleSend"
                        >
                            <SendOutlined :size="16" />
                            <span>{{
                                isSending
                                    ? t("email.sending")
                                    : t("email.sent")
                            }}</span>
                        </button>
                        <button class="btn btn-ghost" @click="handleSaveDraft">
                            {{ t("email.saveDraft") }}
                        </button>
                        <button class="btn btn-ghost" @click="handleClose">
                            {{ t("common.cancel") }}
                        </button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.compose-window {
    width: 90%;
    max-width: 800px;
    height: 80vh;
    display: flex;
    flex-direction: column;
}

.compose-editor {
    flex: 1;
    padding: 16px;
    overflow-y: auto;
    font-size: 14px;
    line-height: 1.6;
    color: var(--text-primary);
    background: var(--bg-base);
    min-height: 200px;
}

.compose-editor:empty::before {
    content: attr(data-placeholder);
    color: var(--text-muted);
}

.compose-editor:focus {
    outline: none;
}
</style>
