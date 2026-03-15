<script setup lang="ts">
import { computed, ref, h } from "vue";
import {
    useEmailStore,
    useAccountStore,
    useUIStore,
    useSyncStore,
} from "@/stores";
import { useI18n } from "vue-i18n";
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
    SyncOutlined,
} from "@vicons/material";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();
const syncStore = useSyncStore();

// i18n
const { t } = useI18n();

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
        label: t("email.inbox"),
        icon: "inbox",
        count: emailStore.folderCounts.inbox,
    },
    {
        id: "starred",
        label: t("email.starred"),
        icon: "star",
        count: emailStore.folderCounts.starred,
    },
    {
        id: "sent",
        label: t("email.sent"),
        icon: "send",
        count: emailStore.folderCounts.sent,
    },
    {
        id: "drafts",
        label: t("email.drafts"),
        icon: "file",
        count: emailStore.folderCounts.drafts,
    },
    {
        id: "spam",
        label: t("email.spam"),
        icon: "alert",
        count: emailStore.folderCounts.spam,
    },
    {
        id: "trash",
        label: t("email.trash"),
        icon: "trash",
        count: emailStore.folderCounts.trash,
    },
]);

// 视图导航项
const viewNavItems = computed(() => [
    {
        id: "calendar",
        label: t("nav.calendar"),
        icon: "calendar",
    },
    {
        id: "workflow",
        label: t("nav.workflow"),
        icon: "workflow",
    },
]);

// 标签配置
const labels = computed(() => [
    { id: "urgent", name: t("sidebar.labels.urgent"), color: "#EF4444" },
    { id: "work", name: t("sidebar.labels.work"), color: "#3B82F6" },
    { id: "personal", name: t("sidebar.labels.personal"), color: "#10B981" },
    { id: "finance", name: t("sidebar.labels.finance"), color: "#F59E0B" },
]);

// 选中的导航项
const activeNav = computed(() => emailStore.currentFolder);

// 选中的视图
const activeView = computed(() => uiStore.currentView);

// 切换邮件文件夹导航
async function handleNavClick(folder: EmailFolder) {
    uiStore.setView("email");
    await emailStore.setFolder(folder);
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

// 同步账号邮件
async function syncAccountEmail() {
    if (!accountStore.currentAccount) return;

    try {
        await accountStore.syncAccount();
        await emailStore.fetchEmails();
        uiStore.showSuccess(t("email.syncSuccess"));
    } catch (error) {
        uiStore.showError(t("email.syncFailed"));
    }
}

// 手动同步所有账号
async function syncAllAccounts() {
    if (!accountStore.currentAccount) {
        console.log("[AppSidebar] 没有当前账号，跳过同步");
        return;
    }

    console.log("[AppSidebar] 开始同步账号:", accountStore.currentAccount.id);

    try {
        await syncStore.syncAccount(accountStore.currentAccount.id);
        await emailStore.fetchEmails();
        uiStore.showSuccess("同步完成");
    } catch (error) {
        console.error("[AppSidebar] 同步失败:", error);
        uiStore.showError("同步失败");
    }
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
                <button
                    class="icon-btn"
                    @click="openSettings"
                    :title="t('settings.title')"
                >
                    <SettingsOutlined :size="18" />
                </button>
                <button
                    class="icon-btn"
                    @click="syncAllAccounts"
                    :disabled="syncStore.hasAnySyncing"
                    :title="syncStore.hasAnySyncing ? '同步中...' : '手动同步'"
                >
                    <SyncOutlined
                        :size="18"
                        :class="{ spinning: syncStore.hasAnySyncing }"
                    />
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
                        <span>{{ t("sidebar.addAccount") }}</span>
                    </div>
                </div>
            </div>
        </div>

        <!-- Compose Button -->
        <button class="compose-btn" @click="openCompose">
            <EditOutlined :size="20" />
            <span>{{ t("email.compose") }}</span>
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
                <div class="nav-section-header">{{ t("nav.views") }}</div>
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
                <div class="nav-section-header">{{ t("nav.labels") }}</div>
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
                    {{ t("sidebar.storageUsed") }} {{ storageUsed }}
                    {{ t("sidebar.storageTotal") }} / {{ storageTotal }}
                    {{ t("sidebar.storageTotal") }}
                </div>
            </div>
        </div>
    </aside>
</template>

<style scoped>
/* 组件使用全局样式，此处仅添加作用域样式如有需要 */

/* 头部同步按钮旋转动画 */
.icon-btn.spinning :deep(svg) {
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

@keyframes spin {
    from {
        transform: rotate(0deg);
    }
    to {
        transform: rotate(360deg);
    }
}

/* 重写 nav-badge 为圆角正方形 */
.nav-item :deep(.nav-badge) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    width: 20px;
    height: 20px;
    min-width: 20px;
    max-width: 20px;
    border-radius: 4px;
    line-height: 1;
    box-sizing: border-box;
    overflow: hidden;
}

/* 确保 label-dot 是圆形 */
.label-item :deep(.label-dot) {
    width: 8px !important;
    height: 8px !important;
    min-width: 8px !important;
    max-width: 8px !important;
    border-radius: 50% !important;
    display: inline-block !important;
    flex-shrink: 0 !important;
}
</style>
