import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/**
 * 使用默认浏览器打开 URL
 */
async function openInBrowser(url: string): Promise<void> {
  try {
    // 使用 Tauri 的 shell-open 命令
    await invoke('plugin:shell|open', { path: url })
  } catch (error) {
    console.error('[OAuthHelper] 打开浏览器失败:', error)
    // 如果 Tauri 命令失败，尝试使用 window.open
    window.open(url, '_blank')
  }
}
import type { Account } from '@/types'
import { useAccountStore } from '@/stores/account'
import { ErrorHandler } from './errorHandler'

/**
 * OAuth 回调事件载荷（旧版 deep-link）
 */
interface OAuthCallbackPayload {
  code?: string
  state?: string
  error?: string
  errorDescription?: string
}

/**
 * OAuth 流程完成事件载荷（新版 HTTP localhost）
 */
interface OAuthFlowResultPayload {
  session_id: string
  status: 'success' | 'error'
  account?: Account
  error?: string
}

/**
 * OAuth 提供商配置
 */
interface OAuthProviderConfig {
  name: string
  displayName: string
  authUrl?: string
  scopes: string[]
}

/**
 * 支持的 OAuth 提供商配置
 */
const OAUTH_PROVIDERS: Record<string, OAuthProviderConfig> = {
  gmail: {
    name: 'gmail',
    displayName: 'Gmail',
    scopes: ['https://www.googleapis.com/auth/gmail.modify'],
  },
  outlook: {
    name: 'outlook',
    displayName: 'Outlook',
    scopes: ['https://outlook.office.com/Mail.ReadWrite', 'https://outlook.office.com/Mail.Send'],
  },
}

/**
 * OAuth 辅助工具类
 */
export class OAuthHelper {
  /**
   * 获取支持的 OAuth 提供商列表
   */
  static getSupportedProviders(): OAuthProviderConfig[] {
    return Object.values(OAUTH_PROVIDERS)
  }

  /**
   * 检查提供商是否支持 OAuth
   */
  static isProviderSupported(provider: string): boolean {
    return provider in OAUTH_PROVIDERS
  }

  /**
   * 获取提供商配置
   */
  static getProviderConfig(provider: string): OAuthProviderConfig | null {
    return OAUTH_PROVIDERS[provider] || null
  }

  /**
   * 启动 OAuth 登录流程
   * @param provider 邮件服务商 (gmail, outlook)
   * @param timeout 超时时间（毫秒），默认 5 分钟
   * @returns Promise<Account> 返回创建的账号
   */
  static async startLogin(provider: string, timeout: number = 5 * 60 * 1000): Promise<Account> {
    try {
      console.log('[OAuthHelper] 开始 OAuth 登录:', provider)

      // 检查提供商是否支持
      if (!this.isProviderSupported(provider)) {
        throw ErrorHandler.createError(
          'OAUTH_ERROR',
          `不支持的 OAuth 提供商: ${provider}`,
          'medium' as any,
          false
        )
      }

      // 获取授权 URL
      const authUrl = await invoke<string>('get_oauth_auth_url', { provider })
      console.log('[OAuthHelper] 获取到授权 URL')

      // 监听回调
      const accountPromise = this.listenCallback(timeout)

      // 打开外部浏览器
      await openInBrowser(authUrl)
      console.log('[OAuthHelper] 已打开浏览器')

      // 等待回调处理完成
      const account = await accountPromise
      console.log('[OAuthHelper] OAuth 登录成功:', account)

      return account
    } catch (error) {
      console.error('[OAuthHelper] OAuth 登录失败:', error)
      throw ErrorHandler.parse(error)
    }
  }

  /**
   * 监听 OAuth 回调
   * @param timeout 超时时间（毫秒）
   * @returns Promise<Account> 返回创建的账号
   */
  private static async listenCallback(timeout: number): Promise<Account> {
    return new Promise<Account>((resolve, reject) => {
      let isResolved = false

      // 设置超时
      const timeoutId = setTimeout(() => {
        if (!isResolved) {
          isResolved = true
          reject(ErrorHandler.createError('OAUTH_ERROR', 'OAuth 登录超时', 'medium' as any, false))
        }
      }, timeout)

      // 监听回调事件
      listen<OAuthFlowResultPayload>('oauth-flow-complete', (event) => {
        if (isResolved) return

        const payload = event.payload
        console.log('[OAuthHelper] 收到 OAuth 流程完成事件:', payload)

        clearTimeout(timeoutId)
        isResolved = true

        // 处理错误
        if (payload.status === 'error') {
          const error = ErrorHandler.createError(
            'OAUTH_ERROR',
            payload.error || 'OAuth 授权失败',
            'medium' as any,
            false
          )
          reject(error)
          return
        }

        // 处理成功
        if (payload.status === 'success' && payload.account) {
          console.log('[OAuthHelper] OAuth 登录成功:', payload.account)
          resolve(payload.account)
        } else {
          const error = ErrorHandler.createError(
            'OAUTH_ERROR',
            'OAuth 流程返回无效状态',
            'medium' as any,
            false
          )
          reject(error)
        }
      }).catch((error) => {
        clearTimeout(timeoutId)
        isResolved = true
        reject(error)
      })
    })
  }

