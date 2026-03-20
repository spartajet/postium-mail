import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Account, EmailProvider } from '@/types'
import { AccountType, AuthType } from '@/types'
import { invoke } from '@tauri-apps/api/core'
import { useSyncStore } from './sync'

// ============================================================
// 后端 DTO 类型定义
// ============================================================

interface AccountDto {
  id: number
  name: string
  email: string
  provider: string
  account_type?: string
  auth_type?: string
  imap_host: string | null
  imap_port: number | null
  imap_ssl: boolean | null
  smtp_host: string | null
  smtp_port: number | null
  smtp_ssl: boolean | null
  color: string | null
  sync_enabled: boolean
  last_sync_at: number | null
  oauth_provider?: string | null
  oauth_token_expiry?: number | null
  enterprise_tenant_id?: string | null
  enterprise_domain?: string | null
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
    // 新增字段
    accountType: parseAccountType(dto.account_type),
    authType: parseAuthType(dto.auth_type),
    // IMAP/SMTP 配置
    imapHost: dto.imap_host || undefined,
    imapPort: dto.imap_port || undefined,
    imapSsl: dto.imap_ssl ?? undefined,
    smtpHost: dto.smtp_host || undefined,
    smtpPort: dto.smtp_port || undefined,
    smtpSsl: dto.smtp_ssl ?? undefined,
    // UI 配置
    color: dto.color || '#7C3AED',
    unreadCount: 0, // TODO: 从后端获取
    syncEnabled: dto.sync_enabled,
    lastSyncAt: dto.last_sync_at ? new Date(dto.last_sync_at) : undefined,
    // OAuth 相关
    oauthProvider: dto.oauth_provider || undefined,
    oauthTokenExpiry: dto.oauth_token_expiry ? new Date(dto.oauth_token_expiry) : undefined,
    // 企业配置
    enterpriseTenantId: dto.enterprise_tenant_id || undefined,
    enterpriseDomain: dto.enterprise_domain || undefined,
    // 时间戳
    createdAt: new Date(dto.created_at),
    updatedAt: new Date(dto.updated_at),
  }
}

// 解析账号类型
function parseAccountType(type?: string): AccountType {
  if (type === 'enterprise') return AccountType.Enterprise
  return AccountType.Personal
}

// 解析认证类型
function parseAuthType(type?: string): AuthType {
  switch (type) {
    case 'oauth2': return AuthType.OAuth2
    case 'app_password': return AuthType.AppPassword
    case 'domain_auth': return AuthType.DomainAuth
    case 'saml_sso': return AuthType.SamlSso
    default: return AuthType.Password
  }
}

