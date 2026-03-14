import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Account, EmailProvider } from '@/types'
import { invoke } from '@tauri-apps/api/core'
import { useSyncStore } from './sync'

// 后端 DTO 类型定义
interface AccountDto {
  id: number
  name: string
  email: string
  provider: string
  imap_host: string | null
  imap_port: number | null
  imap_ssl: boolean | null
  smtp_host: string | null
  smtp_port: number | null
  smtp_ssl: boolean | null
  color: string | null
  sync_enabled: boolean
  last_sync_at: number | null
  created_at: number
  updated_at: number
}

interface CreateAccountRequest {
  name: string
  email: string
  provider: string
  password: string
  imap_host?: string
  imap_port?: number
  imap_ssl?: boolean
  smtp_host?: string
  smtp_port?: number
  smtp_ssl?: boolean
  color?: string
}

interface ConnectionTestResult {
  success: boolean
  connect_time: number
  login_time: number
  email_count: number
  error?: string
}

// DTO 转换为前端 Account 类型
function dtoToAccount(dto: AccountDto): Account {
  return {
    id: dto.id.toString(),
    name: dto.name,
    email: dto.email,
    provider: dto.provider as EmailProvider,
    color: dto.color || '#7C3AED',
    unreadCount: 0, // TODO: 从后端获取
    imapHost: dto.imap_host || undefined,
    imapPort: dto.imap_port || undefined,
    imapSsl: dto.imap_ssl ?? undefined,
    smtpHost: dto.smtp_host || undefined,
    smtpPort: dto.smtp_port || undefined,
    smtpSsl: dto.smtp_ssl ?? undefined,
    syncEnabled: dto.sync_enabled,
    lastSyncAt: dto.last_sync_at ? new Date(dto.last_sync_at) : undefined,
  }
}

// 前端 Account 转换为后端请求类型
function accountToRequest(account: Partial<Account> & { password?: string }): CreateAccountRequest {
  return {
    name: account.name || '',
    email: account.email || '',
    provider: account.provider || 'imap',
    password: account.password || '',
    imap_host: account.imapHost,
    imap_port: account.imapPort,
    imap_ssl: account.imapSsl,
    smtp_host: account.smtpHost,
    smtp_port: account.smtpPort,
    smtp_ssl: account.smtpSsl,
    color: account.color,
  }
}

