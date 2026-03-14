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
      console.log('[SyncStore] 开始监听同步进度:', accountId)

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
          console.log('[SyncStore] 收到进度事件:', event.payload)
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
      console.log('[SyncStore] 处理进度事件:', { accountId, progress })

      // 确保 progress.account_id 也是数字类型
      const eventAccountId = typeof progress.account_id === 'string'
        ? parseInt(progress.account_id, 10)
        : progress.account_id

      let stage: 'idle' | 'syncing' | 'completed' | 'error'

      switch (progress.stage) {
        case 'connecting':
        case 'syncing_folders':
        case 'syncing_emails':
          stage = 'syncing'
          this.syncingAccounts.add(eventAccountId)
          break
        case 'completed':
          stage = 'completed'
          this.syncingAccounts.delete(eventAccountId)
          break
        case 'error':
          stage = 'error'
          this.syncingAccounts.delete(eventAccountId)
          break
        default:
          stage = 'idle'
      }

      console.log('[SyncStore] 转换后 stage:', stage, 'syncingAccounts:', Array.from(this.syncingAccounts))

      // 计算进度百分比
      let progressPercent = 0
      if (progress.total > 0) {
        progressPercent = Math.floor((progress.current / progress.total) * 100)
      }

      // 更新状态
      this.syncStatuses.set(eventAccountId, {
        account_id: eventAccountId,
        stage,
        progress: progressPercent,
        message: progress.message,
        current_folder: progress.folder,
      })

      console.log('[SyncStore] 当前状态:', Array.from(this.syncStatuses.entries()))
    },

    /**
     * 同步指定账号
     */
    async syncAccount(accountId: number | string): Promise<SyncResult | null> {
      // 确保 accountId 是数字类型
      const numericAccountId = typeof accountId === 'string' ? parseInt(accountId, 10) : accountId

      try {
        console.log('[SyncStore] syncAccount 调用:', { accountId, numericAccountId, type: typeof accountId })

        // 开始监听进度
        this.startListening(numericAccountId)

        // 调用后端同步命令
        await invoke('sync_account_with_progress', { accountId: numericAccountId })

        // 等待一小段时间，确保最后的进度事件已接收
        await new Promise(resolve => setTimeout(resolve, 500))

        // 获取最终状态
        const status = this.syncStatuses.get(numericAccountId)
        if (status?.stage === 'completed') {
          return {
            total_synced: status.progress || 0,
            folders_synced: 0,
            errors: 0,
            duration_ms: 0,
          }
        }

        return null
      } catch (error: any) {
        console.error('同步失败:', error)

        // 解析错误信息并提供友好的提示
        let errorMessage = String(error)
        if (errorMessage.includes('账号密码不存在') || errorMessage.includes('密码未找到')) {
          errorMessage = '账号密码未保存，请删除账号后重新添加以设置密码'
        } else if (errorMessage.includes('连接 IMAP 服务器失败')) {
          errorMessage = '无法连接到邮件服务器，请检查网络或账号配置'
        } else if (errorMessage.includes('账号不存在')) {
          errorMessage = '账号不存在，请重新添加账号'
        }

        this.syncStatuses.set(numericAccountId, {
          account_id: numericAccountId,
          stage: 'error',
          progress: 0,
          message: errorMessage,
          error: String(error),
        })
        this.syncingAccounts.delete(numericAccountId)
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