// 前端 Account 转换为后端请求类型
function accountToRequest(account: Partial<Account> & { password?: string }): CreateAccountRequest {
  // 如果用户没有指定 provider，使用 'auto' 让后端自动检测
  // 只有当用户明确指定了 imapHost/smtpHost 时，才使用 'imap'
  const hasCustomServer = account.imapHost || account.smtpHost
  const defaultProvider = hasCustomServer ? 'imap' : 'auto'

  return {
    name: account.name || '',
    email: account.email || '',
    provider: account.provider || defaultProvider,
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

  // 按账号类型分组（个人/企业）
  const accountsByType = computed(() => {
    const personal = accounts.value.filter(a => a.accountType === AccountType.Personal)
    const enterprise = accounts.value.filter(a => a.accountType === AccountType.Enterprise)
    return { personal, enterprise }
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
    console.log('[selectAccountById] 尝试选择账号:', accountId)
    console.log('[selectAccountById] 当前账号列表:', accounts.value.map(a => ({ id: a.id, email: a.email })))
    const account = accounts.value.find(a => a.id === accountId)
    if (account) {
      console.log('[selectAccountById] 找到账号，设置为当前账号:', account.email)
      currentAccount.value = account
    } else {
      console.warn('[selectAccountById] 未找到账号:', accountId)
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

      // 如果删除的是当前账号，切换到第一个账号并清空邮件列表
      if (currentAccount.value?.id === accountId) {
        currentAccount.value = accounts.value.length > 0 ? accounts.value[0] : null

        // 清空邮件列表和状态（刷新界面）
        const { useEmailStore } = await import('./email')
        const emailStore = useEmailStore()
        emailStore.clearEmails()
        emailStore.clearSelection()
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

    // 获取账号 email
    const account = accounts.value.find(a => parseInt(a.id) === id)
    const accountEmail = account?.email

    // 使用 sync store 进行同步
    const syncStore = useSyncStore()
    const result = await syncStore.syncAccount(id, accountEmail)

    // 更新账号列表
    await fetchAccounts()

    return result?.total_synced || 0
  }

  // ========================================
  // OAuth 认证相关方法
  // ========================================

  /**
   * 获取 OAuth 授权 URL
   * @param provider 邮件服务商 (gmail, outlook 等)
   * @returns 授权 URL
   */
  async function getOAuthAuthUrl(provider: string): Promise<string> {
    try {
      return await invoke<string>('get_oauth_auth_url', { provider })
    } catch (error) {
      console.error('[AccountStore] 获取 OAuth 授权 URL 失败:', error)
      throw error
    }
  }

  /**
   * 交换 OAuth 授权码以获取访问令牌并添加账号
   * @param code OAuth 授权码
   * @param state 状态参数
   * @returns 新创建的账号
   */
  async function exchangeOAuthCode(code: string, state: string): Promise<Account> {
    try {
      console.log('[AccountStore] 交换 OAuth 授权码')
      const dto = await invoke<AccountDto>('exchange_oauth_code', { code, state })
      const account = dtoToAccount(dto)

      // 检查账号是否已存在
      const exists = accounts.value.some(a => a.email === account.email)
      if (!exists) {
        accounts.value.push(account)
      }

      // 自动选中新添加的账号
      currentAccount.value = account

      console.log('[AccountStore] OAuth 账号添加成功:', account)
      return account
    } catch (error) {
      console.error('[AccountStore] OAuth 授权码交换失败:', error)
      throw error
    }
  }

  /**
   * 刷新 OAuth Token
   * @param accountId 账号 ID
   */
  async function refreshOAuthToken(accountId: string): Promise<void> {
    try {
      console.log('[AccountStore] 刷新 OAuth Token:', accountId)
      await invoke('refresh_oauth_token', {
        accountId: parseInt(accountId)
      })
      // 刷新成功后更新账号信息
      await fetchAccounts()
    } catch (error) {
      console.error('[AccountStore] 刷新 OAuth Token 失败:', error)
      throw error
    }
  }

  /**
   * 验证 OAuth Token
   * @param accountId 账号 ID
   * @returns Token 是否有效
   */
  async function validateOAuthToken(accountId: string): Promise<boolean> {
    try {
      return await invoke<boolean>('validate_oauth_token', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[AccountStore] 验证 OAuth Token 失败:', error)
      return false
    }
  }

  // ========================================
  // FlowEngine 任务管理相关方法
  // ========================================

  /**
   * 添加同步任务到 FlowEngine
   * @param accountId 账号 ID
   * @param schedule 可选的 Cron 表达式，例如每5分钟执行一次
   */
  async function addSyncTask(accountId: string, schedule?: string): Promise<void> {
    try {
      console.log('[AccountStore] 添加同步任务:', { accountId, schedule })
      await invoke('add_sync_task', {
        accountId: parseInt(accountId),
        schedule,
      })
    } catch (error) {
      console.error('[AccountStore] 添加同步任务失败:', error)
      throw error
    }
  }

  /**
   * 从 FlowEngine 移除同步任务
   * @param accountId 账号 ID
   */
  async function removeSyncTask(accountId: string): Promise<void> {
    try {
      console.log('[AccountStore] 移除同步任务:', accountId)
      await invoke('remove_sync_task', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[AccountStore] 移除同步任务失败:', error)
      throw error
    }
  }

  /**
   * 暂停同步任务
   * @param accountId 账号 ID
   */
  async function pauseSyncTask(accountId: string): Promise<void> {
    try {
      console.log('[AccountStore] 暂停同步任务:', accountId)
      await invoke('pause_sync_task', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[AccountStore] 暂停同步任务失败:', error)
      throw error
    }
  }

  /**
   * 恢复同步任务
   * @param accountId 账号 ID
   */
  async function resumeSyncTask(accountId: string): Promise<void> {
    try {
      console.log('[AccountStore] 恢复同步任务:', accountId)
      await invoke('resume_sync_task', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[AccountStore] 恢复同步任务失败:', error)
      throw error
    }
  }

  /**
   * 触发一次性同步
   * @param accountId 账号 ID
   */
  async function triggerSync(accountId: string): Promise<void> {
    try {
      console.log('[AccountStore] 触发一次性同步:', accountId)
      await invoke('trigger_sync', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[AccountStore] 触发同步失败:', error)
      throw error
    }
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
    accountsByType,

    // Actions - 基础 CRUD
    fetchAccounts,
    selectAccount,
    selectAccountById,
    addAccount,
    updateAccount,
    removeAccount,
    testConnection,

    // Actions - 未读数管理
    updateUnreadCount,
    incrementUnreadCount,
    decrementUnreadCount,

    // Actions - UI 状态
    toggleDropdown,
    closeDropdown,
    clearAccounts,

    // Actions - 同步
    syncAccount,

    // Actions - OAuth 认证
    getOAuthAuthUrl,
    exchangeOAuthCode,
    refreshOAuthToken,
    validateOAuthToken,

    // Actions - FlowEngine 任务管理
    addSyncTask,
    removeSyncTask,
    pauseSyncTask,
    resumeSyncTask,
    triggerSync,
  }
})
