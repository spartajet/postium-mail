<script setup lang="ts">
import { computed, ref } from "vue";
import { useEmailStore, useAccountStore, useUIStore } from "@/stores";
import type { EmailFolder } from "@/types";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();

// 导航项配置
const navItems = computed(() => [
    {
        id: "inbox",
        label: "收件箱",
        icon: "inbox",
        count: emailStore.folderCounts.inbox,
    },
    {
        id: "starred",
        label: "星标邮件",
        icon: "star",
        count: emailStore.folderCounts.starred,
    },
    {
        id: "sent",
        label: "已发送",
        icon: "send",
        count: emailStore.folderCounts.sent,
    },
    {
        id: "drafts",
        label: "草稿",
        icon: "file",
        count: emailStore.folderCounts.drafts,
    },
    {
        id: "spam",
        label: "垃圾邮件",
        icon: "alert",
        count: emailStore.folderCounts.spam,
    },
    {
        id: "trash",
        label: "已删除",
        icon: "trash",
        count: emailStore.folderCounts.trash,
    },
]);

// 视图导航项
const viewNavItems = [
    {
        id: "calendar",
        label: "日历",
        icon: "calendar",
    },
    {
        id: "workflow",
        label: "工作流",
        icon: "workflow",
    },
];

// 标签配置
const labels = [
    { id: "urgent", name: "紧急", color: "#EF4444" },
    { id: "work", name: "工作", color: "#3B82F6" },
    { id: "personal", name: "个人", color: "#10B981" },
    { id: "finance", name: "财务", color: "#F59E0B" },
];

// 选中的导航项
const activeNav = computed(() => emailStore.currentFolder);

// 选中的视图
const activeView = computed(() => uiStore.currentView);

// 切换邮件文件夹导航
function handleNavClick(folder: EmailFolder) {
    uiStore.setView("email");
    emailStore.setFolder(folder);
}

// 切换视图导航
function handleViewNavClick(view: "calendar" | "workflow") {
    uiStore.setView(view);
}

// 切换主题
function toggleTheme() {
    uiStore.toggleTheme();
}

// 打开写信模态框
function openCompose() {
    uiStore.openComposeModal();
}

// 打开设置模态框
function openSettings() {
    uiStore.openSettingsModal();
}

// 格式化存储空间
const storageUsed = ref(4.5);
const storageTotal = ref(10);
const storagePercent = computed(
    () => (storageUsed.value / storageTotal.value) * 100,
);
</script>

