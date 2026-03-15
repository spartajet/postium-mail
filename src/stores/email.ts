import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Email, EmailFolder } from '@/types'
import { invoke } from '@tauri-apps/api/core'
import { useAccountStore } from './account'

// 后端 DTO 类型定义
interface EmailDto {
  id: number
  account_id: number
  folder: string
  uid: number | null
  message_id: string | null
  subject: string | null
  sender_name: string | null
  sender_email: string
  recipient_emails: string  // JSON 数组
  cc_emails: string | null
  bcc_emails: string | null
  body_text: string | null
  body_html: string | null
  is_read: boolean
  is_starred: boolean
  is_draft: boolean
  sent_at: number
  received_at: number
  created_at: number
  updated_at: number
}

interface EmailListItemDto {
  id: number
  account_id: number
  folder: string
  subject: string | null
  sender_name: string | null
  sender_email: string
  snippet: string | null
  has_attachment: boolean
  attachment_count: number
  is_read: boolean
  is_starred: boolean
  is_draft: boolean
  sent_at: number
  received_at: number
}

interface EmailListResponse {
  emails: EmailListItemDto[]
  total: number
  page: number
  page_size: number
}

interface EmailDetailDto extends EmailDto {
  recipients: Array<{ name: string | null; email: string }>
  cc: Array<{ name: string | null; email: string }>
  bcc: Array<{ name: string | null; email: string }>
  attachments: Array<{
    id: number
    filename: string
    content_type: string
    size: number
    path: string | null
  }>
}

// DTO 转换为前端 Email 类型
function dtoToEmail(dto: EmailListItemDto): Email {
  // 统一文件夹名称为小写（兼容旧数据的大写格式）
  const normalizedFolder = normalizeFolderName(dto.folder)

  return {
    id: dto.id.toString(),
    subject: dto.subject || '无主题',
    sender: dto.sender_name || dto.sender_email,
    senderEmail: dto.sender_email,
    recipient: dto.sender_email, // 简化：使用发件人邮箱作为收件人
    preview: dto.snippet || '',
    body: '',
    date: new Date(dto.received_at * 1000),
    unread: !dto.is_read,
    starred: dto.is_starred,
    labels: [],
    attachments: dto.has_attachment && dto.attachment_count > 0 ? [{
      name: `${dto.attachment_count} 个附件`,
      size: '',
      path: '',
    }] : [],
    folder: normalizedFolder,
    accountId: dto.account_id.toString(),
  }
}

function detailDtoToEmail(dto: EmailDetailDto): Email {
  // 统一文件夹名称为小写（兼容旧数据的大写格式）
  const normalizedFolder = normalizeFolderName(dto.folder)
  return {
    id: dto.id.toString(),
    subject: dto.subject || '无主题',
    sender: dto.sender_name || dto.sender_email,
    senderEmail: dto.sender_email,
    recipient: dto.recipients.map(r => r.email).join(', '),
    preview: '', // 详情页不需要预览
    body: dto.body_html || dto.body_text || '',
    date: new Date(dto.received_at * 1000),
    unread: !dto.is_read,
    starred: dto.is_starred,
    labels: [],
    attachments: dto.attachments.map(a => ({
      name: a.filename,
      size: formatFileSize(a.size),
      path: a.path || '',
    })),
    folder: normalizedFolder,
    accountId: dto.account_id.toString(),
  }
}

// 统一文件夹名称为小写（兼容旧数据的大写格式）
function normalizeFolderName(folder: string): EmailFolder {
  const folderLower = folder.toLowerCase()
  // 验证是否为有效的文件夹名称
  const validFolders: EmailFolder[] = ['inbox', 'starred', 'sent', 'drafts', 'spam', 'trash', 'archive']
  if (validFolders.includes(folderLower as EmailFolder)) {
    return folderLower as EmailFolder
  }
  // 如果不是标准文件夹名称，返回 inbox（兜底）
  return 'inbox'
}

