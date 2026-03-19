import type { MailError } from '@/types'
import { ErrorSeverity } from '@/types'

/**
 * 错误代码常量
 */
export const ERROR_CODES = {
  // 认证相关
  AUTH_FAILED: 'AUTH_FAILED',
  AUTH_EXPIRED: 'AUTH_EXPIRED',
  INVALID_CREDENTIALS: 'INVALID_CREDENTIALS',

  // 连接相关
  CONNECTION_ERROR: 'CONNECTION_ERROR',
  CONNECTION_TIMEOUT: 'CONNECTION_TIMEOUT',
  NETWORK_UNREACHABLE: 'NETWORK_UNREACHABLE',

  // 同步相关
  SYNC_TIMEOUT: 'SYNC_TIMEOUT',
  SYNC_FAILED: 'SYNC_FAILED',
  SYNC_CANCELLED: 'SYNC_CANCELLED',

  // 服务器相关
  SERVER_ERROR: 'SERVER_ERROR',
  SERVER_UNAVAILABLE: 'SERVER_UNAVAILABLE',
  RATE_LIMIT_EXCEEDED: 'RATE_LIMIT_EXCEEDED',

  // 数据相关
  DATA_CORRUPTED: 'DATA_CORRUPTED',
  PARSE_ERROR: 'PARSE_ERROR',

  // OAuth 相关
  OAUTH_ERROR: 'OAUTH_ERROR',
  OAUTH_CANCELLED: 'OAUTH_CANCELLED',
  TOKEN_EXPIRED: 'TOKEN_EXPIRED',
  REFRESH_FAILED: 'REFRESH_FAILED',

  // 未知错误
  UNKNOWN_ERROR: 'UNKNOWN_ERROR',
} as const

/**
 * 错误代码类型
 */
export type ErrorCode = typeof ERROR_CODES[keyof typeof ERROR_CODES]

/**
 * 用户友好的错误消息映射
 */
const USER_MESSAGES: Record<ErrorCode, string> = {
  [ERROR_CODES.AUTH_FAILED]: '账号认证失败，请检查密码或重新授权',
  [ERROR_CODES.AUTH_EXPIRED]: '账号授权已过期，请重新登录',
  [ERROR_CODES.INVALID_CREDENTIALS]: '账号或密码错误，请检查后重试',

  [ERROR_CODES.CONNECTION_ERROR]: '网络连接失败，请检查网络设置',
  [ERROR_CODES.CONNECTION_TIMEOUT]: '连接超时，请稍后重试',
  [ERROR_CODES.NETWORK_UNREACHABLE]: '网络不可达，请检查网络连接',

  [ERROR_CODES.SYNC_TIMEOUT]: '同步超时，请稍后重试',
  [ERROR_CODES.SYNC_FAILED]: '同步失败，请重试',
  [ERROR_CODES.SYNC_CANCELLED]: '同步已取消',

  [ERROR_CODES.SERVER_ERROR]: '服务器错误，请稍后重试',
  [ERROR_CODES.SERVER_UNAVAILABLE]: '服务暂时不可用，请稍后重试',
  [ERROR_CODES.RATE_LIMIT_EXCEEDED]: '请求过于频繁，请稍后再试',

  [ERROR_CODES.DATA_CORRUPTED]: '数据损坏，请联系技术支持',
  [ERROR_CODES.PARSE_ERROR]: '数据解析失败',

  [ERROR_CODES.OAUTH_ERROR]: 'OAuth 认证失败',
  [ERROR_CODES.OAUTH_CANCELLED]: '用户取消了 OAuth 授权',
  [ERROR_CODES.TOKEN_EXPIRED]: '访问令牌已过期，请重新授权',
  [ERROR_CODES.REFRESH_FAILED]: '刷新令牌失败，请重新登录',

  [ERROR_CODES.UNKNOWN_ERROR]: '未知错误，请重试',
}

