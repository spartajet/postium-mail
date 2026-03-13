<script setup lang="ts">
import { onMounted } from "vue";
import { useEmailStore, useAccountStore, useUIStore } from "@/stores";

// 组件导入
import AppSidebar from "@/components/layout/AppSidebar.vue";
import EmailList from "@/components/layout/EmailList.vue";
import EmailDetail from "@/components/layout/EmailDetail.vue";
import ComposeModal from "@/components/compose/ComposeModal.vue";
import ToastContainer from "@/components/common/ToastContainer.vue";

// Stores
const emailStore = useEmailStore();
const accountStore = useAccountStore();
const uiStore = useUIStore();

// 初始化应用
onMounted(async () => {
    // 初始化 UI Store（主题、断点等）
    uiStore.init();

    // 加载账号数据
    await accountStore.fetchAccounts();

    // 加载邮件数据
    await emailStore.fetchEmails(accountStore.currentAccount?.id);
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

        <!-- 主布局 -->
        <div class="main-content">
            <AppSidebar />
            <EmailList />
            <EmailDetail />
        </div>

        <!-- 模态框 -->
        <ComposeModal />

        <!-- Toast 通知 -->
        <ToastContainer />

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
        </footer>
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

.main-content {
    display: flex;
    flex: 1;
    overflow: hidden;
}
</style>
