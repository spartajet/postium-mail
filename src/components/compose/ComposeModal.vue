<script setup lang="ts">
import { ref } from "vue";
import { useUIStore, useEmailStore, useAccountStore } from "@/stores";
import type { ComposeForm } from "@/types";
import { mockAICompose, delay } from "@/mocks";

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

// 编辑器引用（保留用于将来扩展）
// const editorRef = ref<any>(null);

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

// 格式化操作
const formatActions = [
    { command: "bold", icon: "bold", title: "粗体" },
    { command: "italic", icon: "italic", title: "斜体" },
    { command: "underline", icon: "underline", title: "下划线" },
    { command: "strikeThrough", icon: "strikethrough", title: "删除线" },
];

const listActions = [
    { command: "insertUnorderedList", icon: "ul", title: "无序列表" },
    { command: "insertOrderedList", icon: "ol", title: "有序列表" },
];
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
                            <svg
                                class="modal-icon"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
                                />
                            </svg>
                            写信
                        </h3>
                        <div class="modal-controls">
                            <button class="icon-btn-sm" title="最小化">
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path d="M19 13H5v-2h14v2z" />
                                </svg>
                            </button>
                            <button class="icon-btn-sm" title="最大化">
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm0 16H5V5h14v14z"
                                    />
                                </svg>
                            </button>
                            <button
                                class="icon-btn-sm"
                                @click="handleClose"
                                title="关闭"
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
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                                    />
                                </svg>
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
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                                    />
                                </svg>
                                <span>AI 写作助手</span>
                            </div>
                            <svg
                                class="chevron"
                                :class="{ collapsed: aiPanelCollapsed }"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M7.41 8.59L12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z"
                                />
                            </svg>
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
                                    <svg
                                        v-if="action.icon === 'create'"
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
                                        />
                                    </svg>
                                    <svg
                                        v-else-if="action.icon === 'improve'"
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                                        />
                                    </svg>
                                    <svg
                                        v-else-if="action.icon === 'shorten'"
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path d="M19 13H5v-2h14v2z" />
                                    </svg>
                                    <svg
                                        v-else
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M12 7V3H2v18h20V7H12zM6 19H4v-2h2v2zm0-4H4v-2h2v2zm0-4H4V9h2v2zm0-4H4V5h2v2zm4 12H8v-2h2v2zm0-4H8v-2h2v2zm0-4H8V9h2v2zm0-4H8V5h2v2zm10 12h-8v-2h2v-2h-2v-2h2v-2h-2V9h8v10zm-2-8h-2v2h2v-2zm0 4h-2v2h2v-2z"
                                        />
                                    </svg>
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
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"
                                        />
                                    </svg>
                                </button>
                            </div>
                        </div>
                    </div>

                    <!-- 编辑器工具栏 -->
                    <div class="editor-toolbar">
                        <button
                            v-for="action in formatActions"
                            :key="action.command"
                            class="toolbar-btn"
                            :title="action.title"
                            @click="execCommand(action.command)"
                        >
                            <svg
                                v-if="action.icon === 'bold'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M15.6 10.79c.97-.67 1.65-1.77 1.65-2.79 0-2.26-1.75-4-4-4H7v14h7.04c2.09 0 3.71-1.7 3.71-3.79 0-1.52-.86-2.82-2.15-3.42zM10 6.5h3c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5h-3v-3zm3.5 9H10v-3h3.5c.83 0 1.5.67 1.5 1.5s-.67 1.5-1.5 1.5z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'italic'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M10 4v3h2.21l-3.42 8H6v3h8v-3h-2.21l3.42-8H18V4z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'underline'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M12 17c3.31 0 6-2.69 6-6V3h-2.5v8c0 1.93-1.57 3.5-3.5 3.5S8.5 12.93 8.5 11V3H6v8c0 3.31 2.69 6 6 6zm-7 2v2h14v-2H5z"
                                />
                            </svg>
                            <svg
                                v-else
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M10 19h4v-3h-4v3zM5 4v3h5v3h4V7h5V4H5zM3 14h18v-2H3v2z"
                                />
                            </svg>
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            v-for="action in listActions"
                            :key="action.command"
                            class="toolbar-btn"
                            :title="action.title"
                            @click="execCommand(action.command)"
                        >
                            <svg
                                v-if="action.icon === 'ul'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M4 10.5c-.83 0-1.5.67-1.5 1.5s.67 1.5 1.5 1.5 1.5-.67 1.5-1.5-.67-1.5-1.5-1.5zm0-6c-.83 0-1.5.67-1.5 1.5S3.17 7.5 4 7.5 5.5 6.83 5.5 6 4.83 4.5 4 4.5zm0 12c-.83 0-1.5.68-1.5 1.5s.68 1.5 1.5 1.5 1.5-.68 1.5-1.5-.67-1.5-1.5-1.5zM7 19h14v-2H7v2zm0-6h14v-2H7v2zm0-8v2h14V5H7z"
                                />
                            </svg>
                            <svg
                                v-else
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M2 17h2v.5H3v1h1v.5H2v1h3v-4H2v1zm1-9h1V4H2v1h1v3zm-1 3h1.8L2 13.1v.9h3v-1H3.2L5 10.9V10H2v1zm5-6v2h14V5H7zm0 14h14v-2H7v2zm0-6h14v-2H7v2z"
                                />
                            </svg>
                        </button>

                        <div class="toolbar-divider"></div>

                        <button
                            class="toolbar-btn"
                            title="插入链接"
                            @click="handleInsertLink"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M3.9 12c0-1.71 1.39-3.1 3.1-3.1h4V7H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h4v-1.9H7c-1.71 0-3.1-1.39-3.1-3.1zM8 13h8v-2H8v2zm9-6h-4v1.9h4c1.71 0 3.1 1.39 3.1 3.1s-1.39 3.1-3.1 3.1h-4V17h4c2.76 0 5-2.24 5-5s-2.24-5-5-5z"
                                />
                            </svg>
                        </button>

                        <button
                            class="toolbar-btn"
                            title="插入图片"
                            @click="handleInsertImage"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M21 19V5c0-1.1-.9-2-2-2H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2zM8.5 13.5l2.5 3.01L14.5 12l4.5 6H5l3.5-4.5z"
                                />
                            </svg>
                        </button>

                        <button class="toolbar-btn" title="添加附件">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M16.5 6v11.5c0 2.21-1.79 4-4 4s-4-1.79-4-4V5c0-1.38 1.12-2.5 2.5-2.5s2.5 1.12 2.5 2.5v10.5c0 .55-.45 1-1 1s-1-.45-1-1V6H10v9.5c0 1.38 1.12 2.5 2.5 2.5s2.5-1.12 2.5-2.5V5c0-2.21-1.79-4-4-4S7 2.79 7 5v12.5c0 3.04 2.46 5.5 5.5 5.5s5.5-2.46 5.5-5.5V6h-1.5z"
                                />
                            </svg>
                        </button>
                    </div>

                    <!-- 编辑区域 -->
                    <div
                        class="modal-body"
                        style="padding: 0; flex: 1; overflow: hidden"
                    >
                        <div
                            ref="editorRef"
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
                            <svg
                                v-if="isSending"
                                class="animate-spin"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z"
                                />
                            </svg>
                            <svg
                                v-else
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"
                                />
                            </svg>
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

.animate-spin {
    animation: spin 1s linear infinite;
}

@keyframes spin {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}
</style>
