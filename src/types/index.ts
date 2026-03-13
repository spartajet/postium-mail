// 邮件相关类型
export interface Email {
  id: string
  sender: string
  senderEmail: string
  recipient: string
  subject: string
  preview: string
  body: string
  date: Date
  unread: boolean
  starred: boolean
  labels: string[]
  folder: EmailFolder
  attachments: Attachment[]
  accountId: string
}

export type EmailFolder = 'inbox' | 'starred' | 'sent' | 'drafts' | 'spam' | 'trash'

export interface Attachment {
  name: string
  size: string
  path?: string
}

// 账号相关类型
export interface Account {
  id: string
  name: string
  email: string
  provider: EmailProvider
  color: string
  unreadCount: number
  imapHost?: string
  imapPort?: number
  smtpHost?: string
  smtpPort?: number
}

export type EmailProvider = 'gmail' | 'outlook' | 'icloud' | 'yahoo' | 'imap'

// 日历相关类型
export interface CalendarEvent {
  id: string
  title: string
  date: Date
  startTime: string
  endTime: string
  repeat: RepeatType
  repeatEnd?: string
  color: string
  notes: string
}

export type RepeatType = 'none' | 'daily' | 'weekly' | 'monthly' | 'yearly'

// 工作流相关类型
export interface WorkflowNode {
  id: string
  type: NodeType
  label: string
  position: { x: number; y: number }
  config: Record<string, unknown>
}

export type NodeType = 'trigger' | 'condition' | 'action' | 'email' | 'delay' | 'ai'

// AI 对话相关类型
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: Date
  pendingAction?: PendingAction
  actionResult?: ActionResult
}

export interface PendingAction {
  type: string
  description: string
  affectedCount: number
  emailIds: string[]
}

export interface ActionResult {
  success: number
  failed: number
}

// Toast 通知类型
export interface Toast {
  id: number
  message: string
  type: 'success' | 'error' | 'info'
}

// 导航项类型
export interface NavItem {
  id: string
  label: string
  icon: string
  count?: number
}

// 标签类型
export interface Label {
  id: string
  name: string
  color: string
}

// 写信表单类型
export interface ComposeForm {
  to: string
  cc: string
  bcc: string
  subject: string
  body: string
  accountId?: string
}

// 设置类型
export interface Settings {
  theme: 'light' | 'dark' | 'system'
  language: string
  notifications: NotificationSettings
  ai: AISettings
}

export interface NotificationSettings {
  enabled: boolean
  sound: boolean
  desktop: boolean
}

export interface AISettings {
  provider: 'openai' | 'anthropic' | 'ollama'
  apiKey?: string
  model: string
}

// API 响应类型
export interface ApiResponse<T> {
  data: T
  total?: number
  error?: string
}
