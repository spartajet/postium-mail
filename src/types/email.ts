// 邮件账户类型
export interface EmailAccount {
  id: string;
  email: string;
  name: string;
  provider: "gmail" | "outlook" | "yahoo" | "icloud" | "qq" | "163" | "custom";
  imapHost?: string;
  imapPort?: number;
  imapSecurity?: "none" | "tls" | "ssl";
  smtpHost?: string;
  smtpPort?: number;
  smtpSecurity?: "none" | "tls" | "ssl";
  username?: string;
  password?: string; // 应该加密存储
  isDefault?: boolean;
  isActive: boolean;
  color?: string;
  avatar?: string;
  signature?: string;
  syncInterval?: number; // 同步间隔（分钟）
  lastSyncTime?: Date | string;
  folders?: FolderInfo[];
  unreadCount?: number;
  totalCount?: number;
  quotaUsed?: number;
  quotaTotal?: number;
}

// 邮件附件类型
export interface Attachment {
  id: string;
  filename: string;
  size: number;
  mimeType: string;
  url?: string;
  content?: string;
  contentId?: string; // 用于内嵌图片
  isInline?: boolean;
}

// 邮件联系人类型
export interface Contact {
  email: string;
  name?: string;
  avatar?: string;
}

// 邮件类型
export interface Email {
  id: string;
  messageId: string;
  accountId: string;
  folder: EmailFolder;
  from: Contact;
  to: Contact[];
  cc?: Contact[];
  bcc?: Contact[];
  replyTo?: Contact;
  subject: string;
  body: string;
  bodyHtml?: string;
  bodyText?: string;
  preview?: string;
  date: Date | string;
  receivedDate?: Date | string;
  isRead: boolean;
  isStarred: boolean;
  isFlagged: boolean;
  isDraft: boolean;
  isImportant: boolean;
  isDeleted: boolean;
  hasAttachments: boolean;
  attachments?: Attachment[];
  labels?: Label[];
  categories?: string[];
  threadId?: string;
  conversationId?: string;
  inReplyTo?: string;
  references?: string[];
  headers?: Record<string, string>;
  size?: number;
  priority?: "low" | "normal" | "high";
}

// 标签类型
export interface Label {
  id: string;
  name: string;
  color: string;
  accountId: string;
  isSystemLabel?: boolean;
}

// 邮件文件夹类型
export type EmailFolder =
  | "inbox"
  | "sent"
  | "drafts"
  | "trash"
  | "spam"
  | "starred"
  | "important"
  | "archive"
  | "all"
  | string; // 支持自定义文件夹

// 邮件文件夹信息
export interface FolderInfo {
  id: string;
  name: string;
  type: EmailFolder;
  icon?: string;
  count: number;
  unreadCount: number;
  color?: string;
  parentId?: string;
  children?: FolderInfo[];
  isSystem?: boolean;
  isCollapsed?: boolean;
  accountId: string;
  syncState?: "idle" | "syncing" | "error";
  lastSyncTime?: Date | string;
}

// 邮件线程/会话
export interface EmailThread {
  id: string;
  emails: Email[];
  subject: string;
  participants: Contact[];
  lastMessageDate: Date | string;
  firstMessageDate: Date | string;
  unreadCount: number;
  totalCount: number;
  hasAttachments: boolean;
  isImportant: boolean;
  isStarred: boolean;
  labels?: Label[];
  preview?: string;
}

// 邮件过滤器
export interface EmailFilter {
  folder?: EmailFolder;
  accountId?: string;
  accountIds?: string[]; // 支持多账户筛选
  isRead?: boolean;
  isStarred?: boolean;
  isImportant?: boolean;
  isFlagged?: boolean;
  hasAttachments?: boolean;
  searchTerm?: string;
  dateFrom?: Date | string;
  dateTo?: Date | string;
  from?: string;
  to?: string;
  subject?: string;
  labels?: string[];
  categories?: string[];
  sizeMin?: number;
  sizeMax?: number;
}

// 邮件排序选项
export type EmailSortBy = "date" | "subject" | "from" | "size" | "importance";
export type SortOrder = "asc" | "desc";

export interface EmailSort {
  by: EmailSortBy;
  order: SortOrder;
}

// 邮件操作
export type EmailAction =
  | "reply"
  | "replyAll"
  | "forward"
  | "delete"
  | "archive"
  | "spam"
  | "markAsRead"
  | "markAsUnread"
  | "star"
  | "unstar"
  | "flag"
  | "unflag"
  | "markAsImportant"
  | "unmarkAsImportant"
  | "moveToFolder"
  | "copyToFolder"
  | "addLabel"
  | "removeLabel"
  | "print"
  | "download";

// 撰写邮件的草稿
export interface ComposeDraft {
  id?: string;
  accountId: string;
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  body: string;
  bodyHtml?: string;
  attachments?: Attachment[];
  inReplyTo?: string;
  references?: string[];
  isDraft: boolean;
  priority?: "low" | "normal" | "high";
  requestReadReceipt?: boolean;
  requestDeliveryReceipt?: boolean;
  sendLater?: Date | string;
  signature?: string;
}

// 邮件同步状态
export interface SyncStatus {
  accountId: string;
  folder: EmailFolder;
  isSync: boolean;
  lastSyncTime?: Date | string;
  nextSyncTime?: Date | string;
  progress?: number;
  error?: string;
  newEmails?: number;
  updatedEmails?: number;
  deletedEmails?: number;
}

// 邮件规则
export interface EmailRule {
  id: string;
  name: string;
  accountId: string;
  isEnabled: boolean;
  conditions: RuleCondition[];
  actions: RuleAction[];
  priority: number;
}

export interface RuleCondition {
  field: "from" | "to" | "subject" | "body" | "hasAttachment";
  operator: "contains" | "equals" | "startsWith" | "endsWith" | "notContains";
  value: string;
}

export interface RuleAction {
  type:
    | "moveToFolder"
    | "addLabel"
    | "markAsRead"
    | "star"
    | "delete"
    | "forward";
  value?: string;
}

// 邮件统计
export interface EmailStatistics {
  accountId: string;
  totalEmails: number;
  unreadEmails: number;
  sentEmails: number;
  receivedEmails: number;
  draftEmails: number;
  trashedEmails: number;
  spamEmails: number;
  starredEmails: number;
  importantEmails: number;
  totalSize: number;
  averageResponseTime?: number;
  topSenders?: Contact[];
  topRecipients?: Contact[];
  emailsByHour?: Record<number, number>;
  emailsByDay?: Record<string, number>;
}

// 快速回复模板
export interface QuickReplyTemplate {
  id: string;
  name: string;
  content: string;
  accountId?: string;
  isGlobal: boolean;
  shortcut?: string;
}

// 邮件签名
export interface EmailSignature {
  id: string;
  name: string;
  content: string;
  contentHtml?: string;
  accountId: string;
  isDefault: boolean;
  applyToReplies: boolean;
  applyToForwards: boolean;
}
