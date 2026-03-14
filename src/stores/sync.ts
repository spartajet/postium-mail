import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

/**
 * 同步进度事件
 */
export interface SyncProgressEvent {
  account_id: number
  stage: 'connecting' | 'syncing_folders' | 'syncing_emails' | 'completed' | 'error'
  folder?: string
  current: number
  total: number
  message: string
}

/**
 * 同步结果
 */
export interface SyncResult {
  total_synced: number
  folders_synced: number
  errors: number
  duration_ms: number
}

/**
 * 同步状态
 */
export interface SyncStatus {
  account_id: number
  stage: 'idle' | 'syncing' | 'completed' | 'error'
  progress: number
  message: string
  current_folder?: string
  result?: SyncResult
  error?: string
}

/**
 * 同步 Store
 */
export const useSyncStore = defineStore('sync', {
  state: () => ({
    // 正在同步的账号 ID 集合
    syncingAccounts: new Set<number>(),
    // 每个账号的同步状态
    syncStatuses: new Map<number, SyncStatus>(),
    // 事件监听器清理函数（Promise）
    unlisteners: new Map<number, Promise<UnlistenFn>>(),
  }),

  getters: {
    /**
     * 是否正在同步指定账号
     */
    isSyncing: (state) => (accountId: number) => {
      return state.syncingAccounts.has(accountId)
    },

    /**
     * 是否有任何账号正在同步
     */
    hasAnySyncing: (state) => {
      return state.syncingAccounts.size > 0
    },

    /**
     * 获取指定账号的同步状态
     */
    getStatus: (state) => (accountId: number) => {
      return state.syncStatuses.get(accountId)
    },

    /**
     * 获取所有同步状态
     */
    allStatuses: (state) => {
      return Array.from(state.syncStatuses.values())
    },
  },

  actions: {
    /**
     * 开始监听指定账号的同步进度
     */
    startListening(accountId: number) {
      // 如果已经在监听，先清理
      this.stopListening(accountId)

      // 初始化状态
      this.syncStatuses.set(accountId, {
        account_id: accountId,
        stage: 'idle',
        progress: 0,
        message: '准备同步...',
      })

      // 监听同步进度事件
      const unlisten = listen<SyncProgressEvent>(
        `sync-progress-${accountId}`,
        (event: any) => {
          const progress = event.payload
          this.handleProgressEvent(accountId, progress)
        }
      )

      // 保存清理函数
      this.unlisteners.set(accountId, unlisten)
    },

    /**
     * 停止监听指定账号的同步进度
     */
    stopListening(accountId: number) {
      const unlisten = this.unlisteners.get(accountId)
      if (unlisten) {
        unlisten.then((fn: any) => fn())
        this.unlisteners.delete(accountId)
      }
    },

    /**
     * 处理同步进度事件
     */
    handleProgressEvent(accountId: number, progress: SyncProgressEvent) {
      let stage: 'idle' | 'syncing' | 'completed' | 'error'

      switch (progress.stage) {
        case 'connecting':
        case 'syncing_folders':
        case 'syncing_emails':
          stage = 'syncing'
          this.syncingAccounts.add(accountId)
          break
        case 'completed':
          stage = 'completed'
          this.syncingAccounts.delete(accountId)
          break
        case 'error':
          stage = 'error'
          this.syncingAccounts.delete(accountId)
          break
        default:
          stage = 'idle'
      }

      // 计算进度百分比
      let progressPercent = 0
      if (progress.total > 0) {
        progressPercent = Math.floor((progress.current / progress.total) * 100)
      }

      // 更新状态
      this.syncStatuses.set(accountId, {
        account_id: accountId,
        stage,
        progress: progressPercent,
        message: progress.message,
        current_folder: progress.folder,
      })
    },

    /**
     * 同步指定账号
     */
    async syncAccount(accountId: number): Promise<SyncResult | null> {
      try {
        // 开始监听进度
        this.startListening(accountId)

        // 调用后端同步命令
        await invoke('sync_account_with_progress', { accountId })

        // 等待一小段时间，确保最后的进度事件已接收
        await new Promise(resolve => setTimeout(resolve, 500))

        // 获取最终状态
        const status = this.syncStatuses.get(accountId)
        if (status?.stage === 'completed') {
          return {
            total_synced: status.progress || 0,
            folders_synced: 0,
            errors: 0,
            duration_ms: 0,
          }
        }

        return null
      } catch (error) {
        console.error('同步失败:', error)
        this.syncStatuses.set(accountId, {
          account_id: accountId,
          stage: 'error',
          progress: 0,
          message: String(error),
          error: String(error),
        })
        this.syncingAccounts.delete(accountId)
        return null
      }
    },

    /**
     * 清除指定账号的同步状态
     */
    clearStatus(accountId: number) {
      this.syncStatuses.delete(accountId)
      this.syncingAccounts.delete(accountId)
    },

    /**
     * 清除所有同步状态
     */
    clearAllStatuses() {
      this.syncStatuses.clear()
      this.syncingAccounts.clear()
      // 清理所有监听器
      this.unlisteners.forEach((unlistenPromise: any, _accountId: number) => {
        unlistenPromise.then((fn: any) => fn())
      })
      this.unlisteners.clear()
    },
  },
})
