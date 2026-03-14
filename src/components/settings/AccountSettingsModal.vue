<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  NModal, NForm, NFormItem, NInput, NInputNumber, NSelect,
  NButton, NSwitch, NAlert, NPopconfirm, NTabs, NTabPane
} from 'naive-ui'
import OAuthLoginModal from '@/components/common/OAuthLoginModal.vue'

const props = defineProps<{
  show: boolean
  account: {
    id: string | number
    name: string
    email: string
    provider: string
    imapHost?: string
    imapPort?: number
    imapSsl?: boolean
    smtpHost?: string
    smtpPort?: number
    smtpSsl?: boolean
    color?: string
    syncEnabled?: boolean
    authType?: string
    oauthProvider?: string
  } | null
}>()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
  (e: 'updated'): void
  (e: 'deleted'): void
}>()

const activeTab = ref('basic')
const loading = ref(false)
const error = ref('')
const success = ref('')
const testing = ref(false)

interface TestResult {
  success: boolean
  connect_time?: number
  login_time?: number
  email_count?: number
  error?: string
}

const testResult = ref<TestResult | null>(null)

const showOAuthModal = ref(false)
const oauthProvider = ref<'microsoft' | 'google'>('microsoft')

// 表单数据
const form = ref({
  name: '',
  email: '',
  provider: 'gmail',
  password: '',
  imapHost: '',
  imapPort: 993,
  imapSsl: true,
  smtpHost: '',
  smtpPort: 587,
  smtpSsl: true,
  color: '#7C3AED',
  syncEnabled: true,
  authType: 'password',
})

const isCustom = computed(() => form.value.provider === 'imap')
const canUseOAuth = computed(() => {
  return form.value.provider === 'outlook' || form.value.provider === 'hotmail' || form.value.provider === 'gmail'
})

// 监听 account 变化，填充表单
watch(() => props.account, (newAccount) => {
  if (newAccount) {
    form.value = {
      name: newAccount.name || '',
      email: newAccount.email || '',
      provider: newAccount.provider || 'gmail',
      password: '', // 不显示现有密码
      imapHost: newAccount.imapHost || '',
      imapPort: newAccount.imapPort || 993,
      imapSsl: newAccount.imapSsl ?? true,
      smtpHost: newAccount.smtpHost || '',
      smtpPort: newAccount.smtpPort || 587,
      smtpSsl: newAccount.smtpSsl ?? true,
      color: newAccount.color || '#7C3AED',
      syncEnabled: newAccount.syncEnabled ?? true,
      authType: newAccount.authType || 'password',
    }
  }
}, { immediate: true })

