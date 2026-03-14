<template>
  <div v-if="syncStore.hasAnySyncing" class="sync-progress-widget">
    <div v-for="status in syncStore.allStatuses" :key="status.account_id" class="sync-item">
      <!-- 进度条 -->
      <div class="sync-info">
        <div class="sync-header">
          <span class="sync-account">{{ getAccountName(status.account_id) }}</span>
          <span class="sync-stage">{{ getStageText(status.stage) }}</span>
        </div>
        <div class="sync-message">{{ status.message }}</div>
      </div>

      <!-- 进度条 -->
      <NProgress
        v-if="status.stage === 'syncing'"
        type="line"
        :percentage="status.progress"
        :processing="true"
        :show-indicator="false"
        :style="{ flex: 1, marginLeft: '12px' }"
      />

      <!-- 完成或错误图标 -->
      <div v-else-if="status.stage === 'completed'" class="sync-icon completed">
        ✓
      </div>
      <div v-else-if="status.stage === 'error'" class="sync-icon error">
        ✗
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { NProgress } from 'naive-ui'
import { useSyncStore } from '@/stores/sync'
import { useAccountStore } from '@/stores/account'

const syncStore = useSyncStore()
const accountStore = useAccountStore()

const getAccountName = (accountId: number) => {
  const account = accountStore.accounts.find((a: any) => a.id === accountId)
  return account?.email || `账号 ${accountId}`
}

const getStageText = (stage: string) => {
  const map: Record<string, string> = {
    idle: '准备中',
    syncing: '同步中',
    completed: '已完成',
    error: '失败'
  }
  return map[stage] || stage
}
</script>

<style scoped>
.sync-progress-widget {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  background: var(--bg-secondary);
  border-radius: 8px;
}

.sync-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.sync-info {
  flex: 1;
  min-width: 0;
}

.sync-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.sync-account {
  font-weight: 500;
  font-size: 13px;
  color: var(--text-primary);
}

.sync-stage {
  font-size: 12px;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
}

.sync-message {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sync-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
}
</style>
