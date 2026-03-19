<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import {
  NModal, NForm, NFormItem, NInput, NInputNumber, NSelect,
  NButton, NSwitch, NAlert, NPopconfirm, NTabs, NTabPane,
  NTag, NTooltip, NCollapse, NCollapseItem
} from 'naive-ui'
import { AccountType, AuthType } from '@/types'
import { useAccountStore } from '@/stores/account'
import { OAuthHelper } from '@/utils/oauthHelper'

const props = defineProps<{
  show: boolean
  account: {
    id: string | number
    name: string
    email: string
    provider: string
    accountType?: string
    authType?: string
    imapHost?: string
    imapPort?: number
    imapSsl?: boolean
    smtpHost?: string
    smtpPort?: number
    smtpSsl?: boolean
    color?: string
    syncEnabled?: boolean
    oauthProvider?: string
    oauthTokenExpiry?: number
    enterpriseTenantId?: string
    enterpriseDomain?: string
  } | null
}>()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
  (e: 'updated'): void
  (e: 'deleted'): void
}>()

const accountStore = useAccountStore()

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

// 表单数据
const form = ref({
  name: '',
  email: '',
  provider: 'gmail',
  accountType: AccountType.Personal,
  authType: AuthType.Password,
  imapHost: '',
  imapPort: 993,
  imapSsl: true,
  smtpHost: '',
  smtpPort: 587,
  smtpSsl: true,
  color: '#7C3AED',
  syncEnabled: true,
  // 企业配置
  enterpriseTenantId: '',
  enterpriseDomain: '',
})

const isCustom = computed(() => form.value.provider === 'imap')
const isEnterprise = computed(() => form.value.accountType === AccountType.Enterprise)
const canUseOAuth = computed(() => {
  return ['gmail', 'outlook'].includes(form.value.provider)
})

// OAuth Token 信息
const oauthTokenInfo = computed(() => {
  if (!props.account?.oauthTokenExpiry) return null

  const expiryDate = new Date(props.account.oauthTokenExpiry)
  const now = new Date()
  const daysUntilExpiry = Math.floor((expiryDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24))

  return {
    expiryDate,
    daysUntilExpiry,
    isExpired: expiryDate < now,
    isExpiringSoon: daysUntilExpiry <= 7 && daysUntilExpiry >= 0,
  }
})

