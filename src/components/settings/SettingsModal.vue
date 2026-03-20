<script setup lang="ts">
import { computed, ref, h, nextTick } from 'vue'
import { useUIStore, useAccountStore } from '@/stores'
import { useI18n } from 'vue-i18n'
import { availableLanguages } from '@/locales'
import {
  NButton,
  NInput,
  NSelect,
  NSwitch,
  NIcon,
  NLayout,
  NLayoutContent,
  NLayoutSider,
  NMenu,
  NCard,
  NSpace,
  NDivider,
  NRadioGroup,
  NRadio,
  NList,
  NListItem,
  NThing,
  NPopconfirm
} from 'naive-ui'
import type { MenuOption } from 'naive-ui'

// 图标组件
import {
  SettingsOutlined,
  NotificationsOutlined,
  AutoAwesomeOutlined,
  PaletteOutlined,
  KeyboardOutlined,
  CloseOutlined,
  ComputerOutlined,
  WbSunnyOutlined,
  BedtimeOutlined,
  EmailOutlined,
  AddOutlined,
  DeleteOutlined,
  EditOutlined
} from '@vicons/material'

// Store
const uiStore = useUIStore()
const accountStore = useAccountStore()

// i18n
const { t } = useI18n()

// 菜单配置
const menuOptions = computed<MenuOption[]>(() => [
  {
    label: '账号',
    key: 'accounts',
    icon: () => h(NIcon, null, { default: () => h(EmailOutlined) })
  },
  {
    label: t('settings.general'),
    key: 'general',
    icon: () => h(NIcon, null, { default: () => h(SettingsOutlined) })
  },
  {
    label: t('settings.notifications'),
    key: 'notifications',
    icon: () => h(NIcon, null, { default: () => h(NotificationsOutlined) })
  },
  {
    label: 'AI',
    key: 'ai',
    icon: () => h(NIcon, null, { default: () => h(AutoAwesomeOutlined) })
  },
  {
    label: t('settings.appearance'),
    key: 'appearance',
    icon: () => h(NIcon, null, { default: () => h(PaletteOutlined) })
  },
  {
    label: t('settings.shortcuts'),
    key: 'shortcuts',
    icon: () => h(NIcon, null, { default: () => h(KeyboardOutlined) })
  }
])

// 当前面板
const currentPanel = computed(() => uiStore.settingsPanel)
const activeKey = computed(() => currentPanel.value as string)

// 设置表单数据
const formData = ref({
  language: uiStore.settings.language,
  theme: uiStore.settings.theme,
  notifications: { ...uiStore.settings.notifications },
  ai: { ...uiStore.settings.ai }
})

// 语言选项
const languageOptions = computed(() =>
  availableLanguages.map(lang => ({
    label: `${lang.flag} ${lang.name}`,
    value: lang.code
  }))
)

// 语言切换处理
function handleLanguageChange(value: string) {
  uiStore.setLanguage(value as any)
}

// AI 提供商选项
const aiProviderOptions = [
  { label: 'OpenAI', value: 'openai' },
  { label: 'Anthropic', value: 'anthropic' },
  { label: 'Ollama', value: 'ollama' }
]

// 快捷键列表
const shortcuts = [
  { key: '新建邮件', value: 'Ctrl + N' },
  { key: '搜索', value: 'Ctrl + F' },
  { key: '切换侧边栏', value: 'Ctrl + B' },
  { key: '打开设置', value: 'Ctrl + ,' }
]

// 关闭模态框
function closeModal() {
  // 使用 nextTick 确保所有待处理的更新完成后再关闭
  nextTick(() => {
    uiStore.closeSettingsModal()
  })
}

// 更新菜单 key
function handleUpdateKey(key: string) {
  uiStore.setSettingsPanel(key as typeof currentPanel.value)
}

// 保存设置
function saveSettings() {
  uiStore.updateSettings({
    language: formData.value.language,
    theme: formData.value.theme,
    notifications: formData.value.notifications,
    ai: formData.value.ai
  })
  uiStore.showSuccess('设置已保存')
}

// 取消设置
function cancelSettings() {
  formData.value = {
    language: uiStore.settings.language,
    theme: uiStore.settings.theme,
    notifications: { ...uiStore.settings.notifications },
    ai: { ...uiStore.settings.ai }
  }
  closeModal()
}

// 切换主题
function setTheme(theme: 'light' | 'dark' | 'system') {
  formData.value.theme = theme
  uiStore.setTheme(theme)
}

// 账号管理
async function deleteAccount(accountId: string) {
  try {
    await accountStore.removeAccount(accountId)
    uiStore.showSuccess('账号已删除')
  } catch (error) {
    uiStore.showError('删除账号失败')
  }
}

function editAccount(_accountId: string) {
  // TODO: 打开编辑账号模态框
  uiStore.showInfo('编辑账号功能即将推出')
}

