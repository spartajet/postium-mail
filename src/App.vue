<script setup lang="ts">
import { onMounted, computed } from "vue";
import { useEmailStore, useAccountStore, useUIStore, useSyncStore } from "@/stores";
import { useI18n } from "vue-i18n";
import { CheckCircleOutlined, LightModeOutlined, DarkModeOutlined } from "@vicons/material";
import { NConfigProvider, NGlobalStyle, darkTheme, lightTheme, NProgress, NSpin } from "naive-ui";
import { getNaiveUILocale, getNaiveUIDateLocale } from "@/locales/naive-ui-locales";

// 组件导入
import WindowControls from "@/components/layout/WindowControls.vue";
import AppSidebar from "@/components/layout/AppSidebar.vue";
import EmailList from "@/components/layout/EmailList.vue";
import EmailDetail from "@/components/layout/EmailDetail.vue";
import ComposeModal from "@/components/compose/ComposeModal.vue";
import AddAccountModal from "@/components/common/AddAccountModal.vue";
import ToastContainer from "@/components/common/ToastContainer.vue";
import LanguageSelector from "@/components/common/LanguageSelector.vue";
import CalendarView from "@/components/calendar/CalendarView.vue";
import WorkflowView from "@/components/workflow/WorkflowView.vue";
import AIChatModal from "@/components/aiChat/AIChatModal.vue";
import SettingsModal from "@/components/settings/SettingsModal.vue";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();
const syncStore = useSyncStore();

// i18n
const { t } = useI18n();

// Naive UI locale 配置
const naiveLocale = computed(() => getNaiveUILocale(uiStore.currentLocale));
const naiveDateLocale = computed(() => getNaiveUIDateLocale(uiStore.currentLocale));

// 是否是邮件视图
const isEmailView = computed(() => uiStore.currentView === "email");

// 是否是日历视图
const isCalendarView = computed(() => uiStore.currentView === "calendar");

// 是否是工作流视图
const isWorkflowView = computed(() => uiStore.currentView === "workflow");

// 同步进度
const syncProgress = computed(() => {
    const statuses = syncStore.allStatuses;
    const hasAnySyncing = syncStore.hasAnySyncing;
    console.log('[App] syncProgress computed:', { statuses, hasAnySyncing });
    if (statuses.length === 0) return 0;
    // 取所有同步账号的平均进度
    return Math.floor(statuses.reduce((sum, s) => sum + s.progress, 0) / statuses.length);
});

// 同步消息
const syncMessage = computed(() => {
    const statuses = syncStore.allStatuses;
    console.log('[App] syncMessage computed:', { statuses });
    if (statuses.length === 0) return '';
    const status = statuses[0];
    return status.message || '同步中...';
});

// 处理账号添加成功事件
async function handleAccountAdded() {
  // 刷新邮件列表
  if (isEmailView.value) {
    await emailStore.fetchEmails();
  }
}

// 初始化应用
onMounted(async () => {
    console.log('[App] onMounted 开始')

    // 检查是否是 OAuth 回调
    const params = new URLSearchParams(window.location.search)
    const code = params.get('code')
    const state = params.get('state')
    const error = params.get('error')

    // 如果是 OAuth 回调，向父窗口发送消息并关闭
    if (code || error) {
        if (window.opener) {
            window.opener.postMessage({
                code,
                state,
                error: error || undefined
            }, window.location.origin)
        }
        // 等待消息发送后关闭窗口
        setTimeout(() => {
            window.close()
        }, 100)
        return
    }

    // 初始化 UI Store（主题、断点等）
    uiStore.init();

    // 加载账号数据
    console.log('[App] 开始加载账号')
    await accountStore.fetchAccounts();
    console.log('[App] 账号加载完成', {
        hasAccount: !!accountStore.currentAccount,
        account: accountStore.currentAccount,
        isEmailView: isEmailView.value
    })

    // 确保有账号后才加载邮件
    if (accountStore.currentAccount && isEmailView.value) {
        console.log('[App] 开始加载邮件')
        try {
            await emailStore.fetchEmails();
            console.log('[App] 邮件加载完成')
        } catch (e) {
            console.error('[App] 邮件加载失败:', e)
        }
    } else {
        console.log('[App] 跳过邮件加载', {
            hasAccount: !!accountStore.currentAccount,
            isEmailView: isEmailView.value
        })
    }
    // 日历数据会在 CalendarView 组件中加载
    // 工作流数据会在 WorkflowView 组件中加载
});
</script>

<template>
    <NConfigProvider
        :theme="uiStore.isDarkTheme ? darkTheme : lightTheme"
        :locale="naiveLocale"
        :date-locale="naiveDateLocale"
    >
        <div class="app" :data-theme="uiStore.appliedTheme">
            <!-- 背景装饰 -->
            <div class="bg-orbs">
                <div class="orb orb-1"></div>
                <div class="orb orb-2"></div>
                <div class="orb orb-3"></div>
            </div>

            <!-- 悬浮窗口控制按钮 -->
            <WindowControls />

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
                        <CheckCircleOutlined :size="14" />
                        <span>{{ t('statusBar.connected') }}</span>
                    </div>
                    <div class="status-item">
                        <span>{{ t('statusBar.unread', { count: emailStore.unreadCount }) }}</span>
                    </div>
                    <!-- 同步进度显示 -->
                    <div v-if="syncStore.hasAnySyncing" class="status-item sync-progress">
                        <NSpin :size="14" />
                        <NProgress
                            type="line"
                            :percentage="syncProgress"
                            :show-indicator="false"
                            :style="{ width: '100px', marginLeft: '8px' }"
                        />
                        <span class="sync-message">{{ syncMessage }}</span>
                    </div>
                </div>
                <div class="status-bar-right">
                    <div class="status-item">
                        <span>{{
                            new Date().toLocaleDateString(uiStore.currentLocale, {
                                month: "long",
                                day: "numeric",
                            })
                        }}</span>
                    </div>
                    <div class="status-item">
                        <span>{{
                            new Date().toLocaleTimeString(uiStore.currentLocale, {
                                hour: "2-digit",
                                minute: "2-digit",
                            })
                        }}</span>
                    </div>
                    <LanguageSelector />
                    <button
                        class="icon-btn"
                        @click="uiStore.toggleTheme"
                        :title="t('settings.theme')"
                    >
                        <LightModeOutlined v-if="uiStore.isDarkTheme" :size="16" />
                        <DarkModeOutlined v-else :size="16" />
                    </button>
                </div>
            </footer>

            <!-- 模态框 -->
            <ComposeModal />
            <AddAccountModal v-if="uiStore.modals.addAccount" @success="handleAccountAdded" />
            <AIChatModal v-if="uiStore.modals.aiChat" />
            <SettingsModal v-if="uiStore.modals.settings" />

            <!-- Toast 通知 -->
            <ToastContainer />
        </div>
        <NGlobalStyle />
    </NConfigProvider>
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

/* 同步进度显示 */
.status-item.sync-progress {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 200px;
}

.sync-message {
    font-size: 12px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
</style>
