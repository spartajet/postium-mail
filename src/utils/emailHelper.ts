/**
 * 邮箱相关工具函数
 * 用于自动判断服务商、认证方式和服务器配置
 */

// 已知服务商的域名映射
const PROVIDER_DOMAIN_MAP: Record<string, string> = {
  'gmail.com': 'gmail',
  'googlemail.com': 'gmail', // Gmail 的别称
  'outlook.com': 'outlook',
  'hotmail.com': 'outlook',
  'live.com': 'outlook',
  'yahoo.com': 'yahoo',
  'yahoomail.com': 'yahoo',
  'icloud.com': 'icloud',
  'me.com': 'icloud',
  'mac.com': 'icloud',
}

/**
 * 解析邮箱地址，提取域名和用户信息
 */
export function parseEmail(email: string): {
  domain: string | null
  localPart: string | null
  isValid: boolean
} {
  if (!email || typeof email !== 'string') {
    return { domain: null, localPart: null, isValid: false }
  }

  const trimmed = email.trim()

  // 基本的邮箱格式验证
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
  if (!emailRegex.test(trimmed)) {
    return { domain: null, localPart: null, isValid: false }
  }

  const [localPart, ...domainParts] = trimmed.split('@')
  const domain = domainParts.join('@').toLowerCase()

  return {
    domain,
    localPart,
    isValid: true,
  }
}

/**
 * 根据邮箱域名判断服务商
 */
export function detectProviderFromEmail(email: string): string | null {
  const { domain, isValid } = parseEmail(email)

  if (!isValid || !domain) {
    return null
  }

  // 直接匹配
  if (PROVIDER_DOMAIN_MAP[domain]) {
    return PROVIDER_DOMAIN_MAP[domain]
  }

  // 检查是否是子域名（例如 mail.google.com）
  for (const [knownDomain, provider] of Object.entries(PROVIDER_DOMAIN_MAP)) {
    if (domain.endsWith(`.${knownDomain}`)) {
      return provider
    }
  }

  return null // 未知服务商，使用自定义 IMAP
}

/**
 * 根据服务商判断推荐的认证方式
 * Gmail/Outlook/Yahoo → oauth，其他 → password
 */
export function getRecommendedAuthType(provider: string): 'password' | 'oauth' {
  return (provider === 'gmail' || provider === 'outlook' || provider === 'yahoo')
    ? 'oauth'
    : 'password'
}

/**
 * 生成默认账号名称
 */
export function generateDefaultAccountName(email: string): string {
  const { localPart, isValid } = parseEmail(email)

  if (!isValid || !localPart) {
    return ''
  }

  // 如果邮箱地址本身很短，直接使用
  if (email.length <= 30) {
    return email
  }

  // 否则使用 localPart
  return localPart
}

/**
 * 根据域名生成 IMAP/SMTP 服务器地址
 */
export function generateServerConfig(domain: string): {
  imapHost: string
  smtpHost: string
  imapPort: number
  smtpPort: number
  imapSsl: boolean
  smtpSsl: boolean
} {
  return {
    imapHost: `imap.${domain}`,
    smtpHost: `smtp.${domain}`,
    imapPort: 993,    // IMAP SSL 默认端口
    smtpPort: 465,    // SMTP SSL 默认端口
    imapSsl: true,    // 默认使用 SSL
    smtpSsl: true,    // 默认使用 SSL
  }
}

/**
 * 从邮箱地址提取所有可自动填充的信息
 */
export function extractAutoFillInfo(email: string, currentName: string): {
  provider: string
  name: string
  authType: 'password' | 'oauth'
  serverConfig: {
    imapHost: string
    smtpHost: string
    imapPort: number
    smtpPort: number
    imapSsl: boolean
    smtpSsl: boolean
  } | null
  isCustom: boolean
} {
  const { domain, isValid } = parseEmail(email)

  if (!isValid || !domain) {
    return {
      provider: 'gmail',
      name: currentName,
      authType: 'oauth',
      serverConfig: null,
      isCustom: false,
    }
  }

  const detectedProvider = detectProviderFromEmail(email)

  // 判断认证方式
  const authType: 'password' | 'oauth' =
    (detectedProvider === 'gmail' || detectedProvider === 'outlook' || detectedProvider === 'yahoo')
      ? 'oauth'
      : 'password'

  // 只有当当前名称为空或者是默认 placeholder 时才自动填充
  const shouldAutoFillName = !currentName || currentName === '工作邮箱'
  const autoFilledName = shouldAutoFillName ? generateDefaultAccountName(email) : currentName

  if (detectedProvider) {
    // 已知服务商
    return {
      provider: detectedProvider,
      name: autoFilledName,
      authType,
      serverConfig: null, // 使用后端预设
      isCustom: false,
    }
  } else {
    // 自定义域名
    return {
      provider: 'imap',
      name: autoFilledName,
      authType: 'password',
      serverConfig: generateServerConfig(domain),
      isCustom: true,
    }
  }
}