// 格式化文件大小
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

export const useEmailStore = defineStore('email', () => {
  // ========================================
  // State
  // ========================================

  // 邮件列表
  const emails = ref<Email[]>([])

  // 当前选中的邮件
  const currentEmail = ref<Email | null>(null)

  // 当前文件夹
  const currentFolder = ref<EmailFolder>('inbox')

  // 搜索关键词
  const searchQuery = ref('')

  // 加载状态
  const isLoading = ref(false)

  // 同步状态
  const isSyncing = ref(false)

  // 选中的邮件 IDs（用于批量操作）
  const selectedEmailIds = ref<Set<string>>(new Set())

  // 分页
  const currentPage = ref(0)
  const pageSize = ref(100)  // 默认显示100封邮件
  const totalEmails = ref(0)

  // 是否还有更多邮件可以加载
  const hasMore = ref(true)
  const isLoadingMore = ref(false)

  // 文件夹统计数据（从后端获取）
  interface FolderStat {
    id: number
    account_id: number
    name: string
    imap_name: string
    email_count: number
    unread_count: number
  }
  const folderStats = ref<Record<string, FolderStat>>({})

  // ========================================
  // Getters
  // ========================================

  // 根据当前文件夹过滤邮件
  const folderEmails = computed(() => {
    if (currentFolder.value === 'starred') {
      return emails.value.filter(email => email.starred)
    }
    return emails.value.filter(email => email.folder === currentFolder.value)
  })

  // 搜索过滤后的邮件
  const filteredEmails = computed(() => {
    let result = folderEmails.value

    if (searchQuery.value.trim()) {
      const query = searchQuery.value.toLowerCase()
      result = result.filter(email =>
        email.subject.toLowerCase().includes(query) ||
        email.sender.toLowerCase().includes(query) ||
        email.senderEmail.toLowerCase().includes(query) ||
        email.preview.toLowerCase().includes(query)
      )
    }

    return result
  })

  // 未读邮件数量
  const unreadCount = computed(() => {
    return emails.value.filter(email => email.unread && email.folder === 'inbox').length
  })

  // 各文件夹邮件数量
  const folderCounts = computed(() => {
    // 使用 folderStats 中的数据，如果没有则返回 0
    const getCount = (folderName: string, isUnread: boolean = false) => {
      const stat = folderStats.value[folderName]
      const count = stat ? (isUnread ? stat.unread_count : stat.email_count) : 0
      console.log(`[folderCounts] ${folderName}:`, {
        stat,
        isUnread,
        count,
        allStats: folderStats.value
      })
      return count
    }

    const counts = {
      inbox: getCount('inbox', true),
      starred: emails.value.filter(e => e.starred).length, // 星标邮件仍从当前邮件列表计算
      sent: getCount('sent', true),  // 改为显示未读数
      drafts: getCount('drafts', true),  // 改为显示未读数
      spam: getCount('spam', true),
      trash: getCount('trash', true),
    }

    console.log('[folderCounts] 最终结果:', counts)
    return counts
  })

  // 当前邮件在列表中的索引
  const currentEmailIndex = computed(() => {
    if (!currentEmail.value) return -1
    return filteredEmails.value.findIndex(e => e.id === currentEmail.value?.id)
  })

  // 是否有上一封邮件
  const hasPreviousEmail = computed(() => currentEmailIndex.value > 0)

  // 是否有下一封邮件
  const hasNextEmail = computed(() =>
    currentEmailIndex.value >= 0 && currentEmailIndex.value < filteredEmails.value.length - 1
  )

  // 是否有选中的邮件（批量操作）
  const hasSelectedEmails = computed(() => selectedEmailIds.value.size > 0)

  // 选中的邮件列表
  const selectedEmails = computed(() =>
    emails.value.filter(e => selectedEmailIds.value.has(e.id))
  )

  // ========================================
  // Actions
  // ========================================

  // 获取账号 ID
  function getAccountId(): number {
    const accountStore = useAccountStore()
    if (!accountStore.currentAccount) {
      throw new Error('请先选择账号')
    }
    return parseInt(accountStore.currentAccount.id)
  }

  // 获取邮件列表
  async function fetchEmails(page = 0, append = false) {
    console.log('[fetchEmails] ========== 开始获取邮件列表 ==========')
    console.log('[fetchEmails] 参数:', { page, currentPage: currentPage.value, append })

    const accountStore = useAccountStore()
    console.log('[fetchEmails] 当前账号状态:', {
      hasAccount: !!accountStore.currentAccount,
      accountId: accountStore.currentAccount?.id,
      accountEmail: accountStore.currentAccount?.email
    })

    if (!accountStore.currentAccount) {
      // 没有账号时，清空邮件列表并返回
      console.log('[fetchEmails] ❌ 没有账号，清空邮件列表')
      emails.value = []
      totalEmails.value = 0
      currentPage.value = 0
      hasMore.value = true
      return
    }

    console.log('[fetchEmails] 当前文件夹:', currentFolder.value)
    console.log('[fetchEmails] 每页数量:', pageSize.value)

    // 非追加模式时才设置 isLoading
    if (!append) {
      isLoading.value = true
    } else {
      isLoadingMore.value = true
    }

    try {
      const accountId = getAccountId()
      const folder = currentFolder.value

      console.log('[fetchEmails] 📡 调用后端 list_emails 命令')
      console.log('[fetchEmails] 调用参数:', {
        accountId,
        folder,
        page,
        limit: pageSize.value
      })

      const response = await invoke<EmailListResponse>('list_emails', {
        accountId,
        folder,
        page,
        limit: pageSize.value,
      })

      console.log('[fetchEmails] ✅ 后端返回成功')
      console.log('[fetchEmails] 返回数据:', {
        total: response.total,
        page: response.page,
        pageSize: response.page_size,
        emailCount: response.emails.length
      })

      if (response.emails.length > 0) {
        console.log('[fetchEmails] 前3封邮件:', response.emails.slice(0, 3).map(e => ({
          id: e.id,
          subject: e.subject,
          folder: e.folder,
          sender: e.sender_email
        })))
      }

      const newEmails = response.emails.map(dtoToEmail)

      if (append) {
        // 追加模式：添加到现有列表
        emails.value = [...emails.value, ...newEmails]
        currentPage.value = response.page
      } else {
        // 替换模式：替换整个列表
        emails.value = newEmails
        totalEmails.value = response.total
        currentPage.value = response.page
      }

      // 更新 hasMore 状态
      hasMore.value = emails.value.length < response.total

      // 获取文件夹统计数据
      await fetchFolderStats()

      console.log('[fetchEmails] ✅ 邮件列表更新完成')
      console.log('[fetchEmails] 当前邮件列表状态:', {
        totalCount: emails.value.length,
        total: totalEmails.value,
        currentFolder: currentFolder.value,
        hasMore: hasMore.value
      })
    } catch (error) {
      console.error('[fetchEmails] ❌ 获取邮件列表失败:', error)
      throw error
    } finally {
      if (!append) {
        isLoading.value = false
      } else {
        isLoadingMore.value = false
      }
      console.log('[fetchEmails] ========== 获取邮件列表结束 ==========')
    }
  }

  // 加载更多邮件
  async function loadMore() {
    console.log('[loadMore] ========== 开始加载更多邮件 ==========')

    // 防止重复加载
    if (isLoadingMore.value || !hasMore.value) {
      console.log('[loadMore] ⚠️  跳过加载:', { isLoadingMore: isLoadingMore.value, hasMore: hasMore.value })
      return
    }

    try {
      const nextPage = currentPage.value + 1
      console.log('[loadMore] 加载第', nextPage, '页')

      await fetchEmails(nextPage, true)
    } catch (error) {
      console.error('[loadMore] ❌ 加载更多失败:', error)
      throw error
    }
  }

  // 获取邮件详情
  async function fetchEmailDetail(emailId: string) {
    try {
      const dto = await invoke<EmailDetailDto>('get_email', { id: parseInt(emailId) })
      const email = detailDtoToEmail(dto)

      // 更新当前邮件
      currentEmail.value = email

      // 更新列表中的邮件
      const index = emails.value.findIndex(e => e.id === emailId)
      if (index !== -1) {
        emails.value[index] = email
      }

      return email
    } catch (error) {
      console.error('获取邮件详情失败:', error)
      throw error
    }
  }

  // 获取文件夹统计数据
  async function fetchFolderStats() {
    console.log('[fetchFolderStats] ========== 开始获取文件夹统计 ==========')

    const accountStore = useAccountStore()
    if (!accountStore.currentAccount) {
      console.log('[fetchFolderStats] ❌ 没有账号，跳过')
      return
    }

    try {
      const accountId = getAccountId()
      console.log('[fetchFolderStats] 调用参数:', { accountId })

      const stats = await invoke<FolderStat[]>('get_folder_stats', { accountId })

      console.log('[fetchFolderStats] ✅ 获取成功，文件夹数量:', stats.length)

      // 将数组转换为对象，以文件夹名称为键
      const statsMap: Record<string, FolderStat> = {}
      for (const stat of stats) {
        statsMap[stat.name] = stat
      }

      folderStats.value = statsMap

      console.log('[fetchFolderStats] ✅ 文件夹统计已更新:', Object.keys(statsMap))
    } catch (error) {
      console.error('[fetchFolderStats] ❌ 获取失败:', error)
    }
  }

  // 同步账号邮件
  async function syncAccount() {
    const accountStore = useAccountStore()
    if (!accountStore.currentAccount) {
      // 没有账号时，直接返回
      return
    }

    isSyncing.value = true
    try {
      const accountId = parseInt(accountStore.currentAccount.id)
      const count = await invoke<number>('sync_account', { accountId })

      // 同步完成后刷新邮件列表
      await fetchEmails()

      // 更新账号的同步时间
      await accountStore.fetchAccounts()

      return count
    } catch (error) {
      console.error('同步账号失败:', error)
      throw error
    } finally {
      isSyncing.value = false
    }
  }

  // 选择邮件
  async function selectEmail(email: Email | null) {
    if (!email) {
      currentEmail.value = null
      return
    }

    // 标记为已读
    if (email.unread) {
      await markAsRead(email.id)
    }

    // 获取详情
    await fetchEmailDetail(email.id)
  }

  // 切换文件夹
  async function setFolder(folder: EmailFolder) {
    currentFolder.value = folder
    currentEmail.value = null
    searchQuery.value = ''
    clearSelection()
    currentPage.value = 0
    await fetchEmails(0)
  }

  // 初始化：获取文件夹统计
  async function initialize() {
    console.log('[emailStore] ========== 初始化 Email Store ==========')
    await fetchFolderStats()
    console.log('[emailStore] ========== Email Store 初始化完成 ==========')
  }

  // 搜索邮件
  function setSearchQuery(query: string) {
    searchQuery.value = query
  }

  // 标记邮件为已读
  async function markAsRead(emailId: string) {
    try {
      await invoke('mark_as_read', { emailId: parseInt(emailId), isRead: true })

      const email = emails.value.find(e => e.id === emailId)
      if (email) {
        email.unread = false
      }
    } catch (error) {
      console.error('标记已读失败:', error)
      throw error
    }
  }

  // 标记邮件为未读
  async function markAsUnread(emailId: string) {
    try {
      await invoke('mark_as_read', { emailId: parseInt(emailId), isRead: false })

      const email = emails.value.find(e => e.id === emailId)
      if (email) {
        email.unread = true
      }
    } catch (error) {
      console.error('标记未读失败:', error)
      throw error
    }
  }

  // 切换星标
  async function toggleStar(emailId: string) {
    try {
      await invoke('toggle_star', { emailId: parseInt(emailId) })

      const email = emails.value.find(e => e.id === emailId)
      if (email) {
        email.starred = !email.starred
      }
    } catch (error) {
      console.error('切换星标失败:', error)
      throw error
    }
  }

  // 移动邮件到文件夹
  async function moveEmail(emailId: string, folder: EmailFolder) {
    try {
      await invoke('move_email_to_folder', {
        emailId: parseInt(emailId),
        folder,
      })

      const email = emails.value.find(e => e.id === emailId)
      if (email) {
        email.folder = folder
      }

      if (currentEmail.value?.id === emailId) {
        currentEmail.value = null
      }
    } catch (error) {
      console.error('移动邮件失败:', error)
      throw error
    }
  }

  // 删除邮件（移动到垃圾箱）
  async function deleteEmail(emailId: string) {
    await moveEmail(emailId, 'trash')
  }

  // 永久删除邮件
  async function permanentlyDeleteEmail(emailId: string) {
    try {
      await invoke('delete_emails', { emailIds: [parseInt(emailId)] })

      const index = emails.value.findIndex(e => e.id === emailId)
      if (index !== -1) {
        emails.value.splice(index, 1)
      }

      if (currentEmail.value?.id === emailId) {
        currentEmail.value = null
      }
    } catch (error) {
      console.error('永久删除失败:', error)
      throw error
    }
  }

  // 导航到上一封邮件
  function navigatePrevious() {
    if (hasPreviousEmail.value) {
      selectEmail(filteredEmails.value[currentEmailIndex.value - 1])
    }
  }

  // 导航到下一封邮件
  function navigateNext() {
    if (hasNextEmail.value) {
      selectEmail(filteredEmails.value[currentEmailIndex.value + 1])
    }
  }

  // 批量选择邮件
  function toggleEmailSelection(emailId: string) {
    if (selectedEmailIds.value.has(emailId)) {
      selectedEmailIds.value.delete(emailId)
    } else {
      selectedEmailIds.value.add(emailId)
    }
  }

  // 全选当前过滤的邮件
  function selectAllEmails() {
    filteredEmails.value.forEach(email => {
      selectedEmailIds.value.add(email.id)
    })
  }

  // 清除选择
  function clearSelection() {
    selectedEmailIds.value.clear()
  }

  // 批量标记已读
  async function batchMarkAsRead() {
    const emailIds = Array.from(selectedEmailIds.value).map(id => parseInt(id))
    try {
      for (const emailId of emailIds) {
        await invoke('mark_as_read', { emailId, isRead: true })
        const email = emails.value.find(e => e.id === emailId.toString())
        if (email) {
          email.unread = false
        }
      }
      clearSelection()
    } catch (error) {
      console.error('批量标记已读失败:', error)
      throw error
    }
  }

  // 批量移动
  async function batchMoveToFolder(folder: EmailFolder) {
    const emailIds = Array.from(selectedEmailIds.value)
    try {
      for (const emailId of emailIds) {
        await moveEmail(emailId, folder)
      }
      clearSelection()
    } catch (error) {
      console.error('批量移动失败:', error)
      throw error
    }
  }

  // 批量删除
  async function batchDelete() {
    const emailIds = Array.from(selectedEmailIds.value).map(id => parseInt(id))
    try {
      await invoke('delete_emails', { emailIds })

      // 从列表中移除
      emailIds.forEach(id => {
        const index = emails.value.findIndex(e => e.id === id.toString())
        if (index !== -1) {
          emails.value.splice(index, 1)
        }
      })

      clearSelection()
    } catch (error) {
      console.error('批量删除失败:', error)
      throw error
    }
  }

  // 添加新邮件（用于发送邮件后）
  function addEmail(email: Email) {
    emails.value.unshift(email)
  }

  // 更新邮件
  function updateEmail(emailId: string, updates: Partial<Email>) {
    const email = emails.value.find(e => e.id === emailId)
    if (email) {
      Object.assign(email, updates)
    }
  }

  // 清空所有邮件
  function clearEmails() {
    emails.value = []
    currentEmail.value = null
    searchQuery.value = ''
    clearSelection()
  }

  // 批量归档邮件
  async function archiveEmails(emailIds: string[]) {
    const results = { success: 0, failed: 0 }
    for (const id of emailIds) {
      try {
        await moveEmail(id, 'archive' as EmailFolder)
        results.success++
      } catch {
        results.failed++
      }
    }
    return results
  }

  // 批量删除邮件
  async function deleteEmails(emailIds: string[]) {
    const ids = emailIds.map(id => parseInt(id))
    try {
      await invoke('delete_emails', { emailIds: ids })

      // 从列表中移除
      ids.forEach(id => {
        const index = emails.value.findIndex(e => e.id === id.toString())
        if (index !== -1) {
          emails.value.splice(index, 1)
        }
      })

      return { success: ids.length, failed: 0 }
    } catch (error) {
      console.error('批量删除失败:', error)
      return { success: 0, failed: ids.length }
    }
  }

  // 批量标记已读
  async function markAsReadBatch(emailIds: string[]) {
    const results = { success: 0, failed: 0 }
    for (const id of emailIds) {
      try {
        await markAsRead(id)
        results.success++
      } catch {
        results.failed++
      }
    }
    return results
  }

  // 批量标记未读
  async function markAsUnreadBatch(emailIds: string[]) {
    const results = { success: 0, failed: 0 }
    for (const id of emailIds) {
      try {
        await markAsUnread(id)
        results.success++
      } catch {
        results.failed++
      }
    }
    return results
  }

  // 批量星标
  async function starEmails(emailIds: string[]) {
    const results = { success: 0, failed: 0 }
    for (const id of emailIds) {
      try {
        await toggleStar(id)
        const email = emails.value.find(e => e.id === id)
        if (email && !email.starred) {
          await toggleStar(id)
        }
        results.success++
      } catch {
        results.failed++
      }
    }
    return results
  }

  // 批量取消星标
  async function unstarEmails(emailIds: string[]) {
    const results = { success: 0, failed: 0 }
    for (const id of emailIds) {
      try {
        const email = emails.value.find(e => e.id === id)
        if (email && email.starred) {
          await toggleStar(id)
        }
        results.success++
      } catch {
        results.failed++
      }
    }
    return results
  }

  return {
    // State
    emails,
    currentEmail,
    currentFolder,
    searchQuery,
    isLoading,
    isSyncing,
    selectedEmailIds,
    currentPage,
    pageSize,
    totalEmails,
    hasMore,
    isLoadingMore,

    // Getters
    folderEmails,
    filteredEmails,
    unreadCount,
    folderCounts,
    currentEmailIndex,
    hasPreviousEmail,
    hasNextEmail,
    hasSelectedEmails,
    selectedEmails,

    // Actions
    fetchEmails,
    loadMore,
    fetchEmailDetail,
    fetchFolderStats,
    initialize,
    syncAccount,
    selectEmail,
    setFolder,
    setSearchQuery,
    markAsRead,
    markAsUnread,
    toggleStar,
    moveEmail,
    deleteEmail,
    permanentlyDeleteEmail,
    navigatePrevious,
    navigateNext,
    toggleEmailSelection,
    selectAllEmails,
    clearSelection,
    batchMarkAsRead,
    batchMoveToFolder,
    batchDelete,
    addEmail,
    updateEmail,
    clearEmails,
    archiveEmails,
    deleteEmails,
    markAsReadBatch,
    markAsUnreadBatch,
    starEmails,
    unstarEmails,
  }
})