export const useAccountStore = defineStore('account', () => {
  // ========================================
  // State
  // ========================================

  // 账号列表
  const accounts = ref<Account[]>([])

  // 当前激活的账号
  const currentAccount = ref<Account | null>(null)

  // 加载状态
  const isLoading = ref(false)

  // 账号选择器下拉状态
  const isDropdownOpen = ref(false)

  // ========================================
  // Getters
  // ========================================

  // 是否有账号
  const hasAccounts = computed(() => accounts.value.length > 0)

  // 总未读邮件数量
  const totalUnreadCount = computed(() =>
    accounts.value.reduce((sum, account) => sum + account.unreadCount, 0)
  )

  // 按提供商分组的账号
  const accountsByProvider = computed(() => {
    const grouped: Record<EmailProvider, Account[]> = {
      gmail: [],
      outlook: [],
      icloud: [],
      yahoo: [],
      imap: [],
    }

    accounts.value.forEach(account => {
      grouped[account.provider].push(account)
    })

    return grouped
  })

  // ========================================
  // Actions
  // ========================================

  // 获取账号列表
  async function fetchAccounts() {
    console.log('[fetchAccounts] 开始加载账号')
    isLoading.value = true
    try {
      const dtos = await invoke<AccountDto[]>('list_accounts')
      console.log('[fetchAccounts] 后端返回账号数量:', dtos.length)
      accounts.value = dtos.map(dtoToAccount)

      // 默认选择第一个账号
      // 如果当前没有选中账号，或者选中的账号不在列表中，则选择第一个
      if (accounts.value.length > 0) {
        const currentExists = currentAccount.value &&
          accounts.value.some(a => a.id === currentAccount.value!.id)
        if (!currentAccount.value || !currentExists) {
          currentAccount.value = accounts.value[0]
          console.log('[fetchAccounts] 设置当前账号:', currentAccount.value)
        }
      } else {
        // 没有账号时清空当前账号
        currentAccount.value = null
        console.log('[fetchAccounts] 没有账号')
      }
    } catch (error) {
      console.error('获取账号列表失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  // 选择账号
  function selectAccount(account: Account) {
    currentAccount.value = account
    isDropdownOpen.value = false
  }

  // 通过 ID 选择账号
  function selectAccountById(accountId: string) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      currentAccount.value = account
    }
  }

  // 添加账号
  async function addAccount(accountData: Partial<Account>): Promise<Account> {
    isLoading.value = true
    try {
      const request = accountToRequest(accountData)
      const dto = await invoke<AccountDto>('add_account', { account: request })
      const account = dtoToAccount(dto)

      accounts.value.push(account)
      return account
    } catch (error) {
      console.error('添加账号失败:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  // 更新账号
  async function updateAccount(accountId: string, updates: Partial<Account>): Promise<Account> {
    try {
      const request = accountToRequest(updates)
      const dto = await invoke<AccountDto>('update_account', {
        id: parseInt(accountId),
        account: request,
      })
      const account = dtoToAccount(dto)

      // 更新本地列表
      const index = accounts.value.findIndex(a => a.id === accountId)
      if (index !== -1) {
        accounts.value[index] = account
      }

      // 如果更新的是当前账号，也更新当前账号
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = account
      }

      return account
    } catch (error) {
      console.error('更新账号失败:', error)
      throw error
    }
  }

  // 删除账号
  async function removeAccount(accountId: string) {
    try {
      await invoke('delete_account', { id: parseInt(accountId) })

      const index = accounts.value.findIndex(a => a.id === accountId)
      if (index !== -1) {
        accounts.value.splice(index, 1)
      }

      // 如果删除的是当前账号，切换到第一个账号
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = accounts.value.length > 0 ? accounts.value[0] : null
      }
    } catch (error) {
      console.error('删除账号失败:', error)
      throw error
    }
  }

  // 测试账号连接
  async function testConnection(accountData: Partial<Account>): Promise<ConnectionTestResult> {
    try {
      const request = accountToRequest(accountData)
      const result = await invoke<ConnectionTestResult>('test_account_connection', {
        account: request,
      })
      return result
    } catch (error) {
      console.error('测试连接失败:', error)
      throw error
    }
  }

  // 更新账号未读数量
  function updateUnreadCount(accountId: string, count: number) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      account.unreadCount = count
    }
  }

  // 增加未读数量
  function incrementUnreadCount(accountId: string) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      account.unreadCount++
    }
  }

  // 减少未读数量
  function decrementUnreadCount(accountId: string) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account && account.unreadCount > 0) {
      account.unreadCount--
    }
  }

  // 切换下拉状态
  function toggleDropdown() {
    isDropdownOpen.value = !isDropdownOpen.value
  }

  // 关闭下拉
  function closeDropdown() {
    isDropdownOpen.value = false
  }

  // 清空所有账号
  function clearAccounts() {
    accounts.value = []
    currentAccount.value = null
  }

  // 同步账号邮件
  async function syncAccount(accountId?: string) {
    const id = accountId ? parseInt(accountId) : (currentAccount.value ? parseInt(currentAccount.value.id) : null)
    if (!id) {
      throw new Error('没有可同步的账号')
    }

    // 使用 sync store 进行同步
    const syncStore = useSyncStore()
    const result = await syncStore.syncAccount(id)

    // 更新账号列表
    await fetchAccounts()

    return result?.total_synced || 0
  }

  // ========================================
  // Helper Functions
  // ========================================

  return {
    // State
    accounts,
    currentAccount,
    isLoading,
    isDropdownOpen,

    // Getters
    hasAccounts,
    totalUnreadCount,
    accountsByProvider,

    // Actions
    fetchAccounts,
    selectAccount,
    selectAccountById,
    addAccount,
    updateAccount,
    removeAccount,
    testConnection,
    updateUnreadCount,
    incrementUnreadCount,
    decrementUnreadCount,
    toggleDropdown,
    closeDropdown,
    clearAccounts,
    syncAccount,
  }
})
