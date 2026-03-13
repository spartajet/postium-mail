<script setup lang="ts">
import { ref, computed, nextTick } from 'vue'
import { useAIChatStore, useEmailStore, useUIStore } from '@/stores'
import type { ChatMessage } from '@/types'

import {
  NConfigProvider,
  NGlobalStyle,
  darkTheme,
  lightTheme,
  NButton,
  NInput,
  NIcon,
  NCard,
  NSpace,
  NAvatar,
  NSpin
} from 'naive-ui'

// 图标
import {
  CloseOutlined,
  SendOutlined,
  SmartToyOutlined,
  PersonOutlined,
  CheckCircleOutlined,
  ErrorOutlined
} from '@vicons/material'

// Stores
const aiChatStore = useAIChatStore()
const emailStore = useEmailStore()
const uiStore = useUIStore()

// 输入框引用
const inputRef = ref<HTMLInputElement | null>(null)
const messagesRef = ref<HTMLDivElement | null>(null)

// 用户输入
const userInput = ref('')

// 是否正在发送
const isSending = ref(false)

// 计算属性
const messages = computed(() => aiChatStore.messages)
const isLoading = computed(() => aiChatStore.isLoading)

// 关闭模态框
function closeModal() {
  uiStore.closeAIChatModal()
}

// 发送消息
async function sendMessage() {
  const content = userInput.value.trim()
  if (!content || isSending.value) return

  // 添加用户消息
  aiChatStore.addMessage({
    role: 'user',
    content
  })

  // 清空输入
  userInput.value = ''
  isSending.value = true

  try {
    // 调用 AI API
    await aiChatStore.sendMessage(content)
  } catch (error) {
    console.error('发送消息失败:', error)
    uiStore.showError('AI 响应失败，请重试')
  } finally {
    isSending.value = false
  }

  // 滚动到底部
  await nextTick()
  scrollToBottom()
}

// 处理待执行操作
async function handlePendingAction(message: ChatMessage) {
  if (!message.pendingAction) return

  const { type, description, affectedCount, emailIds } = message.pendingAction

  // 确认操作
  const confirmed = confirm(`${description}\n\n确定要对 ${affectedCount} 封邮件执行此操作吗？`)
  if (!confirmed) return

  try {
    // 执行操作
    let result
    switch (type) {
      case 'archive':
        result = await emailStore.archiveEmails(emailIds)
        break
      case 'delete':
        result = await emailStore.deleteEmails(emailIds)
        break
      case 'mark_read':
        result = await emailStore.markAsReadBatch(emailIds)
        break
      case 'mark_unread':
        result = await emailStore.markAsUnreadBatch(emailIds)
        break
      case 'star':
        result = await emailStore.starEmails(emailIds)
        break
      case 'unstar':
        result = await emailStore.unstarEmails(emailIds)
        break
      default:
        throw new Error(`未知操作类型: ${type}`)
    }

    // 更新消息状态
    aiChatStore.updateMessageResult(message.id, {
      success: result.success || 0,
      failed: result.failed || 0
    })

    uiStore.showSuccess(`操作完成: 成功 ${result.success || 0}, 失败 ${result.failed || 0}`)
  } catch (error) {
    console.error('执行操作失败:', error)
    uiStore.showError('操作失败，请重试')
  }
}

// 滚动到底部
function scrollToBottom() {
  if (messagesRef.value) {
    messagesRef.value.scrollTop = messagesRef.value.scrollHeight
  }
}

