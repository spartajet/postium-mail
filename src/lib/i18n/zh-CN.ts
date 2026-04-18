/**
 * Postium Mail - 简体中文（zh-CN）翻译文件
 * zh-CN.ts
 *
 * 本文件定义了应用界面中所有简体中文的翻译文本。
 * 使用 `as const` 断言确保所有键名在编译时进行类型检查，
 * 保证各语言翻译文件的结构一致性。
 *
 * ==================== 文件结构 ====================
 * 翻译文本按功能模块分组，主要包含以下部分：
 * - app：应用级别的通用文本（应用名称等）
 * - sidebar：侧边栏相关文本（文件夹名称、操作按钮等）
 * - email：邮件相关文本（搜索、操作、字段标签等）
 * - account：账号管理相关文本（表单标签、提示信息等）
 * - sync：同步相关文本（同步状态、进度提示等）
 * - settings：设置页面相关文本（分类标题、选项名称等）
 * - ai：AI 功能相关文本（摘要、翻译、智能回复等）
 * - common：通用操作文本（确认、取消、保存等）
 *
 * ==================== 维护说明 ====================
 * 1. 新增翻译键时，需同时在 en-US.ts 中添加对应的英文翻译
 * 2. 键名使用驼峰命名法（camelCase）
 * 3. 嵌套层级不宜过深，建议最多 2 层
 * 4. 修改或删除键名时，需搜索全局确保没有遗漏的引用
 *
 * ==================== 使用方式 ====================
 * ```typescript
 * import { getI18nState } from '$lib/stores/i18n.svelte';
 * const i18n = getI18nState();
 *
 * // 访问翻译文本
 * console.log(i18n.t.sidebar.inbox); // '收件箱'
 * console.log(i18n.t.email.search);  // '搜索邮件...'
 * ```
 */
export default {
  app: { name: "Postium Mail" },
  sidebar: {
    inbox: "收件箱",
    starred: "星标邮件",
    sent: "已发送",
    drafts: "草稿",
    spam: "垃圾邮件",
    trash: "回收站",
    archive: "归档",
    compose: "写邮件",
    calendar: "日历",
    workflow: "工作流",
    labels: "标签",
    storage: "存储",
    settings: "设置",
    sync: "同步",
    allAccounts: "全部账号",
    labelUrgent: "紧急",
    labelWork: "工作",
    labelPersonal: "个人",
    labelFinance: "财务",
  },
  email: {
    search: "搜索邮件...",
    noEmailSelected: "选择一封邮件开始阅读",
    noEmails: "没有邮件",
    markRead: "标记已读",
    markUnread: "标记未读",
    star: "星标",
    unstar: "取消星标",
    delete: "删除",
    archive: "归档",
    move: "移动到",
    reply: "回复",
    replyAll: "全部回复",
    forward: "转发",
    send: "发送",
    to: "收件人",
    cc: "抄送",
    bcc: "密送",
    subject: "主题",
    from: "发件人",
    date: "日期",
    attachments: "附件",
    loading: "加载中...",
    emailCount: "封邮件",
  },
  account: {
    add: "添加账号",
    edit: "编辑",
    delete: "删除",
    name: "账号名称",
    email: "邮箱地址",
    password: "密码",
    provider: "邮件服务商",
    authType: "认证方式",
    detecting: "检测服务商...",
    detected: "检测到",
    notDetected: "未识别，请手动配置",
    selectProvider: "选择邮件服务商",
    selectProviderHint: "选择你的邮箱服务商，或手动配置",
    manualConfig: "手动配置",
    manualConfigHint: "输入 IMAP/SMTP 服务器信息",
    imapHost: "IMAP 服务器",
    imapPort: "IMAP 端口",
    smtpHost: "SMTP 服务器",
    smtpPort: "SMTP 端口",
    displayName: "显示名称",
    other: "其他",
    otherHint: "手动输入服务器信息",
    step1: "选择服务商",
    step2: "输入凭据",
    step3: "完成",
    testing: "正在测试连接...",
    testSuccess: "连接成功",
    testFailed: "连接失败",
    addAnother: "继续添加",
  },
  sync: {
    syncing: "同步中...",
    completed: "同步完成",
    failed: "同步失败",
    lastSync: "上次同步",
    never: "从未同步",
    connecting: "连接中...",
    syncingFolders: "同步文件夹...",
    syncingEmails: "同步邮件...",
  },
  settings: {
    title: "设置",
    general: "通用",
    accounts: "账号管理",
    appearance: "外观",
    language: "语言",
    theme: "主题",
    light: "浅色",
    dark: "深色",
    system: "跟随系统",
  },
  ai: {
    summary: "AI 摘要",
    smartReply: "智能回复",
    translate: "翻译",
    tasks: "提取任务",
    provider: "助手",
    generating: "AI 生成中...",
    noSummary: "暂无摘要",
  },
  common: {
    confirm: "确认",
    cancel: "取消",
    save: "保存",
    close: "关闭",
    loading: "加载中...",
    error: "出错了",
    retry: "重试",
    delete: "删除",
    operations: "操作",
  },
} as const;