  /**
   * 交换授权码获取访问令牌并添加账号
   */
  private static async exchangeCode(code: string, state: string): Promise<Account> {
    try {
      const accountStore = useAccountStore()
      const account = await accountStore.exchangeOAuthCode(code, state)
      return account
    } catch (error) {
      throw ErrorHandler.parse(error)
    }
  }

  /**
   * 刷新 OAuth Token
   * @param accountId 账号 ID
   * @returns Promise<boolean> 是否刷新成功
   */
  static async refreshTokens(accountId: string): Promise<boolean> {
    try {
      console.log('[OAuthHelper] 刷新 OAuth Token:', accountId)

      await invoke('refresh_oauth_token', {
        accountId: parseInt(accountId)
      })

      // 刷新成功后更新账号信息
      const accountStore = useAccountStore()
      await accountStore.fetchAccounts()

      return true
    } catch (error) {
      console.error('[OAuthHelper] 刷新 Token 失败:', error)
      return false
    }
  }

  /**
   * 验证 OAuth Token 是否有效
   * @param accountId 账号 ID
   * @returns Promise<boolean> Token 是否有效
   */
  static async validateToken(accountId: string): Promise<boolean> {
    try {
      console.log('[OAuthHelper] 验证 OAuth Token:', accountId)

      return await invoke<boolean>('validate_oauth_token', {
        accountId: parseInt(accountId)
      })
    } catch (error) {
      console.error('[OAuthHelper] 验证 Token 失败:', error)
      return false
    }
  }

  /**
   * 检查账号 Token 是否即将过期
   * @param account 账号对象
   * @param thresholdDays 提前天数，默认 7 天
   * @returns 是否即将过期
   */
  static isTokenExpiringSoon(account: Account, thresholdDays: number = 7): boolean {
    if (!account.oauthTokenExpiry) {
      return false
    }

    const now = new Date()
    const expiryDate = new Date(account.oauthTokenExpiry)
    const daysUntilExpiry = Math.floor((expiryDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24))

    return daysUntilExpiry <= thresholdDays
  }

  /**
   * 获取 Token 过期信息
   * @param account 账号对象
   * @returns 过期信息对象
   */
  static getTokenExpiryInfo(account: Account): {
    isExpired: boolean
    isExpiringSoon: boolean
    daysUntilExpiry: number | null
    expiryDate: Date | null
  } {
    if (!account.oauthTokenExpiry) {
      return {
        isExpired: false,
        isExpiringSoon: false,
        daysUntilExpiry: null,
        expiryDate: null,
      }
    }

    const now = new Date()
    const expiryDate = new Date(account.oauthTokenExpiry)
    const daysUntilExpiry = Math.floor((expiryDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24))

    return {
      isExpired: expiryDate < now,
      isExpiringSoon: this.isTokenExpiringSoon(account),
      daysUntilExpiry,
      expiryDate,
    }
  }

  /**
   * 格式化过期时间显示
   * @param account 账号对象
   * @returns 格式化的过期时间字符串
   */
  static formatExpiryDate(account: Account): string {
    const info = this.getTokenExpiryInfo(account)

    if (!info.expiryDate) {
      return '未设置'
    }

    if (info.isExpired) {
      return '已过期'
    }

    if (info.daysUntilExpiry === 0) {
      return '今天过期'
    }

    if (info.daysUntilExpiry === 1) {
      return '明天过期'
    }

    if (info.isExpiringSoon) {
      return `${info.daysUntilExpiry} 天后过期`
    }

    return info.expiryDate.toLocaleDateString('zh-CN')
  }

  /**
   * 批量刷新多个账号的 Token
   * @param accountIds 账号 ID 数组
   * @returns Promise<{success: string[], failed: string[]}> 刷新结果
   */
  static async refreshMultipleTokens(accountIds: string[]): Promise<{
    success: string[]
    failed: string[]
  }> {
    const success: string[] = []
    const failed: string[] = []

    for (const accountId of accountIds) {
      const result = await this.refreshTokens(accountId)
      if (result) {
        success.push(accountId)
      } else {
        failed.push(accountId)
      }
    }

    return { success, failed }
  }

  /**
   * 自动刷新即将过期的 Token
   * @param accounts 账号列表
   * @param thresholdDays 提前天数，默认 7 天
   * @returns Promise<{refreshed: number, failed: number}> 刷新统计
   */
  static async autoRefreshExpiringTokens(
    accounts: Account[],
    thresholdDays: number = 7
  ): Promise<{ refreshed: number; failed: number }> {
    const expiringAccounts = accounts.filter(account =>
      account.authType === 'oauth2' && this.isTokenExpiringSoon(account, thresholdDays)
    )

    console.log(`[OAuthHelper] 发现 ${expiringAccounts.length} 个即将过期的 Token`)

    let refreshed = 0
    let failed = 0

    for (const account of expiringAccounts) {
      const result = await this.refreshTokens(account.id)
      if (result) {
        refreshed++
      } else {
        failed++
      }
    }

    return { refreshed, failed }
  }
}

/**
 * 默认导出
 */
export default OAuthHelper

/**
 * 便捷导出
 */
export const {
  getSupportedProviders,
  isProviderSupported,
  getProviderConfig,
  startLogin,
  refreshTokens,
  validateToken,
  isTokenExpiringSoon,
  getTokenExpiryInfo,
  formatExpiryDate,
} = OAuthHelper