// 监听 account 变化，填充表单
watch(() => props.account, (newAccount) => {
  if (newAccount) {
    form.value = {
      name: newAccount.name || '',
      email: newAccount.email || '',
      provider: newAccount.provider || 'gmail',
      accountType: (newAccount.accountType as AccountType) || AccountType.Personal,
      authType: (newAccount.authType as AuthType) || AuthType.Password,
      imapHost: newAccount.imapHost || '',
      imapPort: newAccount.imapPort || 993,
      imapSsl: newAccount.imapSsl ?? true,
      smtpHost: newAccount.smtpHost || '',
      smtpPort: newAccount.smtpPort || 587,
      smtpSsl: newAccount.smtpSsl ?? true,
      color: newAccount.color || '#7C3AED',
      syncEnabled: newAccount.syncEnabled ?? true,
      enterpriseTenantId: newAccount.enterpriseTenantId || '',
      enterpriseDomain: newAccount.enterpriseDomain || '',
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
    await accountStore.updateAccount(String(props.account?.id), {
      name: form.value.name,
      email: form.value.email,
      provider: form.value.provider as any,
      accountType: form.value.accountType,
      authType: form.value.authType,
      imapHost: isCustom.value ? form.value.imapHost : undefined,
      imapPort: isCustom.value ? form.value.imapPort : undefined,
      imapSsl: isCustom.value ? form.value.imapSsl : undefined,
      smtpHost: isCustom.value ? form.value.smtpHost : undefined,
      smtpPort: isCustom.value ? form.value.smtpPort : undefined,
      smtpSsl: isCustom.value ? form.value.smtpSsl : undefined,
      color: form.value.color,
      enterpriseTenantId: isEnterprise.value ? form.value.enterpriseTenantId : undefined,
      enterpriseDomain: isEnterprise.value ? form.value.enterpriseDomain : undefined,
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
    const result = await accountStore.testConnection({
      email: form.value.email,
      provider: form.value.provider as any,
      imapHost: isCustom.value ? form.value.imapHost : undefined,
      imapPort: isCustom.value ? form.value.imapPort : undefined,
      imapSsl: isCustom.value ? form.value.imapSsl : undefined,
      smtpHost: isCustom.value ? form.value.smtpHost : undefined,
      smtpPort: isCustom.value ? form.value.smtpPort : undefined,
      smtpSsl: isCustom.value ? form.value.smtpSsl : undefined,
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
    await accountStore.removeAccount(String(props.account?.id))
    emit('deleted')
    emit('update:show', false)
  } catch (e: any) {
    error.value = String(e)
    loading.value = false
  }
}

// OAuth 登录
async function startOAuthLogin() {
  const provider = form.value.provider

  try {
    await OAuthHelper.startLogin(provider)
    success.value = 'OAuth 授权成功！'
    form.value.authType = AuthType.OAuth2
  } catch (e: any) {
    error.value = String(e)
  }
}

// 刷新 OAuth Token
async function handleRefreshToken() {
  loading.value = true
  try {
    const result = await OAuthHelper.refreshTokens(String(props.account?.id))
    if (result) {
      success.value = 'Token 刷新成功！'
      // 重新加载账号信息以更新过期时间
      await accountStore.fetchAccounts()
    } else {
      error.value = 'Token 刷新失败，请重新进行 OAuth 授权'
    }
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

// 格式化过期时间
function formatExpiryDate(timestamp: number): string {
  const date = new Date(timestamp)
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// 获取认证类型标签
function getAuthTypeLabel(authType: AuthType): string {
  const labels: Record<AuthType, string> = {
    [AuthType.Password]: '密码',
    [AuthType.OAuth2]: 'OAuth 2.0',
    [AuthType.AppPassword]: '应用密码',
    [AuthType.DomainAuth]: '域认证',
    [AuthType.SamlSso]: 'SAML SSO',
  }
  return labels[authType] || authType
}

// 获取认证类型样式
function getAuthTypeClass(authType: AuthType): string {
  if (authType === AuthType.OAuth2) return 'oauth'
  if (authType === AuthType.SamlSso) return 'saml'
  return 'password'
}
</script>

<template>
  <NModal
    :show="show"
    @update:show="emit('update:show', $event)"
    preset="card"
    title="账号设置"
    :style="{ width: '650px' }"
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
          <div class="account-badges">
            <NTag :type="isEnterprise ? 'warning' : 'default'" size="small">
              {{ isEnterprise ? '企业邮箱' : '个人邮箱' }}
            </NTag>
            <NTag size="small" :class="getAuthTypeClass(form.authType)">
              {{ getAuthTypeLabel(form.authType) }}
            </NTag>
          </div>
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
            <!-- 账号类型 -->
            <NFormItem label="账号类型">
              <NRadioGroup v-model:value="form.accountType">
                <NRadioButton :value="AccountType.Personal">个人邮箱</NRadioButton>
                <NRadioButton :value="AccountType.Enterprise">企业邮箱</NRadioButton>
              </NRadioGroup>
            </NFormItem>

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
                  { label: 'Outlook', value: 'outlook' },
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

        <!-- 企业配置 -->
        <NTabPane name="enterprise" tab="企业配置" :disabled="!isEnterprise">
          <NForm v-if="isEnterprise">
            <NAlert type="info" class="mb-4">
              配置企业邮箱服务器信息。根据不同的邮件服务商，配置可能有所不同。
            </NAlert>

            <NFormItem label="租户 ID (Microsoft 365)">
              <NInput
                v-model:value="form.enterpriseTenantId"
                placeholder="contoso.onmicrosoft.com"
                :disabled="loading"
              />
              <template #feedback>
                Microsoft 365 租户 ID，例如: contoso.onmicrosoft.com
              </template>
            </NFormItem>

            <NFormItem label="组织域名">
              <NInput
                v-model:value="form.enterpriseDomain"
                placeholder="example.com"
                :disabled="loading"
              />
              <template #feedback>
                组织的主域名，用于邮箱地址（如 user@example.com）
              </template>
            </NFormItem>
          </NForm>

          <div v-else class="empty-enterprise">
            <span class="icon">🏢</span>
            <p>请将账号类型设置为"企业邮箱"以配置企业选项</p>
          </div>
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
              <NCollapse>
                <NCollapseItem title="查看服务器配置">
                  <template v-if="form.provider === 'gmail'">
                    <p><strong>IMAP:</strong> imap.gmail.com:993 (SSL)</p>
                    <p><strong>SMTP:</strong> smtp.gmail.com:587 (SSL)</p>
                  </template>
                  <template v-else-if="form.provider === 'outlook'">
                    <p><strong>IMAP:</strong> outlook.office365.com:993 (SSL)</p>
                    <p><strong>SMTP:</strong> smtp.office365.com:587 (SSL)</p>
                  </template>
                  <template v-else-if="form.provider === 'icloud'">
                    <p><strong>IMAP:</strong> imap.mail.me.com:993 (SSL)</p>
                    <p><strong>SMTP:</strong> smtp.mail.me.com:587 (SSL)</p>
                  </template>
                </NCollapseItem>
              </NCollapse>
            </div>
          </NForm>
        </NTabPane>

        <!-- 认证设置 -->
        <NTabPane name="auth" tab="认证设置">
          <NForm>
            <div class="section-title">认证方式</div>

            <NFormItem label="当前认证方式">
              <div class="auth-type-display">
                <NTag :type="form.authType === AuthType.OAuth2 ? 'success' : 'default'" size="medium">
                  {{ getAuthTypeLabel(form.authType) }}
                </NTag>
              </div>
            </NFormItem>

            <!-- OAuth Token 信息 -->
            <template v-if="form.authType === AuthType.OAuth2 && oauthTokenInfo">
              <div class="section-title">OAuth Token</div>

              <NFormItem label="Token 状态">
                <div v-if="oauthTokenInfo.isExpired" class="token-status expired">
                  <span class="icon">⚠</span>
                  <span>Token 已过期</span>
                </div>
                <div v-else-if="oauthTokenInfo.isExpiringSoon" class="token-status expiring">
                  <span class="icon">⏰</span>
                  <span>Token 即将过期 ({{ oauthTokenInfo.daysUntilExpiry }} 天后)</span>
                </div>
                <div v-else class="token-status valid">
                  <span class="icon">✓</span>
                  <span>Token 有效 ({{ oauthTokenInfo.daysUntilExpiry }} 天后过期)</span>
                </div>
              </NFormItem>

              <NFormItem label="过期时间">
                <NTooltip>
                  <template #trigger>
                    <span>{{ formatExpiryDate(props.account?.oauthTokenExpiry || 0) }}</span>
                  </template>
                  {{ new Date(props.account?.oauthTokenExpiry || 0).toLocaleString('zh-CN') }}
                </NTooltip>
              </NFormItem>

              <NFormItem label="操作">
                <NButton
                  type="primary"
                  :disabled="loading"
                  @click="handleRefreshToken"
                >
                  刷新 Token
                </NButton>
              </NFormItem>
            </template>

            <!-- OAuth 登录按钮 -->
            <NFormItem v-if="canUseOAuth && form.authType !== AuthType.OAuth2" label="OAuth 授权">
              <NButton
                type="primary"
                @click="startOAuthLogin"
                :disabled="loading"
              >
                使用 {{ form.provider === 'gmail' ? 'Google' : 'Microsoft' }} 账号登录
              </NButton>
              <template #feedback>
                切换到 OAuth 2.0 认证，无需存储密码
              </template>
            </NFormItem>

            <!-- 密码修改提示 -->
            <template v-if="form.authType === AuthType.Password || form.authType === AuthType.AppPassword">
              <NAlert type="info" :bordered="false">
                <template #header>
                  修改密码
                </template>
                <template v-if="form.authType === AuthType.AppPassword">
                  应用密码需要在邮箱服务提供商处生成。要修改密码，请删除账号后重新添加。
                </template>
                <template v-else>
                  要修改账号密码，请删除账号后重新添加。或者使用 OAuth 2.0 认证，无需存储密码。
                </template>
              </NAlert>
            </template>
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
  background: var(--n-color-card);
  border-radius: 8px;
  margin-bottom: 24px;
  border: 1px solid var(--n-border-color);
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
  flex-shrink: 0;
}

.account-info {
  flex: 1;
  min-width: 0;
}

.account-info h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--n-text-color);
}

.account-info p {
  margin: 4px 0 0 0;
  color: var(--n-text-color-3);
  font-size: 14px;
}

.account-badges {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.account-badges :deep(.n-tag) {
  font-size: 11px;
}

.account-badges .oauth {
  background: rgba(16, 185, 129, 0.1);
  color: #10B981;
  border-color: #10B981;
}

.account-badges .saml {
  background: rgba(139, 92, 246, 0.1);
  color: #8B5CF6;
  border-color: #8B5CF6;
}

.account-badges .password {
  background: rgba(59, 130, 246, 0.1);
  color: #3B82F6;
  border-color: #3B82F6;
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
  border-color: var(--n-text-color);
  box-shadow: 0 0 0 2px var(--n-card-color);
}

.section-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--n-text-color-2);
  margin: 20px 0 12px 0;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.empty-enterprise {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  color: var(--n-text-color-3);
  text-align: center;
}

.empty-enterprise .icon {
  font-size: 48px;
  margin-bottom: 12px;
}

.preset-info {
  padding: 16px;
  background: var(--n-color-card);
  border-radius: 8px;
  color: var(--n-text-color-2);
  border: 1px solid var(--n-border-color);
}

.preset-info p {
  margin: 4px 0;
}

.auth-type-display {
  display: flex;
  align-items: center;
}

.token-status {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 13px;
}

.token-status.valid {
  background: rgba(16, 185, 129, 0.1);
  color: #10B981;
}

.token-status.expiring {
  background: rgba(245, 158, 11, 0.1);
  color: #F59E0B;
}

.token-status.expired {
  background: rgba(244, 63, 94, 0.1);
  color: #F43F5E;
}

.hint-text {
  margin: 12px 0 0 0;
  padding: 12px;
  background: var(--n-color-card);
  border-radius: 6px;
  font-size: 13px;
  color: var(--n-text-color-2);
  border: 1px solid var(--n-border-color);
}

.modal-actions {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid var(--n-border-color);
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
  background: var(--n-color-card);
  border-radius: 8px;
  border: 1px solid var(--n-border-color);
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
  color: var(--n-text-color-3);
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