function formatDate(date: Date | string): string {
  const d = typeof date === 'string' ? new Date(date) : date
  const now = new Date()
  const diffMs = now.getTime() - d.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return '刚刚'
  if (diffMins < 60) return `${diffMins} 分钟前`
  if (diffHours < 24) return `${diffHours} 小时前`
  if (diffDays < 7) return `${diffDays} 天前`
  return d.toLocaleDateString()
}
</script>

<template>
  <div class="settings-modal-overlay" @click.self="closeModal">
      <NLayout class="settings-modal" has-sider>
        <!-- 左侧菜单 -->
        <NLayoutSider
          width="200"
          bordered
          class="settings-sider"
        >
          <div class="settings-header">
            <h2>设置</h2>
            <NButton text @click="closeModal">
              <template #icon>
                <NIcon>
                  <CloseOutlined />
                </NIcon>
              </template>
            </NButton>
          </div>
          <NMenu
            :value="activeKey"
            :options="menuOptions"
            @update:value="handleUpdateKey"
          />
        </NLayoutSider>

        <!-- 右侧内容 -->
        <NLayoutContent class="settings-content">
          <!-- 账号管理 -->
          <div v-if="currentPanel === 'accounts'" class="panel">
            <NCard title="邮箱账号" :bordered="false">
              <template #header-extra>
                <NButton type="primary" size="small" @click="uiStore.openAddAccountModal">
                  <template #icon>
                    <NIcon><AddOutlined /></NIcon>
                  </template>
                  添加账号
                </NButton>
              </template>

              <div v-if="accountStore.accounts.length === 0" class="empty-state">
                <NIcon :size="48" color="#999">
                  <EmailOutlined />
                </NIcon>
                <p>还没有添加邮箱账号</p>
                <NButton type="primary" @click="uiStore.openAddAccountModal">
                  添加第一个账号
                </NButton>
              </div>

              <NList v-else>
                <NListItem v-for="account in accountStore.accounts" :key="account.id">
                  <template #prefix>
                    <div
                      class="account-avatar"
                      :style="{ backgroundColor: account.color }"
                    >
                      {{ account.name.charAt(0) }}
                    </div>
                  </template>
                  <NThing>
                    <template #header>
                      <div class="account-header">
                        <span class="account-name">{{ account.name }}</span>
                        <NSpace :size="8">
                          <NButton
                            size="tiny"
                            quaternary
                            @click="editAccount(account.id)"
                          >
                            <template #icon>
                              <NIcon><EditOutlined :size="16" /></NIcon>
                            </template>
                          </NButton>
                          <NPopconfirm
                            @positive-click="deleteAccount(account.id)"
                            positive-text="确认删除"
                            negative-text="取消"
                          >
                            <template #trigger>
                              <NButton
                                size="tiny"
                                quaternary
                                type="error"
                              >
                                <template #icon>
                                  <NIcon><DeleteOutlined :size="16" /></NIcon>
                                </template>
                              </NButton>
                            </template>
                            <div>确定要删除账号 "{{ account.name }}" 吗？</div>
                          </NPopconfirm>
                        </NSpace>
                      </div>
                    </template>
                    <template #description>
                      <div class="account-details">
                        <div class="account-email">{{ account.email }}</div>
                        <div class="account-meta">
                          <span class="provider-badge">{{ account.provider }}</span>
                          <span v-if="account.lastSyncAt" class="sync-time">
                            上次同步: {{ formatDate(account.lastSyncAt) }}
                          </span>
                          <span v-else class="sync-time">未同步</span>
                        </div>
                      </div>
                    </template>
                  </NThing>
                </NListItem>
              </NList>
            </NCard>
          </div>

          <!-- 通用设置 -->
          <div v-if="currentPanel === 'general'" class="panel">
            <NCard :title="`${t('settings.general')} ${t('settings.title')}`" :bordered="false">
              <NSpace vertical size="large">
                <div>
                  <div class="setting-label">{{ t('settings.language') }}</div>
                  <NSelect
                    v-model:value="formData.language"
                    :options="languageOptions"
                    @update:value="handleLanguageChange"
                  />
                </div>
              </NSpace>
            </NCard>
          </div>

          <!-- 通知设置 -->
          <div v-else-if="currentPanel === 'notifications'" class="panel">
            <NCard :title="`${t('settings.notifications')} ${t('settings.title')}`" :bordered="false">
              <NSpace vertical size="large">
                <NListItem>
                  <NThing :title="t('notifications.enabled')" />
                  <template #suffix>
                    <NSwitch v-model:value="formData.notifications.enabled" />
                  </template>
                </NListItem>
                <NDivider />
                <NListItem>
                  <NThing :title="t('notifications.sound')" />
                  <template #suffix>
                    <NSwitch v-model:value="formData.notifications.sound" />
                  </template>
                </NListItem>
                <NDivider />
                <NListItem>
                  <NThing :title="t('notifications.desktop')" />
                  <template #suffix>
                    <NSwitch v-model:value="formData.notifications.desktop" />
                  </template>
                </NListItem>
              </NSpace>
            </NCard>
          </div>

          <!-- AI 设置 -->
          <div v-else-if="currentPanel === 'ai'" class="panel">
            <NCard title="AI 设置" :bordered="false">
              <NSpace vertical size="large">
                <div>
                  <div class="setting-label">{{ t('ai.provider') }}</div>
                  <NSelect
                    v-model:value="formData.ai.provider"
                    :options="aiProviderOptions"
                  />
                </div>
                <div>
                  <div class="setting-label">{{ t('ai.apiKey') }}</div>
                  <NInput
                    v-model:value="formData.ai.apiKey"
                    type="password"
                    show-password-on="click"
                    :placeholder="t('ai.apiKey')"
                  />
                </div>
                <div>
                  <div class="setting-label">{{ t('ai.model') }}</div>
                  <NInput
                    v-model:value="formData.ai.model"
                    placeholder="例如: gpt-4, claude-3-opus"
                  />
                </div>
              </NSpace>
            </NCard>
          </div>

          <!-- 外观设置 -->
          <div v-else-if="currentPanel === 'appearance'" class="panel">
            <NCard :title="`${t('settings.appearance')} ${t('settings.title')}`" :bordered="false">
              <NSpace vertical size="large">
                <div>
                  <div class="setting-label">{{ t('settings.theme') }}</div>
                  <NRadioGroup v-model:value="formData.theme" @update:value="setTheme">
                    <NSpace vertical>
                      <NRadio value="light">
                        <template #default>
                          <div class="theme-option">
                            <NIcon><WbSunnyOutlined /></NIcon>
                            <span>{{ t('settings.light') }}</span>
                          </div>
                        </template>
                      </NRadio>
                      <NRadio value="dark">
                        <template #default>
                          <div class="theme-option">
                            <NIcon><BedtimeOutlined /></NIcon>
                            <span>{{ t('settings.dark') }}</span>
                          </div>
                        </template>
                      </NRadio>
                      <NRadio value="system">
                        <template #default>
                          <div class="theme-option">
                            <NIcon><ComputerOutlined /></NIcon>
                            <span>{{ t('settings.system') }}</span>
                          </div>
                        </template>
                      </NRadio>
                    </NSpace>
                  </NRadioGroup>
                </div>
              </NSpace>
            </NCard>
          </div>

          <!-- 快捷键设置 -->
          <div v-else-if="currentPanel === 'shortcuts'" class="panel">
            <NCard title="快捷键设置" :bordered="false">
              <NList>
                <NListItem v-for="item in shortcuts" :key="item.key">
                  <NThing :title="item.key" />
                  <template #suffix>
                    <code class="shortcut-key">{{ item.value }}</code>
                  </template>
                </NListItem>
              </NList>
            </NCard>
          </div>

          <!-- 底部按钮 -->
          <div class="settings-footer">
            <NSpace>
              <NButton @click="cancelSettings">{{ t('common.cancel') }}</NButton>
              <NButton type="primary" @click="saveSettings">{{ t('common.save') }}</NButton>
            </NSpace>
          </div>
        </NLayoutContent>
      </NLayout>
    </div>
