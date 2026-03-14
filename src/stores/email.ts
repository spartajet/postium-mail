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
    folder: dto.folder as EmailFolder,
    accountId: dto.account_id.toString(),
  }
}

function detailDtoToEmail(dto: EmailDetailDto): Email {
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
    folder: dto.folder as EmailFolder,
    accountId: dto.account_id.toString(),
  }
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
  const pageSize = ref(50)
  const totalEmails = ref(0)

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
    return {
      inbox: emails.value.filter(e => e.folder === 'inbox' && e.unread).length,
      starred: emails.value.filter(e => e.starred).length,
      sent: emails.value.filter(e => e.folder === 'sent').length,
      drafts: emails.value.filter(e => e.folder === 'drafts').length,
      spam: emails.value.filter(e => e.folder === 'spam').length,
      trash: emails.value.filter(e => e.folder === 'trash').length,
    }
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
  async function fetchEmails(page = 0) {
    isLoading.value = true
    try {
      const accountId = getAccountId()
      const folder = currentFolder.value

      const response = await invoke<EmailListResponse>('list_emails', {
        accountId,
        folder,
        page,
        limit: pageSize.value,
      })

      emails.value = response.emails.map(dtoToEmail)
      totalEmails.value = response.total
      currentPage.value = response.page
    } catch (error) {
      console.error('获取邮件列表失败:', error)
      throw error
    } finally {
      isLoading.value = false
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

  // 同步账号邮件
  async function syncAccount() {
    const accountStore = useAccountStore()
    if (!accountStore.currentAccount) {
      throw new Error('请先选择账号')
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
    fetchEmailDetail,
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
