<template>
  <div class="sync-status-indicator" :class="{ syncing, error: hasError }">
    <!-- 同步中 -->
    <div v-if="syncing" class="syncing-indicator">
      <NSpin :size="14" />
      <span class="status-text">{{ syncStatus?.message || '同步中...' }}</span>
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
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { NTooltip, NSpin } from 'naive-ui'
import { useSyncStore } from '@/stores/sync'

interface Props {
  accountId: number
  lastSyncTime?: number
}

const props = defineProps<Props>()

const syncStore = useSyncStore()

const syncing = computed(() => syncStore.isSyncing(props.accountId))
const syncStatus = computed(() => syncStore.getStatus(props.accountId))
const hasSynced = computed(() => props.lastSyncTime && props.lastSyncTime > 0)
const hasError = computed(() => syncStatus.value?.stage === 'error')

const formatTime = (timestamp?: number) => {
  if (!timestamp) return '-'
  const date = new Date(timestamp * 1000)
  const now = new Date()
  const diff = Math.floor((now.getTime() - date.getTime()) / 1000 / 60) // 分钟差

  if (diff < 1) return '刚刚'
  if (diff < 60) return `${diff} 分钟前`
  if (diff < 1440) return `${Math.floor(diff / 60)} 小时前`
  return `${Math.floor(diff / 1440)} 天前`
}
</script>

<style scoped>
.sync-status-indicator {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 80px;
}

.syncing-indicator {
  display: flex;
  align-items: center;
  gap: 4px;
  color: var(--primary-color);
}

.syncing-indicator .status-text {
  font-size: 12px;
  color: var(--text-secondary);
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
</style>