</template>

<style scoped>
.settings-modal-overlay {
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

.settings-modal {
  width: 900px;
  height: 80vh;
  background-color: var(--n-color);
  border-radius: 8px;
  overflow: hidden;
}

.settings-sider {
  display: flex;
  flex-direction: column;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid var(--n-border-color);
}

.settings-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.settings-content {
  padding: 24px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.panel {
  flex: 1;
  overflow-y: auto;
}

.setting-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--n-text-color-2);
  margin-bottom: 8px;
}

.theme-option {
  display: flex;
  align-items: center;
  gap: 8px;
}

.shortcut-key {
  padding: 4px 12px;
  background-color: var(--n-color-modal);
  border: 1px solid var(--n-border-color);
  border-radius: 4px;
  font-family: monospace;
  font-size: 13px;
  color: var(--n-text-color-2);
}

.settings-footer {
  padding-top: 16px;
  border-top: 1px solid var(--n-border-color);
  margin-top: auto;
}

/* 响应式 */
@media (max-width: 768px) {
  .settings-modal {
    width: 100%;
    height: 100%;
    border-radius: 0;
  }

  .settings-sider {
    width: 80px !important;
  }

  .settings-header h2 {
    display: none;
  }
}

/* 账号管理样式 */
.account-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-weight: 600;
  font-size: 16px;
}

.account-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.account-name {
  font-weight: 500;
  font-size: 15px;
}

.account-details {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.account-email {
  font-size: 13px;
  color: var(--n-text-color-2);
}

.account-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 12px;
}

.provider-badge {
  padding: 2px 8px;
  background-color: var(--n-color-modal);
  border-radius: 4px;
  font-weight: 500;
  text-transform: uppercase;
}

.sync-time {
  color: var(--n-text-color-3);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  gap: 16px;
  text-align: center;
}

.empty-state p {
  margin: 0;
  color: var(--n-text-color-2);
}
</style>
