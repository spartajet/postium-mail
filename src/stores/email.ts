import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Email, EmailFolder } from '@/types'
import { generateEmails, delay } from '@/mocks'

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

  // 选中的邮件 IDs（用于批量操作）
  const selectedEmailIds = ref<Set<string>>(new Set())

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

  // 获取邮件列表
  async function fetchEmails(accountId?: string) {
    isLoading.value = true
    try {
      // 模拟 API 调用延迟
      await delay(300)

      // 使用 mock 数据
      emails.value = generateEmails(50, accountId)
    } catch (error) {
      console.error('Failed to fetch emails:', error)
    } finally {
      isLoading.value = false
    }
  }

  // 选择邮件
  function selectEmail(email: Email | null) {
    currentEmail.value = email

    // 标记为已读
    if (email && email.unread) {
      markAsRead(email.id)
    }
  }

  // 切换文件夹
  function setFolder(folder: EmailFolder) {
    currentFolder.value = folder
    currentEmail.value = null
    searchQuery.value = ''
    clearSelection()
  }

  // 搜索邮件
  function setSearchQuery(query: string) {
    searchQuery.value = query
  }

  // 标记邮件为已读
  function markAsRead(emailId: string) {
    const email = emails.value.find(e => e.id === emailId)
    if (email) {
      email.unread = false
    }
  }

  // 标记邮件为未读
  function markAsUnread(emailId: string) {
    const email = emails.value.find(e => e.id === emailId)
    if (email) {
      email.unread = true
    }
  }

  // 切换星标
  function toggleStar(emailId: string) {
    const email = emails.value.find(e => e.id === emailId)
    if (email) {
      email.starred = !email.starred
    }
  }

  // 移动邮件到文件夹
  function moveEmail(emailId: string, folder: EmailFolder) {
    const email = emails.value.find(e => e.id === emailId)
    if (email) {
      email.folder = folder
    }
  }

  // 删除邮件（移动到垃圾箱）
  function deleteEmail(emailId: string) {
    moveEmail(emailId, 'trash')
    if (currentEmail.value?.id === emailId) {
      currentEmail.value = null
    }
  }

  // 永久删除邮件
  function permanentlyDeleteEmail(emailId: string) {
    const index = emails.value.findIndex(e => e.id === emailId)
    if (index !== -1) {
      emails.value.splice(index, 1)
    }
    if (currentEmail.value?.id === emailId) {
      currentEmail.value = null
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
  function batchMarkAsRead() {
    selectedEmails.value.forEach(email => {
      email.unread = false
    })
    clearSelection()
  }

  // 批量移动
  function batchMoveToFolder(folder: EmailFolder) {
    selectedEmails.value.forEach(email => {
      email.folder = folder
    })
    clearSelection()
  }

  // 批量删除
  function batchDelete() {
    selectedEmails.value.forEach(email => {
      email.folder = 'trash'
    })
    clearSelection()
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

  return {
    // State
    emails,
    currentEmail,
    currentFolder,
    searchQuery,
    isLoading,
    selectedEmailIds,

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
  }
})
