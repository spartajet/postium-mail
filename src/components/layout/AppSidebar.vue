<script setup lang="ts">
import { computed, ref, h } from "vue";
import { useEmailStore, useAccountStore, useUIStore } from "@/stores";
import type { EmailFolder } from "@/types";
import {
    EmailOutlined,
    SettingsOutlined,
    LightModeOutlined,
    DarkModeOutlined,
    ExpandMoreOutlined,
    AddOutlined,
    EditOutlined,
    InboxOutlined,
    StarOutlined,
    SendOutlined,
    DraftsOutlined,
    ReportOutlined,
    DeleteOutlined,
    CalendarTodayOutlined,
    AccountTreeOutlined,
} from "@vicons/material";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();

// 图标组件映射
const iconMap: Record<string, any> = {
    inbox: InboxOutlined,
    star: StarOutlined,
    send: SendOutlined,
    file: DraftsOutlined,
    alert: ReportOutlined,
    trash: DeleteOutlined,
    calendar: CalendarTodayOutlined,
    workflow: AccountTreeOutlined,
};

// 渲染图标函数
const renderIcon = (iconName: string) => {
    const IconComponent = iconMap[iconName];
    return IconComponent ? h(IconComponent, { size: 20 }) : null;
};

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
                    <EmailOutlined :size="24" />
                </div>
                <span class="logo-text">Postium</span>
            </div>
            <div class="header-actions">
                <button class="icon-btn" @click="openSettings" title="设置">
                    <SettingsOutlined :size="18" />
                </button>
                <button class="icon-btn" @click="toggleTheme" title="切换主题">
                    <LightModeOutlined v-if="uiStore.isDarkTheme" :size="18" />
                    <DarkModeOutlined v-else :size="18" />
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
                    <ExpandMoreOutlined class="select-arrow" :size="20" />
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
                            <AddOutlined :size="20" />
                        </div>
                        <span>添加账号</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- Compose Button -->
        <button class="compose-btn" @click="openCompose">
            <EditOutlined :size="20" />
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
                        :class="[
                            'nav-item',
                            {
                                active:
                                    activeNav === item.id &&
                                    activeView === 'email',
                            },
                        ]"
                        @click="handleNavClick(item.id as EmailFolder)"
                    >
                        <component :is="renderIcon(item.icon)" />
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
                        :class="[
                            'nav-item',
                            { active: activeView === item.id },
                        ]"
                        @click="
                            handleViewNavClick(
                                item.id as 'calendar' | 'workflow',
                            )
                        "
                    >
                        <component :is="renderIcon(item.icon)" />
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
