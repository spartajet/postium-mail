/**
 * Postium Mail - 英文（美国）翻译文件
 * en-US.ts
 *
 * 本文件定义了应用界面中所有英文文本的翻译映射。
 * 使用 `as const` 断言确保所有键值在编译时进行类型检查，
 * 保证与其他语言翻译文件的结构一致性。
 *
 * ==================== 翻译结构说明 ====================
 * 翻译文本按功能模块分组，主要包含以下部分：
 * - app：应用全局信息（应用名称等）
 * - sidebar：侧边栏导航项和标签
 * - email：邮件相关的界面文本（搜索、操作按钮、字段标签等）
 * - account：账号管理相关的界面文本（添加、编辑、删除、配置等）
 * - sync：同步功能相关的界面文本
 * - settings：设置页面的界面文本
 * - ai：AI 功能相关的界面文本（摘要、智能回复、翻译等）
 * - common：通用操作文本（确认、取消、保存、关闭等）
 *
 * ==================== 维护说明 ====================
 * 1. 修改翻译时，需同步更新 zh-CN.ts 文件，保持两端结构一致
 * 2. 新增翻译键时，必须在所有语言文件中添加对应条目
 * 3. 键名使用驼峰命名法（camelCase）
 * 4. 模块名使用驼峰命名法，与功能模块名称对应
 *
 * ==================== 使用方式 ====================
 * 此文件由 i18n.svelte.ts 中的 I18nState 导入并注册，
 * 切换语言到 'en-US' 时，会自动使用此文件中的翻译文本。
 */
export default {
  app: { name: "Postium Mail" },
  sidebar: {
    inbox: "Inbox",
    starred: "Starred",
    sent: "Sent",
    drafts: "Drafts",
    spam: "Spam",
    trash: "Trash",
    archive: "Archive",
    compose: "Compose",
    calendar: "Calendar",
    workflow: "Workflow",
    labels: "Labels",
    storage: "Storage",
    settings: "Settings",
    sync: "Sync",
    allAccounts: "All Accounts",
    labelUrgent: "Urgent",
    labelWork: "Work",
    labelPersonal: "Personal",
    labelFinance: "Finance",
  },
  email: {
    search: "Search emails...",
    noEmailSelected: "Select an email to read",
    noEmails: "No emails",
    markRead: "Mark as read",
    markUnread: "Mark as unread",
    reload: "Reload",
    star: "Star",
    unstar: "Unstar",
    delete: "Delete",
    archive: "Archive",
    move: "Move to",
    reply: "Reply",
    replyAll: "Reply all",
    forward: "Forward",
    send: "Send",
    to: "To",
    otherRecipients: "and {count} other recipients",
    expandRecipients: "Expand recipients",
    collapseRecipients: "Collapse recipients",
    cc: "CC",
    bcc: "BCC",
    subject: "Subject",
    from: "From",
    date: "Date",
    attachments: "Attachments",
    attachmentDownload: "Download",
    attachmentOpen: "Open",
    attachmentSaveAs: "Save as",
    attachmentSave: "Save",
    attachmentDownloading: "Downloading",
    attachmentCached: "Cached",
    attachmentNotDownloaded: "Not downloaded",
    attachmentSaveCanceled: "Save canceled",
    loading: "Loading...",
    loadMore: "Load more",
    syncOlder: "Load older mail",
    syncingOlder: "Loading older mail...",
    emailCount: "emails",
  },
  account: {
    add: "Add Account",
    edit: "Edit",
    delete: "Delete",
    name: "Account Name",
    email: "Email Address",
    password: "Password",
    provider: "Email Provider",
    authType: "Auth Type",
    detecting: "Detecting provider...",
    detected: "Detected",
    notDetected: "Not recognized, configure manually",
    selectProvider: "Select Email Provider",
    selectProviderHint: "Choose your email provider, or configure manually",
    manualConfig: "Manual Configuration",
    manualConfigHint: "Enter IMAP/SMTP server details",
    imapHost: "IMAP Server",
    imapPort: "IMAP Port",
    smtpHost: "SMTP Server",
    smtpPort: "SMTP Port",
    displayName: "Display Name",
    other: "Other",
    otherHint: "Enter server details manually",
    step1: "Select Provider",
    step2: "Enter Credentials",
    step3: "Done",
    testing: "Testing connection...",
    testSuccess: "Connection successful",
    testFailed: "Connection failed",
    addAnother: "Add Another",
    syncScopeTitle: "Choose mail sync range",
    syncScopeHint: "You can sync older mail later from the bottom of the mail list.",
    syncRangeWeek: "Last 1 week",
    syncRangeMonth: "Last 1 month",
    syncRangeThreeMonths: "Last 3 months",
    syncRangeYear: "Last 1 year",
    syncRangeAll: "All mail",
    syncRangeAllHint: "All mail may take longer.",
    startSync: "Start sync",
    oauthAccountMissing:
      "OAuth2 authorization completed, but account {email} could not be found to choose a sync range. Refresh the account list and try again.",
    initialSyncFailed: "Account added, but initial sync failed: {error}",
    initialSyncStarted: "Account added. Initial sync has started.",
  },
  sync: {
    syncing: "Syncing...",
    completed: "Sync completed",
    failed: "Sync failed",
    lastSync: "Last sync",
    never: "Never",
    connecting: "Connecting...",
    syncingFolders: "Syncing folders...",
    syncingEmails: "Syncing emails...",
  },
  settings: {
    title: "Settings",
    general: "General",
    accounts: "Accounts",
    appearance: "Appearance",
    language: "Language",
    theme: "Theme",
    light: "Light",
    dark: "Dark",
    system: "System",
  },
  ai: {
    summary: "AI Summary",
    smartReply: "Smart Reply",
    translate: "Translate",
    tasks: "Extract Tasks",
    provider: "Assistant",
    generating: "AI generating...",
    noSummary: "No summary yet",
  },
  common: {
    confirm: "Confirm",
    cancel: "Cancel",
    save: "Save",
    close: "Close",
    loading: "Loading...",
    error: "Error",
    retry: "Retry",
    delete: "Delete",
    operations: "Operations",
  },
} as const;