async function handleUpdate() {
  error.value = ''
  success.value = ''

  if (!form.value.name.trim()) {
    error.value = '请输入账号名称'
    return
  }

  if (!form.value.email.trim()) {
    error.value = '请输入邮箱地址'
    return
  }

  loading.value = true

  try {
    await invoke('update_account', {
      id: props.account?.id,
      account: {
        name: form.value.name,
        email: form.value.email,
        provider: form.value.provider,
        password: form.value.password || undefined,
        imapHost: isCustom.value ? form.value.imapHost : undefined,
        imapPort: isCustom.value ? form.value.imapPort : undefined,
        imapSsl: isCustom.value ? form.value.imapSsl : undefined,
        smtpHost: isCustom.value ? form.value.smtpHost : undefined,
        smtpPort: isCustom.value ? form.value.smtpPort : undefined,
        smtpSsl: isCustom.value ? form.value.smtpSsl : undefined,
        color: form.value.color,
      }
    })

    success.value = '账号信息已更新'
    setTimeout(() => {
      emit('updated')
      emit('update:show', false)
    }, 1000)
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

async function handleTestConnection() {
  error.value = ''
  success.value = ''
  testResult.value = null
  testing.value = true

  try {
    const result = await invoke<TestResult>('test_account_connection', {
      account: {
        email: form.value.email,
        provider: form.value.provider,
        password: form.value.password,
        imapHost: isCustom.value ? form.value.imapHost : undefined,
        imapPort: isCustom.value ? form.value.imapPort : undefined,
        imapSsl: isCustom.value ? form.value.imapSsl : undefined,
        smtpHost: isCustom.value ? form.value.smtpHost : undefined,
        smtpPort: isCustom.value ? form.value.smtpPort : undefined,
        smtpSsl: isCustom.value ? form.value.smtpSsl : undefined,
      }
    })

    testResult.value = result

    if (result.success) {
      success.value = `连接成功！检测到 ${result.email_count || 0} 封邮件`
    } else {
      error.value = result.error || '连接失败'
    }
  } catch (e: any) {
    error.value = String(e)
  } finally {
    testing.value = false
  }
}

async function handleDelete() {
  error.value = ''
  loading.value = true

  try {
    await invoke('delete_account', { id: props.account?.id })
    emit('deleted')
    emit('update:show', false)
  } catch (e: any) {
    error.value = String(e)
    loading.value = false
  }
}

function startOAuthLogin() {
  if (form.value.provider === 'outlook' || form.value.provider === 'hotmail') {
    oauthProvider.value = 'microsoft'
  } else if (form.value.provider === 'gmail') {
    oauthProvider.value = 'google'
  }
  showOAuthModal.value = true
}

async function handleOAuthSuccess(token: { access_token: string, refresh_token: string, expires_at: number }) {
  // OAuth 成功后，保存 token 到账号
  loading.value = true

  try {
    await invoke('update_account', {
      id: props.account?.id,
      account: {
        name: form.value.name,
        email: form.value.email,
        provider: form.value.provider,
        authType: 'oauth',
        oauthProvider: oauthProvider.value,
        oauthToken: token.access_token,
        oauthRefreshToken: token.refresh_token,
        oauthExpiresAt: token.expires_at,
      }
    })

    success.value = 'OAuth 授权成功！'
    form.value.authType = 'oauth'
    setTimeout(() => {
      emit('updated')
    }, 1000)
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <NModal
    :show="show"
    @update:show="emit('update:show', $event)"
    preset="card"
    title="账号设置"
    :style="{ width: '600px' }"
    :mask-closable="!loading && !testing"
    :close-on-esc="!loading && !testing"
  >
    <div class="account-settings">
      <!-- 账号信息卡片 -->
      <div class="account-header">
        <div class="account-avatar" :style="{ backgroundColor: form.color }">
          {{ form.name.charAt(0).toUpperCase() }}
        </div>
        <div class="account-info">
          <h3>{{ form.name }}</h3>
          <p>{{ form.email }}</p>
        </div>
      </div>

      <!-- 提示信息 -->
      <NAlert v-if="error" type="error" :title="error" class="mb-4" closable @close="error = ''" />
      <NAlert v-if="success" type="success" :title="success" class="mb-4" closable @close="success = ''" />

      <!-- 选项卡 -->
      <NTabs v-model:value="activeTab" type="line">
        <!-- 基本设置 -->
        <NTabPane name="basic" tab="基本设置">
          <NForm @submit.prevent="handleUpdate">
            <!-- 账号名称 -->
            <NFormItem label="账号名称">
              <NInput
                v-model:value="form.name"
                placeholder="工作邮箱"
                :disabled="loading"
              />
            </NFormItem>

            <!-- 邮箱地址 -->
            <NFormItem label="邮箱地址">
              <NInput
                v-model:value="form.email"
                placeholder="user@example.com"
                :disabled="loading"
              />
            </NFormItem>

            <!-- 服务商 -->
            <NFormItem label="邮箱服务商">
              <NSelect
                v-model:value="form.provider"
                :options="[
                  { label: 'Gmail', value: 'gmail' },
                  { label: 'Outlook / Hotmail', value: 'outlook' },
                  { label: 'iCloud Mail', value: 'icloud' },
                  { label: 'Yahoo Mail', value: 'yahoo' },
                  { label: '自定义 IMAP/SMTP', value: 'imap' },
                ]"
                :disabled="loading"
              />
            </NFormItem>

            <!-- 颜色标签 -->
            <NFormItem label="颜色标签">
              <div class="color-options">
                <div
                  v-for="color in ['#7C3AED', '#F43F5E', '#10B981', '#F59E0B', '#3B82F6', '#8B5CF6', '#06B6D4']"
                  :key="color"
                  class="color-option"
                  :class="{ active: form.color === color }"
                  :style="{ backgroundColor: color }"
                  @click="form.color = color"
                />
              </div>
            </NFormItem>

            <!-- 同步开关 -->
            <NFormItem label="启用同步">
              <NSwitch
                v-model:value="form.syncEnabled"
                :disabled="loading"
              />
            </NFormItem>
          </NForm>
        </NTabPane>

        <!-- 服务器设置 -->
        <NTabPane name="server" tab="服务器设置">
          <NForm>
            <!-- 自定义 IMAP/SMTP 配置 -->
            <template v-if="isCustom">
              <div class="section-title">IMAP 设置</div>

              <NFormItem label="IMAP 服务器">
                <NInput
                  v-model:value="form.imapHost"
                  placeholder="imap.example.com"
                  :disabled="loading"
                />
              </NFormItem>

              <NFormItem label="IMAP 端口">
                <NInputNumber
                  v-model:value="form.imapPort"
                  :min="1"
                  :max="65535"
                  placeholder="993"
                  :disabled="loading"
                  style="width: 100%"
                />
              </NFormItem>

              <NFormItem label="IMAP SSL">
                <NSwitch
                  v-model:value="form.imapSsl"
                  :disabled="loading"
                />
              </NFormItem>

              <div class="section-title">SMTP 设置</div>

              <NFormItem label="SMTP 服务器">
                <NInput
                  v-model:value="form.smtpHost"
                  placeholder="smtp.example.com"
                  :disabled="loading"
                />
              </NFormItem>

              <NFormItem label="SMTP 端口">
                <NInputNumber
                  v-model:value="form.smtpPort"
                  :min="1"
                  :max="65535"
                  placeholder="587"
                  :disabled="loading"
                  style="width: 100%"
                />
              </NFormItem>

              <NFormItem label="SMTP SSL">
                <NSwitch
                  v-model:value="form.smtpSsl"
                  :disabled="loading"
                />
              </NFormItem>
            </template>

            <div v-else class="preset-info">
              <p>当前使用预设的服务器配置，无需手动设置。</p>
              <p v-if="form.provider === 'gmail'">Gmail: imap.gmail.com:993 (SSL), smtp.gmail.com:587 (SSL)</p>
              <p v-else-if="form.provider === 'outlook' || form.provider === 'hotmail'">
                Outlook: outlook.office365.com:993 (SSL), smtp-mail.outlook.com:587 (SSL)
              </p>
            </div>
          </NForm>
        </NTabPane>

        <!-- 认证设置 -->
        <NTabPane name="auth" tab="认证设置">
          <NForm>
            <div class="section-title">认证方式</div>

            <NFormItem label="当前认证方式">
              <div class="auth-type-display">
                <span v-if="form.authType === 'oauth'" class="auth-badge oauth">
                  OAuth 2.0
                </span>
                <span v-else class="auth-badge password">
                  密码登录
                </span>
              </div>
            </NFormItem>

            <!-- OAuth 登录按钮 -->
            <NFormItem v-if="canUseOAuth" label="OAuth 授权">
              <NButton
                type="primary"
                @click="startOAuthLogin"
                :disabled="loading"
              >
                使用 {{ form.provider === 'gmail' ? 'Google' : 'Microsoft' }} 账号登录
              </NButton>
            </NFormItem>

            <!-- 密码修改 -->
            <div class="section-title">修改密码</div>

            <NFormItem label="新密码">
              <NInput
                v-model:value="form.password"
                type="password"
                placeholder="留空则不修改"
                show-password-on="click"
                :disabled="loading"
              />
            </NFormItem>

            <p class="hint-text">
              如果使用 OAuth 认证，无需设置密码。
            </p>
          </NForm>
        </NTabPane>
      </NTabs>

      <!-- 操作按钮 -->
      <div class="modal-actions">
        <div class="left-actions">
          <NPopconfirm
            @positive-click="handleTestConnection"
            positive-text="确认测试"
            negative-text="取消"
          >
            <template #trigger>
              <NButton
                :loading="testing"
                :disabled="loading"
              >
                测试连接
              </NButton>
            </template>
            <div>测试邮件服务器连接是否正常</div>
          </NPopconfirm>

          <NPopconfirm
            @positive-click="handleDelete"
            positive-text="确认删除"
            negative-text="取消"
          >
            <template #trigger>
              <NButton
                type="error"
                :disabled="loading"
              >
                删除账号
              </NButton>
            </template>
            <div>删除后无法恢复，确认删除此账号？</div>
          </NPopconfirm>
        </div>

        <div class="right-actions">
          <NButton @click="emit('update:show', false)" :disabled="loading">
            取消
          </NButton>
          <NButton type="primary" @click="handleUpdate" :loading="loading">
            保存更改
          </NButton>
        </div>
      </div>

      <!-- 测试结果 -->
      <div v-if="testResult" class="test-result">
        <h4>连接测试结果</h4>
        <div class="result-item">
          <span class="label">连接状态:</span>
          <span :class="testResult.success ? 'success' : 'error'">
            {{ testResult.success ? '成功' : '失败' }}
          </span>
        </div>
        <div v-if="testResult.connect_time" class="result-item">
          <span class="label">连接耗时:</span>
          <span>{{ testResult.connect_time }} ms</span>
        </div>
        <div v-if="testResult.login_time" class="result-item">
          <span class="label">登录耗时:</span>
          <span>{{ testResult.login_time }} ms</span>
        </div>
        <div v-if="testResult.email_count !== undefined" class="result-item">
          <span class="label">邮件数量:</span>
          <span>{{ testResult.email_count }}</span>
        </div>
      </div>
    </div>

    <!-- OAuth 登录弹窗 -->
    <OAuthLoginModal
      :show="showOAuthModal"
      :provider="oauthProvider"
      @update:show="showOAuthModal = $event"
      @success="handleOAuthSuccess"
    />
  </NModal>
</template>

<style scoped>
.account-settings {
  padding: 0;
}

.account-header {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 20px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  margin-bottom: 24px;
}

.account-avatar {
  width: 56px;
  height: 56px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: bold;
  color: white;
}

.account-info h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
}

.account-info p {
  margin: 4px 0 0 0;
  color: var(--text-color-2);
  font-size: 14px;
}

.color-options {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}

.color-option {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  cursor: pointer;
  border: 2px solid transparent;
  transition: all 0.15s ease;
}

.color-option:hover {
  transform: scale(1.1);
}

.color-option.active {
  border-color: var(--text-primary);
  box-shadow: 0 0 0 2px var(--bg-elevated);
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-color-2);
  margin: 20px 0 12px 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.preset-info {
  padding: 16px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  color: var(--text-color-2);
}

.preset-info p {
  margin: 4px 0;
}

.auth-type-display {
  display: flex;
  align-items: center;
}

.auth-badge {
  padding: 4px 12px;
  border-radius: var(--radius-sm);
  font-size: 12px;
  font-weight: 600;
}

.auth-badge.oauth {
  background: rgba(16, 185, 129, 0.1);
  color: #10B981;
}

.auth-badge.password {
  background: rgba(59, 130, 246, 0.1);
  color: #3B82F6;
}

.hint-text {
  margin: 12px 0 0 0;
  padding: 12px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  font-size: 13px;
  color: var(--text-color-2);
}

.modal-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid var(--border-color);
}

.left-actions {
  display: flex;
  gap: 12px;
}

.right-actions {
  display: flex;
  gap: 12px;
}

.test-result {
  margin-top: 20px;
  padding: 16px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
}

.test-result h4 {
  margin: 0 0 12px 0;
  font-size: 14px;
  font-weight: 600;
}

.result-item {
  display: flex;
  justify-content: space-between;
  padding: 6px 0;
  font-size: 13px;
}

.result-item .label {
  color: var(--text-color-2);
}

.result-item .success {
  color: #10B981;
  font-weight: 600;
}

.result-item .error {
  color: #F43F5E;
  font-weight: 600;
}

.mb-4 {
  margin-bottom: 16px;
}
</style>
