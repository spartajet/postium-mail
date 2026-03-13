import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Account, EmailProvider } from '@/types'
import { generateAccounts, delay } from '@/mocks'

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
    isLoading.value = true
    try {
      await delay(200)
      accounts.value = generateAccounts(3)

      // 默认选择第一个账号
      if (accounts.value.length > 0 && !currentAccount.value) {
        currentAccount.value = accounts.value[0]
      }
    } catch (error) {
      console.error('Failed to fetch accounts:', error)
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
      await delay(500)

      const newAccount: Account = {
        id: `acc-${Date.now()}`,
        name: accountData.name || '新邮箱',
        email: accountData.email || '',
        provider: accountData.provider || 'imap',
        color: accountData.color || generateRandomColor(),
        unreadCount: 0,
        imapHost: accountData.imapHost,
        imapPort: accountData.imapPort,
        smtpHost: accountData.smtpHost,
        smtpPort: accountData.smtpPort,
      }

      accounts.value.push(newAccount)
      return newAccount
    } finally {
      isLoading.value = false
    }
  }

  // 更新账号
  function updateAccount(accountId: string, updates: Partial<Account>) {
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      Object.assign(account, updates)
    }
  }

  // 删除账号
  function removeAccount(accountId: string) {
    const index = accounts.value.findIndex(a => a.id === accountId)
    if (index !== -1) {
      accounts.value.splice(index, 1)

      // 如果删除的是当前账号，切换到第一个账号
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = accounts.value.length > 0 ? accounts.value[0] : null
      }
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

  // ========================================
  // Helper Functions
  // ========================================

  // 生成随机颜色
  function generateRandomColor(): string {
    const colors = [
      '#7C3AED', // Primary purple
      '#3B82F6', // Blue
      '#10B981', // Green
      '#F59E0B', // Amber
      '#EF4444', // Red
      '#EC4899', // Pink
      '#8B5CF6', // Violet
      '#06B6D4', // Cyan
    ]
    return colors[Math.floor(Math.random() * colors.length)]
  }

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
    updateUnreadCount,
    incrementUnreadCount,
    decrementUnreadCount,
    toggleDropdown,
    closeDropdown,
    clearAccounts,
  }
})
