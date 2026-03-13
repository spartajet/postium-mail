<script setup lang="ts">
import { onMounted, computed } from "vue";
import { useEmailStore, useAccountStore, useUIStore } from "@/stores";

// 组件导入
import AppSidebar from "@/components/layout/AppSidebar.vue";
import EmailList from "@/components/layout/EmailList.vue";
import EmailDetail from "@/components/layout/EmailDetail.vue";
import ComposeModal from "@/components/compose/ComposeModal.vue";
import ToastContainer from "@/components/common/ToastContainer.vue";
import CalendarView from "@/components/calendar/CalendarView.vue";
import WorkflowView from "@/components/workflow/WorkflowView.vue";
import AIChatModal from "@/components/aiChat/AIChatModal.vue";
import SettingsModal from "@/components/settings/SettingsModal.vue";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();

// 是否是邮件视图
const isEmailView = computed(() => uiStore.currentView === "email");

// 是否是日历视图
const isCalendarView = computed(() => uiStore.currentView === "calendar");

// 是否是工作流视图
const isWorkflowView = computed(() => uiStore.currentView === "workflow");

// 初始化应用
onMounted(async () => {
    // 初始化 UI Store（主题、断点等）
    uiStore.init();

    // 加载账号数据
    await accountStore.fetchAccounts();

    // 根据当前视图加载数据
    if (isEmailView.value) {
        await emailStore.fetchEmails(accountStore.currentAccount?.id);
    }
    // 日历数据会在 CalendarView 组件中加载
    // 工作流数据会在 WorkflowView 组件中加载
});
</script>

<template>
    <div class="app" :data-theme="uiStore.appliedTheme">
        <!-- 背景装饰 -->
        <div class="bg-orbs">
            <div class="orb orb-1"></div>
            <div class="orb orb-2"></div>
            <div class="orb orb-3"></div>
        </div>

        <!-- 主布局容器 -->
        <div class="main-layout">
            <AppSidebar />

            <!-- 内容区域 -->
            <div class="content-area">
                <!-- 邮件视图 -->
                <div v-if="isEmailView" class="email-view">
                    <EmailList />
                    <EmailDetail />
                </div>

                <!-- 日历视图 -->
                <div v-else-if="isCalendarView" class="calendar-view">
                    <CalendarView />
                </div>

                <!-- 工作流视图 -->
                <div v-else-if="isWorkflowView" class="workflow-view">
                    <WorkflowView />
                </div>
            </div>
        </div>

        <!-- 状态栏 -->
        <footer class="status-bar">
            <div class="status-bar-left">
                <div class="status-item">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                        />
                    </svg>
                    <span>已连接</span>
                </div>
                <div class="status-item">
                    <span>{{ emailStore.unreadCount }} 封未读</span>
                </div>
            </div>
            <div class="status-bar-right">
                <div class="status-item">
                    <span>{{
                        new Date().toLocaleDateString("zh-CN", {
                            month: "long",
                            day: "numeric",
                        })
                    }}</span>
                </div>
                <div class="status-item">
                    <span>{{
                        new Date().toLocaleTimeString("zh-CN", {
                            hour: "2-digit",
                            minute: "2-digit",
                        })
                    }}</span>
                </div>
                <button
                    class="icon-btn"
                    @click="uiStore.toggleTheme"
                    title="切换主题"
                >
                    <svg
                        v-if="uiStore.isDarkTheme"
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M12 7c-2.76 0-5 2.24-5 5s2.24 5 5 5 5-2.24 5-5-2.24-5-5-5-2.24-5-5-5-2.76 0-5 2.24-5 5s2.24 5 5 5-2.24 5-5-5-2.24-5-5zM2 13h2c.55 0 1-.45 1-1s-.45-1-1-1H2c-.55 0-1 .45-1 1s.45 1 1 1zm18 0h2c.55 0 1-.45 1-1s-.45-1-1-1h-2c-.55 0-1 .45-1 1s.45 1 1 1zM11 2v2c0 .55.45 1 1 1s1-.45 1-1V2c0-.55-.45-1-1-1s-1 .45-1 1zm0 18v2c0 .55.45 1 1 1s1-.45 1-1v-2c0-.55-.45-1-1-1s-1 .45-1 1zM5.99 4.58c-.39-.39-1.03-.39-1.41 0-.39.39-.39 1.03 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0s.39-1.03 0-1.41L5.99 4.58zm12.37 12.37c-.39-.39-1.03-.39-1.41 0-.39.39-.39 1.03 0 1.41l1.06 1.06c.39.39 1.03.39 1.41 0 .39-.39.39-1.03 0-1.41l-1.06-1.06zm1.06-10.96c.39-.39.39-1.03 0-1.41-.39-.39-1.03-.39-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06zM7.05 18.36c.39-.39.39-1.03 0-1.41-.39-.39-1.03-.39-1.41 0l-1.06 1.06c-.39.39-.39 1.03 0 1.41s1.03.39 1.41 0l1.06-1.06z"
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
        </footer>

        <!-- 模态框 -->
        <ComposeModal />
        <AIChatModal v-if="uiStore.modals.aiChat" />
        <SettingsModal v-if="uiStore.modals.settings" />

        <!-- Toast 通知 -->
        <ToastContainer />
    </div>
</template>

<style scoped>
.app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    position: relative;
}

.main-layout {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
}

.content-area {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
}

.bg-orbs {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: -1;
    background: var(--bg-base);
}

.orb {
    position: fixed;
    border-radius: 50%;
    filter: blur(60px);
    opacity: 0.5;
    pointer-events: none;
}

.orb-1 {
    width: 400px;
    height: 400px;
    top: -100px;
    left: -100px;
    background: radial-gradient(
        circle at center,
        var(--primary) 0%,
        transparent 70%
    );
}

.orb-2 {
    width: 300px;
    height: 300px;
    bottom: -50px;
    right: -50px;
    background: radial-gradient(
        circle at center,
        var(--secondary) 0%,
        transparent 70%
    );
}

.orb-3 {
    width: 200px;
    height: 200px;
    bottom: 100px;
    left: 100px;
    background: radial-gradient(
        circle at center,
        var(--accent) 0%,
        transparent 70%
    );
}

.main-content {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
}

.email-view {
    display: flex;
    flex: 1;
    width: 100%;
    height: 100%;
}

.calendar-view {
    display: flex;
    flex: 1;
    overflow: hidden;
    width: 100%;
    height: 100%;
}

.workflow-view {
    display: flex;
    flex: 1;
    overflow: hidden;
    width: 100%;
    height: 100%;
}

.status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    height: 32px;
    padding: 0 24px;
    background: var(--bg-elevated);
    border-top: 1px solid var(--border-subtle);
    font-size: 12px;
    color: var(--text-muted);
    z-index: 10;
}

.status-bar-left,
.status-bar-right {
    display: flex;
    align-items: center;
    gap: 24px;
}

.status-item {
    display: flex;
    align-items: center;
    gap: 6px;
}

.status-item svg {
    width: 14px;
    height: 14px;
}

.icon-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
    background: transparent;
}

.icon-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.icon-btn svg {
    width: 16px;
    height: 16px;
}
</style>