/**
 * 错误处理工具类
 */
export class ErrorHandler {
  /**
   * 解析后端错误或原生错误，返回标准化的 MailError
   */
  static parse(error: unknown): MailError {
    // 错误是字符串
    if (typeof error === 'string') {
      return this.parseString(error)
    }

    // 错误是 Error 对象
    if (error instanceof Error) {
      return this.parseError(error)
    }

    // 错误是对象（可能是后端返回的错误对象）
    if (typeof error === 'object' && error !== null) {
      return this.parseObject(error as Record<string, unknown>)
    }

    // 默认未知错误
    return {
      code: ERROR_CODES.UNKNOWN_ERROR,
      message: '未知错误',
      severity: ErrorSeverity.Low,
      retryable: false,
    }
  }

  /**
   * 解析字符串错误
   */
  private static parseString(message: string): MailError {
    if (message.includes('认证失败') || message.includes('登录失败') || message.includes('AUTHENTICATION_FAILED')) {
      return {
        code: ERROR_CODES.AUTH_FAILED,
        message: USER_MESSAGES[ERROR_CODES.AUTH_FAILED],
        severity: ErrorSeverity.High,
        retryable: false,
      }
    }

    if (message.includes('连接') || message.includes('网络') || message.includes('CONNECTION')) {
      return {
        code: ERROR_CODES.CONNECTION_ERROR,
        message: USER_MESSAGES[ERROR_CODES.CONNECTION_ERROR],
        severity: ErrorSeverity.Medium,
        retryable: true,
        retryAfter: 5,
      }
    }

    if (message.includes('超时') || message.includes('TIMEOUT')) {
      return {
        code: ERROR_CODES.CONNECTION_TIMEOUT,
        message: USER_MESSAGES[ERROR_CODES.CONNECTION_TIMEOUT],
        severity: ErrorSeverity.Medium,
        retryable: true,
        retryAfter: 10,
      }
    }

    if (message.includes('账号不存在') || message.includes('NOT_FOUND')) {
      return {
        code: ERROR_CODES.AUTH_FAILED,
        message: '账号不存在，请重新添加',
        severity: ErrorSeverity.High,
        retryable: false,
      }
    }

    if (message.includes('OAuth') || message.includes('oauth')) {
      return {
        code: ERROR_CODES.OAUTH_ERROR,
        message: USER_MESSAGES[ERROR_CODES.OAUTH_ERROR],
        severity: ErrorSeverity.High,
        retryable: false,
      }
    }

    // 默认错误
    return {
      code: ERROR_CODES.UNKNOWN_ERROR,
      message: message || '未知错误',
      severity: ErrorSeverity.Low,
      retryable: false,
    }
  }

  /**
   * 解析 Error 对象
   */
  private static parseError(error: Error): MailError {
    const result = this.parseString(error.message)

    // 附加堆栈信息到 details
    if (error.stack) {
      result.details = {
        ...result.details,
        stack: error.stack,
      }
    }

    return result
  }

  /**
   * 解析对象类型的错误
   */
  private static parseObject(obj: Record<string, unknown>): MailError {
    // 尝试提取错误码和消息
    const code = (obj.code as string) || (obj.errorCode as string)
    const message = (obj.message as string) || (obj.error as string) || '未知错误'

    if (code) {
      return {
        code,
        message: USER_MESSAGES[code as ErrorCode] || message,
        severity: (obj.severity as ErrorSeverity) || ErrorSeverity.Medium,
        retryable: (obj.retryable as boolean) ?? false,
        retryAfter: obj.retryAfter as number | undefined,
        details: obj,
      }
    }

    return this.parseString(message)
  }

  /**
   * 获取用户友好的错误提示
   */
  static getUserMessage(error: MailError): string {
    return USER_MESSAGES[error.code as ErrorCode] || error.message
  }

  /**
   * 判断是否需要重试
   */
  static shouldRetry(error: MailError): boolean {
    return error.retryable && error.severity !== ErrorSeverity.Critical
  }

