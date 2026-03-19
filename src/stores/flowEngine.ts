import { defineStore } from 'pinia'
import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { FlowEngineStatus } from '@/types'

/**
 * FlowEngine Store
 *
 * 管理流程引擎状态，提供状态查询和轮询功能
 */
export const useFlowEngineStore = defineStore('flowEngine', () => {
  // ========== State ==========
  const status = ref<FlowEngineStatus | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  // ========== Getters ==========

  /**
   * 引擎是否正在运行
   */
  const isRunning = computed(() => status.value?.isRunning ?? false)

  /**
   * 是否有活动任务
   */
  const hasActiveTasks = computed(() => (status.value?.activeTasks ?? 0) > 0)

  /**
   * 是否有排队中的任务
   */
  const hasQueuedTasks = computed(() => (status.value?.queuedTasks ?? 0) > 0)

  /**
   * 总任务数（活动 + 排队）
   */
  const totalPendingTasks = computed(
    () => (status.value?.activeTasks ?? 0) + (status.value?.queuedTasks ?? 0)
  )

  /**
   * 完成率
   */
  const completionRate = computed(() => {
    const total = (status.value?.completedTasks ?? 0) + (status.value?.failedTasks ?? 0)
    if (total === 0) return 0
    return (status.value?.completedTasks ?? 0) / total * 100
  })

  // ========== Actions ==========

  /**
   * 获取 FlowEngine 状态
   */
  async function fetchStatus() {
    isLoading.value = true
    error.value = null
    try {
      status.value = await invoke<FlowEngineStatus>('get_flow_engine_status')
      console.log('[FlowEngineStore] 状态更新:', status.value)
    } catch (err: any) {
      error.value = String(err)
      console.error('[FlowEngineStore] 获取状态失败:', err)
    } finally {
      isLoading.value = false
    }
  }

  /**
   * 开始定期轮询状态
   * @param interval 轮询间隔（毫秒），默认 5000ms
   */
  function startPolling(interval: number = 5000) {
    console.log('[FlowEngineStore] 开始轮询状态，间隔:', interval)

    // 立即获取一次状态
    fetchStatus()

    const intervalId = setInterval(() => {
      fetchStatus()
    }, interval)

    // 组件卸载时清理
    onUnmounted(() => {
      clearInterval(intervalId)
      console.log('[FlowEngineStore] 停止轮询')
    })

    return intervalId
  }

  /**
   * 重置状态
   */
  function reset() {
    status.value = null
    error.value = null
    isLoading.value = false
  }

  return {
    // State
    status,
    isLoading,
    error,

    // Getters
    isRunning,
    hasActiveTasks,
    hasQueuedTasks,
    totalPendingTasks,
    completionRate,

    // Actions
    fetchStatus,
    startPolling,
    reset,
  }
})