<template>
    <aside class="sidebar">
        <!-- Header -->
        <div class="sidebar-header" data-tauri-drag-region>
            <div class="logo">
                <div class="logo-icon">
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
                <span class="logo-text">Postium</span>
            </div>
            <div class="header-actions">
                <button class="icon-btn" @click="openSettings" title="设置">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M19.14,12.94c0.04-0.31,0.06-0.63,0.06-0.94c0-0.31-0.02-0.63-0.06-0.94l1.96-0.82c0.27-0.11,0.44-0.4,0.37-0.69l-0.8-3.36 c-0.06-0.28-0.33-0.46-0.61-0.4l-2.12,0.51c-0.5-0.38-1.03-0.7-1.62-0.94l-0.35-2.15c-0.05-0.28-0.3-0.48-0.58-0.48h-3.5 c-0.28,0-0.53,0.2-0.58,0.48L10.89,4.3c-0.59,0.24-1.13,0.56-1.62,0.94L7.15,4.73c-0.28-0.06-0.55,0.12-0.61,0.4L5.74,8.49 c-0.06,0.28,0.11,0.58,0.37,0.69l1.96,0.82c-0.04,0.31-0.06,0.63-0.06,0.94s0.02,0.63,0.06,0.94l-1.96,0.82 c-0.27,0.11-0.44,0.4-0.37,0.69l0.8,3.36c0.06,0.28,0.33,0.46,0.61,0.4l2.12-0.51c0.5,0.38,1.03,0.7,1.62,0.94l0.35,2.15 c0.05,0.28,0.3,0.48,0.58,0.48h3.5c0.28,0,0.53-0.2,0.58-0.48l0.35-2.15c0.59-0.24,1.13-0.56,1.62-0.94l2.12,0.51 c0.28,0.06,0.55-0.12,0.61-0.4l0.8-3.36c0.06-0.28-0.11-0.58-0.37-0.69L19.14,12.94z M12,15.6c-1.98,0-3.6-1.62-3.6-3.6 s1.62-3.6,3.6-3.6s3.6,1.62,3.6,3.6S13.98,15.6,12,15.6z"
                        />
                    </svg>
                </button>
                <button class="icon-btn" @click="toggleTheme" title="切换主题">
                <svg
                    v-if="uiStore.isDarkTheme"
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5zM2 13h2c.55 0 1-.45 1-1s-.45-1-1-1H2c-.55 0-1 .45-1 1s.45 1 1 1zm18 0h2c.55 0 1-.45 1-1s-.45-1-1-1h-2c-.55 0-1 .45-1 1s.45 1 1 1zM11 2v2c0 .55.45 1 1 1s1-.45 1-1V2c0-.55-.45-1-1-1s-1 .45-1 1zm0 18v2c0 .55.45 1 1 1s1-.45 1-1v-2c0-.55-.45-1-1-1s-1 .45-1 1zM5.99 4.58c-.39-.39-1.03-.39-1.41 0-.39.39-.39 1.03 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0s.39-1.03 0-1.41L5.99 4.58zm12.37 12.37c-.39-.39-1.03-.39-1.41 0-.39.39-.39 1.03 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0 .39-.39.39-1.03 0-1.41l-1.06-1.06zm1.06-10.96c.39-.39.39-1.03 0-1.41-.39-.39-1.03-.39-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06zM7.05 18.36c.39-.39.39-1.03 0-1.41-.39-.39-1.03-.39-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06z"
                    />
                </svg>
                <svg
                    v-else
                    xmlns="http://www.w3.org/2000/svg"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                >
                    <path
                        d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9 9-4.03 9-9c0-.46-.04-.92-.1-1.36-.98 1.37-2.58 2.26-4.4 2.26-2.98 0-5.4-2.42-5.4-5.4 0-1.81.89-3.42 2.26-4.4-.44-.06-.9-.1-1.36-.1z"
                    />
                </svg>
            </button>
            </div>
        </div>

        <!-- Account Selector -->
        <div class="account-selector-wrapper">
            <div
                class="custom-select"
                :class="{ open: accountStore.isDropdownOpen }"
            >
                <div
                    class="custom-select-trigger"
                    @click="accountStore.toggleDropdown"
                >
                    <div
                        class="selected-account"
                        v-if="accountStore.currentAccount"
                    >
                        <div
                            class="selected-account-avatar"
                            :style="{
                                backgroundColor:
                                    accountStore.currentAccount.color,
                            }"
                        >
                            {{ accountStore.currentAccount.name.charAt(0) }}
                        </div>
                        <div class="selected-account-info">
                            <div class="selected-account-name">
                                {{ accountStore.currentAccount.name }}
                            </div>
                            <div class="selected-account-email">
                                {{ accountStore.currentAccount.email }}
                            </div>
                        </div>
                    </div>
                    <svg
                        class="select-arrow"
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path d="M7 10l5 5 5-5z" />
                    </svg>
                </div>

                <div
                    class="custom-select-options"
                    v-show="accountStore.isDropdownOpen"
                >
                    <div
                        v-for="account in accountStore.accounts"
                        :key="account.id"
                        :class="[
                            'account-option',
                            {
                                active:
                                    accountStore.currentAccount?.id ===
                                    account.id,
                            },
                        ]"
                        @click="accountStore.selectAccount(account)"
                    >
                        <div
                            class="account-option-avatar"
                            :style="{ backgroundColor: account.color }"
                        >
                            {{ account.name.charAt(0) }}
                        </div>
                        <div class="account-option-info">
                            <div class="account-option-name">
                                {{ account.name }}
                            </div>
                            <div class="account-option-email">
                                {{ account.email }}
                            </div>
                        </div>
                        <span
                            v-if="account.unreadCount > 0"
                            class="nav-badge"
                            >{{ account.unreadCount }}</span
                        >
                    </div>

                    <div
                        class="account-option add-option"
                        @click="uiStore.openAddAccountModal"
                    >
                        <div class="add-option-icon">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" />
                            </svg>
                        </div>
                        <span>添加账号</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- Compose Button -->
        <button class="compose-btn" @click="openCompose">
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
            >
                <path
                    d="M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34c-.39-.39-1.02-.39-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z"
                />
            </svg>
            <span>写信</span>
        </button>

        <!-- Navigation -->
        <nav class="nav">
            <!-- Folders -->
            <div class="nav-section">
                <div class="nav-items">
                    <a
                        v-for="item in navItems"
                        :key="item.id"
                        :class="['nav-item', { active: activeNav === item.id && activeView === 'email' }]"
                        @click="handleNavClick(item.id as EmailFolder)"
                    >
                        <!-- Inbox Icon -->
                        <svg
                            v-if="item.icon === 'inbox'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
                            />
                        </svg>
                        <!-- Star Icon -->
                        <svg
                            v-else-if="item.icon === 'star'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"
                            />
                        </svg>
                        <!-- Send Icon -->
                        <svg
                            v-else-if="item.icon === 'send'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path d="M2.01 21L23 12 2.01 3 2 10l15 2 15-2v2z" />
                        </svg>
                        <!-- File Icon -->
                        <svg
                            v-else-if="item.icon === 'file'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm4 18H6V4h7v5h5v11z"
                            />
                        </svg>
                        <!-- Alert Icon -->
                        <svg
                            v-else-if="item.icon === 'alert'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2v-2h2v2z"
                            />
                        </svg>
                        <!-- Trash Icon -->
                        <svg
                            v-else-if="item.icon === 'trash'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                            />
                        </svg>
                        <!-- Default Icon -->
                        <svg
                            v-else
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
                            />
                        </svg>
                        <span>{{ item.label }}</span>
                        <span
                            v-if="item.count && item.count > 0"
                            class="nav-badge"
                            >{{ item.count }}</span
                        >
                    </a>
                </div>
            </div>

            <!-- Views -->
            <div class="nav-section">
                <div class="nav-section-header">视图</div>
                <div class="nav-items">
                    <a
                        v-for="item in viewNavItems"
                        :key="item.id"
                        :class="['nav-item', { active: activeView === item.id }]"
                        @click="handleViewNavClick(item.id as 'calendar' | 'workflow')"
                    >
                        <!-- Calendar Icon -->
                        <svg
                            v-if="item.icon === 'calendar'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 16H5V9h14v11zM9 11H7v2h2v-2zm4 0h-2v2h2v-2zm4 0h-2v2h2v-2zm-8 4H7v2h2v-2zm4 0h-2v2h2v-2zm4 0h-2v2h2v-2z"
                            />
                        </svg>
                        <!-- Workflow Icon -->
                        <svg
                            v-else-if="item.icon === 'workflow'"
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M22 11V3h-7v3H9V3H2v8h7v-3h2v3h2v3h2v-3h2v-3h2v7h3v-7h-2zM7 9H4V5h3v4zm10 0h-3V5h3v4zm-6 4h-2v-2h2v2zm4 0h-2v-2h2v2z"
                            />
                        </svg>
                        <span>{{ item.label }}</span>
                    </a>
                </div>
            </div>

            <!-- Labels -->
            <div class="nav-section">
                <div class="nav-section-header">标签</div>
                <div class="nav-items">
                    <a
                        v-for="label in labels"
                        :key="label.id"
                        class="nav-item label-item"
                    >
                        <span
                            class="label-dot"
                            :style="{ backgroundColor: label.color }"
                        ></span>
                        <span>{{ label.name }}</span>
                    </a>
                </div>
            </div>
        </nav>

        <!-- Footer - Storage -->
        <div class="sidebar-footer">
            <div class="storage">
                <div class="storage-bar">
                    <div
                        class="storage-fill"
                        :style="{ width: `${storagePercent}%` }"
                    ></div>
                </div>
                <div class="storage-text">
                    已用 {{ storageUsed }} GB / {{ storageTotal }} GB
                </div>
            </div>
        </div>
    </aside>
</template>

<style scoped>
/* 组件使用全局样式，此处仅添加作用域样式如有需要 */
</style>
