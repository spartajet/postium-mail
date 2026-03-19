<template>
  <!-- 简单模式：单账号状态指示器 -->
  <div v-if="!multiAccount" class="sync-status-indicator" :class="{ syncing, error: hasError }">
    <!-- 同步中 -->
    <div v-if="syncing" class="syncing-indicator">
      <NSpin :size="14" />
      <span class="status-text">{{ syncStatus?.message || '同步中...' }}</span>
      <NTooltip trigger="hover" v-if="syncStatus?.progress !== undefined">
        <template #trigger>
          <span class="progress-text">{{ Math.round(syncStatus.progress * 100) }}%</span>
        </template>
        <div class="progress-detail">
          <p><strong>阶段:</strong> {{ getStageLabel(syncStatus.stage) }}</p>
          <p v-if="syncStatus.current_folder"><strong>当前:</strong> {{ syncStatus.current_folder }}</p>
          <p v-if="syncStatus.progress !== undefined">
            <strong>进度:</strong> {{ Math.round(syncStatus.progress * 100) }}%
          </p>
        </div>
      </NTooltip>
    </div>

    <!-- 已完成 -->
    <div v-else-if="hasSynced" class="completed-indicator">
      <NTooltip trigger="hover">
        <template #trigger>
          <span class="icon-success">✓</span>
        </template>
        <span>已同步于 {{ formatTime(lastSyncTime) }}</span>
      </NTooltip>
    </div>

    <!-- 错误 -->
    <div v-else-if="hasError" class="error-indicator">
      <NTooltip trigger="hover">
        <template #trigger>
          <span class="icon-error">⚠</span>
        </template>
        <span>{{ syncStatus?.error || '同步失败' }}</span>
      </NTooltip>
    </div>

    <!-- 空闲 -->
    <div v-else class="idle-indicator">
      <span class="icon-idle">🕐</span>
    </div>
  </div>

  <!-- 多账号模式：显示所有账号同步状态 -->
  <div v-else class="multi-account-sync-status">
    <!-- 标题栏 -->
    <div class="status-header">
      <div class="header-left">
        <span class="title">同步状态</span>
        <NTag v-if="anySyncing" type="info" size="small" round>同步中</NTag>
        <NTag v-else type="default" size="small" round>空闲</NTag>
      </div>
      <div class="header-right">
        <NButton text size="small" @click="showHistory = true">
          <template #icon>
            <span class="icon-history">📋</span>
          </template>
          历史
        </NButton>
        <NButton text size="small" @click="refreshStatus">
          <template #icon>
            <span class="icon-refresh">🔄</span>
          </template>
        </NButton>
      </div>
    </div>

    <!-- 账号列表 -->
    <div class="accounts-list">
      <div
        v-for="account in sortedAccounts"
        :key="account.id"
        class="account-status-item"
        :class="{ syncing: isAccountSyncing(account.id), error: hasAccountError(account.id) }"
      >
        <!-- 账号信息 -->
        <div class="account-info">
          <div class="account-avatar" :style="{ backgroundColor: account.color }">
            {{ account.email[0].toUpperCase() }}
          </div>
          <div class="account-details">
            <div class="account-name">{{ account.name || account.email }}</div>
            <div class="account-email">{{ account.email }}</div>
          </div>
        </div>

        <!-- 状态指示 -->
        <div class="account-status">
          <!-- 同步中 -->
          <template v-if="isAccountSyncing(account.id)">
            <NSpin :size="16" />
            <div class="status-detail">
              <div class="status-message">{{ getAccountStatus(account.id)?.message || '同步中...' }}</div>
              <NProgress
                v-if="getAccountProgress(account.id) !== undefined"
                type="line"
                :percentage="Math.round((getAccountProgress(account.id) || 0) * 100)"
                :height="4"
                :show-indicator="false"
              />
              <div class="status-meta">
                <span class="stage-badge">{{ getStageLabel(getAccountStatus(account.id)?.stage) }}</span>
                <span v-if="getAccountStatus(account.id)?.current_folder" class="folder-info">
                  {{ getAccountStatus(account.id)?.current_folder }}
                </span>
              </div>
            </div>
          </template>

          <!-- 错误 -->
          <template v-else-if="hasAccountError(account.id)">
            <span class="icon-error">⚠</span>
            <div class="status-detail">
              <div class="error-message">{{ getAccountStatus(account.id)?.error || '同步失败' }}</div>
              <NButton size="tiny" @click="retrySync(account.id)">重试</NButton>
            </div>
          </template>

          <!-- 已完成 -->
          <template v-else-if="hasAccountSynced(account)">
            <span class="icon-success">✓</span>
            <div class="status-detail">
              <div class="success-message">
                已于 {{ formatTime(account.lastSyncAt?.getTime() ? account.lastSyncAt.getTime() / 1000 : 0) }} 同步
              </div>
              <NButton size="tiny" @click="triggerSync(account.id)">立即同步</NButton>
            </div>
          </template>

          <!-- 空闲 -->
          <template v-else>
            <span class="icon-idle">🕐</span>
            <div class="status-detail">
              <div class="idle-message">等待同步</div>
              <NButton size="tiny" @click="triggerSync(account.id)">开始同步</NButton>
            </div>
          </template>
        </div>
      </div>

      <!-- 空状态 -->
      <div v-if="accounts.length === 0" class="empty-state">
        <span class="icon-empty">📭</span>
        <p>没有可同步的账号</p>
      </div>
    </div>

    <!-- 同步历史抽屉 -->
    <NDrawer v-model:show="showHistory" :width="400" placement="right">
      <NDrawerContent title="同步历史" closable>
        <div class="sync-history">
          <div v-if="historyItems.length === 0" class="empty-history">
            <span class="icon-empty">📭</span>
            <p>暂无同步历史</p>
          </div>
          <div v-else class="history-list">
            <div v-for="(item, index) in historyItems" :key="index" class="history-item">
              <div class="history-header">
                <span class="history-email">{{ item.accountEmail }}</span>
                <span class="history-time">{{ formatHistoryTime(item.completedAt) }}</span>
              </div>
              <div class="history-result">
                <NTag size="small" type="success">同步完成</NTag>
                <span class="result-stats">
                  {{ item.result.totalSynced }} 封邮件 ·
                  {{ item.result.foldersSynced }} 个文件夹 ·
                  耗时 {{ formatDuration(item.result.durationMs) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </NDrawerContent>
    </NDrawer>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { NTooltip, NSpin, NTag, NButton, NProgress, NDrawer, NDrawerContent } from 'naive-ui'
import { useSyncStore } from '@/stores/sync'
import { useAccountStore } from '@/stores/account'
import { SyncStage } from '@/types'
import type { Account } from '@/types'

interface Props {
  accountId?: number
  lastSyncTime?: number
  multiAccount?: boolean
}

const props = defineProps<Props>()

const syncStore = useSyncStore()
const accountStore = useAccountStore()

// 多账号模式状态
const showHistory = ref(false)
const refreshInterval = ref<number | null>(null)

// ========================================
// Computed - 单账号模式
// ========================================

const syncing = computed(() => props.accountId !== undefined ? syncStore.isSyncing(props.accountId) : false)
const syncStatus = computed(() => props.accountId !== undefined ? syncStore.getStatus(props.accountId) : null)
const hasSynced = computed(() => props.lastSyncTime && props.lastSyncTime > 0)
const hasError = computed(() => syncStatus.value?.stage === 'error')

// ========================================
// Computed - 多账号模式
// ========================================

const accounts = computed(() => accountStore.accounts)
const sortedAccounts = computed(() => {
  // 正在同步的账号排在前面
  return [...accounts.value].sort((a, b) => {
    const aSyncing = isAccountSyncing(a.id)
    const bSyncing = isAccountSyncing(b.id)
    if (aSyncing && !bSyncing) return -1
    if (!aSyncing && bSyncing) return 1
    return 0
  })
})

const anySyncing = computed(() => sortedAccounts.value.some(a => isAccountSyncing(a.id)))

const historyItems = computed(() => syncStore.recentHistory)

// ========================================
// Methods
// ========================================

// 单账号状态辅助方法
function getStageLabel(stage?: string): string {
  const labels: Record<string, string> = {
    [SyncStage.Connecting]: '连接中',
    [SyncStage.SyncingFolders]: '同步文件夹',
    [SyncStage.SyncingEmails]: '同步邮件',
    [SyncStage.Completed]: '已完成',
    [SyncStage.Error]: '错误',
    'idle': '空闲',
    'syncing': '同步中',
  }
  return labels[stage || ''] || stage || '未知'
}

function formatTime(timestamp?: number): string {
  if (!timestamp) return '-'
  const date = new Date(timestamp * 1000)
  const now = new Date()
  const diff = Math.floor((now.getTime() - date.getTime()) / 1000 / 60)

  if (diff < 1) return '刚刚'
  if (diff < 60) return `${diff} 分钟前`
  if (diff < 1440) return `${Math.floor(diff / 60)} 小时前`
  return `${Math.floor(diff / 1440)} 天前`
}

// 多账号状态辅助方法
function isAccountSyncing(accountId: string): boolean {
  return syncStore.isSyncing(parseInt(accountId))
}

function hasAccountError(accountId: string): boolean {
  const status = syncStore.getStatus(parseInt(accountId))
  return status?.stage === 'error'
}

function hasAccountSynced(account: Account): boolean {
  return !!account.lastSyncAt
}

function getAccountStatus(accountId: string) {
  return syncStore.getStatus(parseInt(accountId))
}

function getAccountProgress(accountId: string): number | undefined {
  return syncStore.getStatus(parseInt(accountId))?.progress
}

function formatHistoryTime(date: Date): string {
  const now = new Date()
  const diff = Math.floor((now.getTime() - date.getTime()) / 1000 / 60)

  if (diff < 1) return '刚刚'
  if (diff < 60) return `${diff}分钟前`
  if (diff < 1440) return `${Math.floor(diff / 60)}小时前`
  return date.toLocaleDateString('zh-CN')
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  if (seconds < 60) return `${seconds}秒`
  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}分钟`
  const hours = Math.floor(minutes / 60)
  return `${hours}小时${minutes % 60}分钟`
}

// 操作方法
async function triggerSync(accountId: string) {
  try {
    await accountStore.triggerSync(accountId)
  } catch (error) {
    console.error('触发同步失败:', error)
  }
}

async function retrySync(accountId: string) {
  await triggerSync(accountId)
}

function refreshStatus() {
  // 状态通过事件自动更新，这里主要是触发状态检查
  console.log('[SyncStatus] 刷新同步状态')
}

// 生命周期
onMounted(() => {
  if (props.multiAccount) {
    refreshStatus()
    // 每5秒刷新一次状态
    refreshInterval.value = window.setInterval(() => {
      refreshStatus()
    }, 5000)
  }
})

onUnmounted(() => {
  if (refreshInterval.value) {
    clearInterval(refreshInterval.value)
  }
})
</script>

<style scoped>
/* ========================================
   单账号模式样式
   ======================================== */

.sync-status-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 80px;
}

.syncing-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--primary-color);
}

.syncing-indicator .status-text {
  font-size: 12px;
  color: var(--text-secondary);
}

.syncing-indicator .progress-text {
  font-size: 11px;
  color: var(--primary-color);
  font-weight: 500;
}

.progress-detail p {
  margin: 2px 0;
  font-size: 12px;
}

.completed-indicator,
.error-indicator,
.idle-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
}

.icon-success {
  color: #18a058;
  font-size: 16px;
}

.icon-error {
  color: #d03050;
  font-size: 16px;
}

.icon-idle {
  color: #63a4c7;
  font-size: 14px;
}

.sync-status-indicator.syncing {
  color: var(--primary-color);
}

.sync-status-indicator.error {
  color: #d03050;
}

/* ========================================
   多账号模式样式
   ======================================== */

.multi-account-sync-status {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.status-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--n-color);
  border-radius: 8px;
  border: 1px solid var(--n-border-color);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.title {
  font-size: 14px;
  font-weight: 500;
  color: var(--n-text-color);
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-history,
.icon-refresh {
  font-size: 14px;
}

.accounts-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 400px;
  overflow-y: auto;
}

.account-status-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px;
  background: var(--n-color);
  border-radius: 8px;
  border: 1px solid var(--n-border-color);
  transition: all 0.2s;
}

.account-status-item:hover {
  border-color: var(--primary-color);
}

.account-status-item.syncing {
  border-color: var(--primary-color);
  background: rgba(24, 160, 88, 0.05);
}

.account-status-item.error {
  border-color: #d03050;
  background: rgba(208, 48, 80, 0.05);
}

.account-info {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
}

.account-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-weight: 600;
  font-size: 14px;
  flex-shrink: 0;
}

.account-details {
  flex: 1;
  min-width: 0;
}

.account-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--n-text-color);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.account-email {
  font-size: 11px;
  color: var(--n-text-color-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.account-status {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.status-detail {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 4px;
}

.status-message {
  font-size: 12px;
  color: var(--n-text-color-2);
}

.status-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
}

.stage-badge {
  padding: 2px 6px;
  background: var(--primary-color);
  color: white;
  border-radius: 4px;
  font-size: 10px;
}

.folder-info {
  color: var(--n-text-color-3);
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.error-message {
  font-size: 12px;
  color: #d03050;
}

.success-message {
  font-size: 12px;
  color: #18a058;
}

.idle-message {
  font-size: 12px;
  color: var(--n-text-color-3);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px;
  color: var(--n-text-color-3);
}

.icon-empty {
  font-size: 32px;
  margin-bottom: 8px;
}

.empty-state p {
  font-size: 13px;
}

/* ========================================
   同步历史样式
   ======================================== */

.sync-history {
  display: flex;
  flex-direction: column;
}

.empty-history {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  color: var(--n-text-color-3);
}

.history-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.history-item {
  padding: 12px;
  background: var(--n-color);
  border-radius: 8px;
  border: 1px solid var(--n-border-color);
}

.history-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.history-email {
  font-size: 13px;
  font-weight: 500;
  color: var(--n-text-color);
}

.history-time {
  font-size: 11px;
  color: var(--n-text-color-3);
}

.history-result {
  display: flex;
  align-items: center;
  gap: 8px;
}

.result-stats {
  font-size: 12px;
  color: var(--n-text-color-2);
}
</style>
