// ============================================================
// 枚举类型定义
// ============================================================

/**
 * 账号类型枚举
 */
export enum AccountType {
  Personal = 'personal',
  Enterprise = 'enterprise',
}

/**
 * 认证类型枚举
 */
export enum AuthType {
  Password = 'password',
  OAuth2 = 'oauth2',
  AppPassword = 'app_password',
  DomainAuth = 'domain_auth',
  SamlSso = 'saml_sso',
}

/**
 * SSL 模式枚举
 */
export enum SslMode {
  None = 'none',
  StartTls = 'start_tls',
  Implicit = 'implicit',
}

/**
 * 同步阶段枚举
 */
export enum SyncStage {
  Connecting = 'connecting',
  SyncingFolders = 'syncing_folders',
  SyncingEmails = 'syncing_emails',
  Completed = 'completed',
  Error = 'error',
}

/**
 * 错误严重程度枚举
 */
export enum ErrorSeverity {
  Low = 'low',
  Medium = 'medium',
  High = 'high',
  Critical = 'critical',
}

// ============================================================
// 邮件相关类型
// ============================================================

export interface Email {
  id: string;
  sender: string;
  senderEmail: string;
  recipient: string;
  subject: string;
  preview: string;
  body: string;
  date: Date;
  unread: boolean;
  starred: boolean;
  labels: string[];
  folder: EmailFolder;
  attachments: Attachment[];
  accountId: string;
}

export type EmailFolder =
  | "inbox"
  | "starred"
  | "sent"
  | "drafts"
  | "spam"
  | "trash"
  | "archive";

export interface Attachment {
  name: string;
  size: string;
  path?: string;
}

// ============================================================
// 账号相关类型
// ============================================================

export interface Account {
  id: string;
  name: string;
  email: string;
  provider: EmailProvider;
  color: string;
  unreadCount: number;

  // 账号类型和认证类型
  accountType: AccountType;
  authType: AuthType;

  // IMAP/SMTP 配置
  imapHost?: string;
  imapPort?: number;
  imapSsl?: boolean;
  smtpHost?: string;
  smtpPort?: number;
  smtpSsl?: boolean;

  // 同步配置
  syncEnabled?: boolean;
  lastSyncAt?: Date;

  // OAuth 相关
  oauthProvider?: string;
  oauthTokenExpiry?: Date;

  // 企业邮箱配置
  enterpriseTenantId?: string;
  enterpriseDomain?: string;

  // 时间戳
  createdAt: Date;
  updatedAt: Date;
}

export type EmailProvider = "gmail" | "outlook" | "icloud" | "yahoo" | "imap";

// ============================================================
// 同步相关类型
// ============================================================

/**
 * 同步进度接口
 */
export interface SyncProgress {
  accountId: number;
  stage: SyncStage;
  folder?: string;
  current: number;
  total: number;
  message: string;
  startedAt: Date;
}

/**
 * 同步状态接口
 */
export interface SyncStatus {
  accountId: number;
  stage: 'idle' | 'syncing' | 'completed' | 'error';
  progress: number;
  message: string;
  currentFolder?: string;
  result?: SyncResult;
  error?: string;
}

/**
 * 同步结果接口
 */
export interface SyncResult {
  totalSynced: number;
  foldersSynced: number;
  errors: number;
  durationMs: number;
}

/**
 * 同步历史记录项
 */
export interface SyncHistoryItem {
  accountId: number;
  completedAt: Date;
  result: SyncResult;
}

// ============================================================
// 错误处理类型
// ============================================================

/**
 * 邮件错误接口
 */
export interface MailError {
  code: string;
  message: string;
  severity: ErrorSeverity;
  retryable: boolean;
  retryAfter?: number;
  details?: Record<string, unknown>;
}

// ============================================================
// FlowEngine 相关类型
// ============================================================

/**
 * FlowEngine 状态接口
 */
export interface FlowEngineStatus {
  isRunning: boolean;
  activeTasks: number;
  queuedTasks: number;
  completedTasks: number;
  failedTasks: number;
}

// ============================================================
// 日历相关类型
// ============================================================
export interface CalendarEvent {
  id: string;
  title: string;
  date: Date;
  startTime: string;
  endTime: string;
  repeat: RepeatType;
  repeatEnd?: string;
  color: string;
  notes: string;
}

export type RepeatType = "none" | "daily" | "weekly" | "monthly" | "yearly";

// 工作流相关类型
export interface WorkflowNode {
  id: string;
  type: NodeType;
  label: string;
  position: { x: number; y: number };
  config: Record<string, unknown>;
}

export type NodeType =
  | "trigger"
  | "condition"
  | "action"
  | "email"
  | "delay"
  | "ai";

// AI 对话相关类型
export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  timestamp: Date;
  pendingAction?: PendingAction;
  actionResult?: ActionResult;
}

export interface PendingAction {
  type: string;
  description: string;
  affectedCount: number;
  emailIds: string[];
}

export interface ActionResult {
  success: number;
  failed: number;
}

// Toast 通知类型
export interface Toast {
  id: number;
  message: string;
  type: "success" | "error" | "info";
}

// 导航项类型
export interface NavItem {
  id: string;
  label: string;
  icon: string;
  count?: number;
}

// 标签类型
export interface Label {
  id: string;
  name: string;
  color: string;
}

// 写信表单类型
export interface ComposeForm {
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  accountId?: string;
}

// 设置类型
export interface Settings {
  theme: "light" | "dark" | "system";
  language: string;
  notifications: NotificationSettings;
  ai: AISettings;
}

export interface NotificationSettings {
  enabled: boolean;
  sound: boolean;
  desktop: boolean;
}

export interface AISettings {
  provider: "openai" | "anthropic" | "ollama";
  apiKey?: string;
  model: string;
}

// API 响应类型
export interface ApiResponse<T> {
  data: T;
  total?: number;
  error?: string;
}
