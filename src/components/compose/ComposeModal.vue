<script setup lang="ts">
import { ref } from "vue";
import { useUIStore, useEmailStore, useAccountStore } from "@/stores";
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
const quickActions = [
    { id: "generate", label: "生成草稿", icon: "create" },
    { id: "improve", label: "改进文笔", icon: "improve" },
    { id: "shorten", label: "精简内容", icon: "shorten" },
    { id: "formal", label: "正式化", icon: "formal" },
];

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
        uiStore.showError("请填写收件人和主题");
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

        uiStore.showSuccess("邮件已发送");
        handleClose();
    } catch (error) {
        uiStore.showError("发送失败，请重试");
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
        subject: form.value.subject || "(无主题)",
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

    uiStore.showSuccess("草稿已保存");
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
        uiStore.showSuccess("AI 已生成内容");
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
        uiStore.showSuccess("AI 已生成内容");
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
    const url = window.prompt("输入链接地址");
    if (url) {
        execCommand("createLink", url);
    }
}

// 插入图片
function handleInsertImage() {
    const url = window.prompt("输入图片地址");
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
                            写信
                        </h3>
                        <div class="modal-controls">
                            <button class="icon-btn-sm" title="最小化">
                                <MinimizeOutlined :size="16" />
                            </button>
                            <button class="icon-btn-sm" title="最大化">
                                <WebAssetOutlined :size="16" />
                            </button>
                            <button
                                class="icon-btn-sm"
                                @click="handleClose"
                                title="关闭"
                            >
                                <CloseOutlined :size="16" />
                            </button>
                        </div>
                    </div>

                    <!-- 表单字段 -->
                    <div class="compose-fields">
                        <div class="compose-field">
                            <label>收件人</label>
                            <input
                                v-model="form.to"
                                type="email"
                                placeholder="输入邮箱地址"
                            />
                            <button
                                class="icon-btn-sm"
                                @click="showCcBcc = !showCcBcc"
                                title="抄送/密送"
                            >
                                <ExpandMoreOutlined :size="18" />
                            </button>
                        </div>

                        <template v-if="showCcBcc">
                            <div class="compose-field">
                                <label>抄送</label>
                                <input
                                    v-model="form.cc"
                                    type="email"
                                    placeholder="抄送邮箱地址"
                                />
                            </div>
                            <div class="compose-field">
                                <label>密送</label>
                                <input
                                    v-model="form.bcc"
                                    type="email"
                                    placeholder="密送邮箱地址"
                                />
                            </div>
                        </template>

                        <div class="compose-field">
                            <label>主题</label>
                            <input
                                v-model="form.subject"
                                type="text"
                                placeholder="输入主题"
                            />
                        </div>
                    </div>

                    <!-- AI 写作助手面板 -->
                    <div class="ai-compose-panel">
                        <div class="ai-compose-header" @click="toggleAIPanel">
                            <div class="ai-badge">
                                <AutoAwesomeOutlined :size="16" />
                                <span>AI 写作助手</span>
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
                                    placeholder="输入指令，如：帮我写一封感谢信..."
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
                            title="粗体"
                            @click="execCommand('bold')"
                        >
                            <FormatBoldOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            title="斜体"
                            @click="execCommand('italic')"
                        >
                            <FormatItalicOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            title="下划线"
                            @click="execCommand('underline')"
                        >
                            <FormatUnderlinedOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            title="删除线"
                            @click="execCommand('strikeThrough')"
                        >
                            <StrikethroughSOutlined :size="18" />
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            class="toolbar-btn"
                            title="无序列表"
                            @click="execCommand('insertUnorderedList')"
                        >
                            <FormatListBulletedOutlined :size="18" />
                        </button>
                        <button
                            class="toolbar-btn"
                            title="有序列表"
                            @click="execCommand('insertOrderedList')"
                        >
                            <FormatListNumberedOutlined :size="18" />
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            class="toolbar-btn"
                            title="插入链接"
                            @click="handleInsertLink"
                        >
                            <LinkOutlined :size="18" />
                        </button>

                        <button
                            class="toolbar-btn"
                            title="插入图片"
                            @click="handleInsertImage"
                        >
                            <ImageOutlined :size="18" />
                        </button>

                        <button class="toolbar-btn" title="添加附件">
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
                            data-placeholder="在此输入邮件内容..."
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
                            <span>{{ isSending ? "发送中..." : "发送" }}</span>
                        </button>
                        <button class="btn btn-ghost" @click="handleSaveDraft">
                            存为草稿
                        </button>
                        <button class="btn btn-ghost" @click="handleClose">
                            取消
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