  /**
   * 获取重试延迟时间（秒）
   */
  static getRetryDelay(error: MailError): number {
    return error.retryAfter || 5
  }

  /**
   * 创建特定类型的错误
   */
  static createError(
    code: ErrorCode,
    message?: string,
    severity: ErrorSeverity = ErrorSeverity.Medium,
    retryable: boolean = false,
    retryAfter?: number
  ): MailError {
    return {
      code,
      message: message || USER_MESSAGES[code],
      severity,
      retryable,
      retryAfter,
    }
  }

  /**
   * 创建认证错误
   */
  static authError(message?: string): MailError {
    return this.createError(ERROR_CODES.AUTH_FAILED, message, ErrorSeverity.High, false)
  }

  /**
   * 创建连接错误
   */
  static connectionError(retryAfter: number = 5): MailError {
    return this.createError(ERROR_CODES.CONNECTION_ERROR, undefined, ErrorSeverity.Medium, true, retryAfter)
  }

  /**
   * 创建超时错误
   */
  static timeoutError(): MailError {
    return this.createError(ERROR_CODES.CONNECTION_TIMEOUT, undefined, ErrorSeverity.Medium, true, 10)
  }

  /**
   * 格式化错误消息用于显示
   */
  static formatForDisplay(error: MailError): string {
    const userMessage = this.getUserMessage(error)
    const suggestion = this.getSuggestion(error)

    if (suggestion) {
      return `${userMessage}\n\n建议：${suggestion}`
    }

    return userMessage
  }

  /**
   * 获取错误解决建议
   */
  static getSuggestion(error: MailError): string | null {
    const suggestions: Record<ErrorCode, string | null> = {
      [ERROR_CODES.AUTH_FAILED]: '请检查账号密码是否正确，或使用 OAuth 重新授权',
      [ERROR_CODES.AUTH_EXPIRED]: '请重新登录以获取新的访问令牌',
      [ERROR_CODES.INVALID_CREDENTIALS]: '请检查账号和密码是否正确',

      [ERROR_CODES.CONNECTION_ERROR]: '请检查：1) 网络连接是否正常 2) 邮件服务器地址是否正确 3) 防火墙设置',
      [ERROR_CODES.CONNECTION_TIMEOUT]: '请检查网络连接或稍后重试',
      [ERROR_CODES.NETWORK_UNREACHABLE]: '请检查网络连接',

      [ERROR_CODES.SYNC_TIMEOUT]: '请稍后重试，或减少单次同步的邮件数量',
      [ERROR_CODES.SYNC_FAILED]: '请检查网络连接后重试',
      [ERROR_CODES.SYNC_CANCELLED]: '如需同步，请重新触发同步',

      [ERROR_CODES.SERVER_ERROR]: '请稍后重试',
      [ERROR_CODES.SERVER_UNAVAILABLE]: '请稍后重试，或联系邮件服务提供商',
      [ERROR_CODES.RATE_LIMIT_EXCEEDED]: '请稍后再试，或减少同步频率',

      [ERROR_CODES.DATA_CORRUPTED]: '请删除账号后重新添加',
      [ERROR_CODES.PARSE_ERROR]: '请检查邮件服务器配置',

      [ERROR_CODES.OAUTH_ERROR]: '请重新进行 OAuth 授权',
      [ERROR_CODES.OAUTH_CANCELLED]: '如需使用，请重新进行授权',
      [ERROR_CODES.TOKEN_EXPIRED]: '请重新进行 OAuth 授权',
      [ERROR_CODES.REFRESH_FAILED]: '请重新登录',

      [ERROR_CODES.UNKNOWN_ERROR]: '请重试，如问题持续请联系技术支持',
    }

    return suggestions[error.code as ErrorCode]
  }
}

/**
 * 默认导出
 */
export default ErrorHandler