// 格式化时间
function formatTime(date: Date) {
  return new Date(date).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

// 快捷操作
const quickActions = [
  { label: '归档所有已读邮件', prompt: '帮我归档所有已读邮件' },
  { label: '标记垃圾邮件', prompt: '识别并标记垃圾邮件' },
  { label: '整理工作邮件', prompt: '整理所有工作相关的邮件' },
  { label: '查找重要邮件', prompt: '查找最近一周的重要邮件' }
]

async function handleQuickAction(prompt: string) {
  userInput.value = prompt
  await sendMessage()
}
</script>

<template>
  <NConfigProvider :theme="uiStore.isDarkTheme ? darkTheme : lightTheme">
    <NGlobalStyle />
    <div class="ai-chat-modal-overlay" @click.self="closeModal">
      <NCard class="ai-chat-modal" :bordered="false">
        <template #header>
          <div class="modal-header">
            <div class="header-left">
              <NIcon>
                <SmartToyOutlined />
              </NIcon>
              <h3>AI 助手</h3>
            </div>
            <NButton text @click="closeModal">
              <template #icon>
                <NIcon>
                  <CloseOutlined />
                </NIcon>
              </template>
            </NButton>
          </div>
        </template>

        <!-- 消息列表 -->
        <div class="messages-container" ref="messagesRef">
          <!-- 欢迎/空状态 -->
          <div v-if="messages.length === 0" class="empty-state">
            <NIcon size="64" :depth="3">
              <SmartToyOutlined />
            </NIcon>
            <h3>你好！我是 AI 助手</h3>
            <p>我可以帮你处理邮件，请选择一个快捷操作或直接提问</p>
            <NSpace vertical :size="12">
              <NButton
                v-for="action in quickActions"
                :key="action.label"
                size="small"
                @click="handleQuickAction(action.prompt)"
              >
                {{ action.label }}
              </NButton>
            </NSpace>
          </div>

          <!-- 消息列表 -->
          <div v-else class="messages-list">
            <div
              v-for="message in messages"
              :key="message.id"
              :class="['message', message.role]"
            >
              <!-- 用户消息 -->
              <div v-if="message.role === 'user'" class="message-content user">
                <NAvatar size="small" round>
                  <NIcon>
                    <PersonOutlined />
                  </NIcon>
                </NAvatar>
                <div class="message-bubble">
                  <p>{{ message.content }}</p>
                  <span class="message-time">{{ formatTime(message.timestamp) }}</span>
                </div>
              </div>

              <!-- AI 消息 -->
              <div v-else class="message-content assistant">
                <NAvatar size="small" round>
                  <NIcon>
                    <SmartToyOutlined />
                  </NIcon>
                </NAvatar>
                <div class="message-bubble">
                  <p>{{ message.content }}</p>
                  <span class="message-time">{{ formatTime(message.timestamp) }}</span>

                  <!-- 待执行操作 -->
                  <div v-if="message.pendingAction" class="pending-action">
                    <div class="action-info">
                      <NIcon>
                        <CheckCircleOutlined />
                      </NIcon>
                      <span>{{ message.pendingAction.description }}</span>
                      <strong>{{ message.pendingAction.affectedCount }}</strong>
                      <span>封邮件</span>
                    </div>
                    <NButton
                      type="primary"
                      size="small"
                      @click="handlePendingAction(message)"
                    >
                      执行操作
                    </NButton>
                  </div>

                  <!-- 操作结果 -->
                  <div v-else-if="message.actionResult" class="action-result">
                    <NIcon :color="message.actionResult.failed === 0 ? '#10B981' : '#EF4444'">
                      <ErrorOutlined />
                    </NIcon>
                    <span>
                      成功 {{ message.actionResult.success }},
                      失败 {{ message.actionResult.failed }}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <!-- 加载状态 -->
            <div v-if="isLoading" class="message assistant">
              <div class="message-content">
                <NAvatar size="small" round>
                  <NIcon>
                    <SmartToyOutlined />
                  </NIcon>
                </NAvatar>
                <div class="message-bubble">
                  <NSpin size="small" />
                  <span>AI 正在思考...</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 输入框 -->
        <template #footer>
          <div class="input-area">
            <NInput
              ref="inputRef"
              v-model:value="userInput"
              type="textarea"
              :autosize="{ minRows: 1, maxRows: 4 }"
              placeholder="输入消息... (Ctrl+Enter 发送)"
              @keydown.ctrl.enter="sendMessage"
            />
            <NButton
              type="primary"
              :disabled="!userInput.trim() || isSending"
              :loading="isSending"
              @click="sendMessage"
            >
              <template #icon>
                <NIcon>
                  <SendOutlined />
                </NIcon>
              </template>
              发送
            </NButton>
          </div>
        </template>
      </NCard>
    </div>
  </NConfigProvider>
</template>

<style scoped>
.ai-chat-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.ai-chat-modal {
  width: 600px;
  height: 700px;
  display: flex;
  flex-direction: column;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.header-left h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  color: var(--n-text-color-2);
}

.empty-state h3 {
  margin: 16px 0 8px;
  font-size: 18px;
  font-weight: 600;
}

.empty-state p {
  margin-bottom: 24px;
  font-size: 14px;
}

.messages-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.message {
  display: flex;
  width: 100%;
}

.message.user {
  justify-content: flex-end;
}

.message-content {
  display: flex;
  gap: 8px;
  max-width: 80%;
}

.message.user .message-content {
  flex-direction: row-reverse;
}

.message-bubble {
  padding: 12px 16px;
  border-radius: 12px;
  background-color: var(--n-color-modal);
  position: relative;
}

.message.user .message-bubble {
  background-color: var(--n-color-target);
  color: var(--n-color-target-text);
}

.message-bubble p {
  margin: 0 0 4px 0;
  word-wrap: break-word;
}

.message-time {
  font-size: 11px;
  opacity: 0.7;
}

.pending-action {
  margin-top: 12px;
  padding: 12px;
  background-color: var(--n-color-modal);
  border-radius: 8px;
  border: 1px solid var(--n-border-color);
}

.action-info {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
  font-size: 13px;
}

.action-result {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
  font-size: 13px;
}

.input-area {
  display: flex;
  gap: 12px;
  align-items: flex-end;
}

.input-area .n-input {
  flex: 1;
}

/* 响应式 */
@media (max-width: 768px) {
  .ai-chat-modal {
    width: 100%;
    height: 100%;
    border-radius: 0;
  }
}
</style>
