<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useUIStore, useAccountStore } from '@/stores'
import { invoke } from '@tauri-apps/api/core'
import { NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NButton, NSwitch, NAlert, NRadioGroup, NRadio } from 'naive-ui'
import OAuthLoginModal from './OAuthLoginModal.vue'
import { extractAutoFillInfo, parseEmail, detectProviderFromEmail } from '@/utils/emailHelper'

const uiStore = useUIStore()
const accountStore = useAccountStore()

const emit = defineEmits<{
  (e: 'success'): void
}>()

const providerOptions = [
  { label: 'Gmail', value: 'gmail' },
  { label: 'Outlook / Hotmail', value: 'outlook' },
  { label: 'iCloud Mail', value: 'icloud' },
  { label: 'Yahoo Mail', value: 'yahoo' },
  { label: '自定义 IMAP/SMTP', value: 'imap' },
]

const show = computed(() => uiStore.modals.addAccount)

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
  authType: 'password' as 'password' | 'oauth',
})

const loading = ref(false)
const error = ref('')
const success = ref('')

// 自动判断相关状态
const isProviderManuallySet = ref(false) // 是否由用户手动设置的服务商
const isAuthTypeManuallySet = ref(false) // 是否由用户手动修改了认证方式
const lastValidEmail = ref('') // 上一次有效的邮箱地址

// OAuth 相关
const showOAuthModal = ref(false)
const oauthProvider = ref<'microsoft' | 'google'>('microsoft')
const oauthToken = ref<{ access_token: string, refresh_token: string, expires_at: number } | null>(null)

const isCustom = computed(() => form.value.provider === 'imap')
const canUseOAuth = computed(() => {
  return form.value.provider === 'outlook' || form.value.provider === 'hotmail' || form.value.provider === 'gmail'
})

// 监听 provider 变化，自动设置邮箱前缀
watch(() => form.value.provider, (newProvider, oldProvider) => {
  // 如果是程序自动触发的（由邮箱地址变化导致），不标记为手动设置
  if (lastValidEmail.value) {
    const detected = detectProviderFromEmail(lastValidEmail.value)
    if (detected === newProvider) {
      // 这是自动触发的，不标记为手动
      return
    }
  }

  // 用户手动改变了服务商
  if (oldProvider && newProvider !== oldProvider) {
    isProviderManuallySet.value = true

    // 清空自定义服务器配置（如果切换回已知服务商）
    if (newProvider !== 'imap') {
      form.value.imapHost = ''
      form.value.smtpHost = ''
    }

    // 自动切换认证方式（Gmail/Outlook/Yahoo → oauth，其他 → password）
    const recommendedAuthType = (newProvider === 'gmail' || newProvider === 'outlook' || newProvider === 'yahoo')
      ? 'oauth' as const
      : 'password' as const
    if (form.value.authType !== recommendedAuthType) {
      form.value.authType = recommendedAuthType
    }
  }

  // 原有的自动填充邮箱前缀逻辑保持不变
  if (!form.value.email.includes('@')) {
    switch (newProvider) {
      case 'gmail':
        form.value.email = '@gmail.com'
        break
      case 'outlook':
      case 'hotmail':
        form.value.email = '@outlook.com'
        break
      case 'icloud':
        form.value.email = '@icloud.com'
        break
      case 'yahoo':
        form.value.email = '@yahoo.com'
        break
    }
  }
})

// 监听邮箱地址变化，自动判断服务商和填充信息
watch(() => form.value.email, (newEmail) => {
  // 如果用户手动设置了服务商，不再自动判断
  if (isProviderManuallySet.value) {
    return
  }

  // 如果用户手动修改了认证方式，不再自动更改认证方式
  const shouldUpdateAuthType = !isAuthTypeManuallySet.value

  // 邮箱地址不完整，不触发
  if (!newEmail || !newEmail.includes('@')) {
    return
  }

  const { isValid } = parseEmail(newEmail)
  if (!isValid) {
    return
  }

  // 提取自动填充信息
  const autoFillInfo = extractAutoFillInfo(newEmail, form.value.name)

  // 自动设置服务商
  if (autoFillInfo.provider !== form.value.provider) {
    form.value.provider = autoFillInfo.provider
  }

  // 自动设置认证方式（仅在用户未手动修改时）
  if (shouldUpdateAuthType && autoFillInfo.authType !== form.value.authType) {
    form.value.authType = autoFillInfo.authType
  }

  // 自动填充账号名称（仅在名称为空时）
  if (!form.value.name.trim() && autoFillInfo.name) {
    form.value.name = autoFillInfo.name
  }

  // 如果是自定义服务商，自动配置服务器
  if (autoFillInfo.isCustom && autoFillInfo.serverConfig) {
    Object.assign(form.value, {
      imapHost: autoFillInfo.serverConfig.imapHost,
      smtpHost: autoFillInfo.serverConfig.smtpHost,
      imapPort: autoFillInfo.serverConfig.imapPort,
      smtpPort: autoFillInfo.serverConfig.smtpPort,
      imapSsl: autoFillInfo.serverConfig.imapSsl,
      smtpSsl: autoFillInfo.serverConfig.smtpSsl,
    })
  }

  lastValidEmail.value = newEmail
})

// 监听 IMAP SSL 变化，自动切换端口
watch(() => form.value.imapSsl, (newSsl) => {
  // 只有当端口是默认值时才自动切换
  if (newSsl && (form.value.imapPort === 143)) {
    form.value.imapPort = 993
  } else if (!newSsl && (form.value.imapPort === 993)) {
    form.value.imapPort = 143
  }
})

// 监听 SMTP SSL 变化，自动切换端口
watch(() => form.value.smtpSsl, (newSsl) => {
  if (newSsl && (form.value.smtpPort === 25 || form.value.smtpPort === 587)) {
    form.value.smtpPort = 465
  } else if (!newSsl && (form.value.smtpPort === 465)) {
    form.value.smtpPort = 25
  }
})

// 监听 show 变化，重置表单
watch(show, (newShow) => {
  if (!newShow) {
    error.value = ''
    success.value = ''
    oauthToken.value = null
    // 重置自动判断状态
    isProviderManuallySet.value = false
    isAuthTypeManuallySet.value = false
    lastValidEmail.value = ''
  } else {
    // 打开时，重置为默认状态
    isProviderManuallySet.value = false
    isAuthTypeManuallySet.value = false
    lastValidEmail.value = ''
  }
})

// 处理邮箱输入框失焦，更新服务器配置
function handleEmailBlur() {
  const email = form.value.email

  // 邮箱地址不完整，不触发
  if (!email || !email.includes('@')) {
    return
  }

  const { isValid } = parseEmail(email)
  if (!isValid) {
    return
  }

  // 提取自动填充信息
  const autoFillInfo = extractAutoFillInfo(email, form.value.name)

  // 只更新服务器配置，不更新服务商和认证方式
  if (autoFillInfo.isCustom && autoFillInfo.serverConfig) {
    // 只有当当前是自定义服务商，或者服务器配置为空时才更新
    if (form.value.provider === 'imap' || !form.value.imapHost || !form.value.smtpHost) {
      Object.assign(form.value, {
        imapHost: autoFillInfo.serverConfig.imapHost,
        smtpHost: autoFillInfo.serverConfig.smtpHost,
        imapPort: autoFillInfo.serverConfig.imapPort,
        smtpPort: autoFillInfo.serverConfig.smtpPort,
        imapSsl: autoFillInfo.serverConfig.imapSsl,
        smtpSsl: autoFillInfo.serverConfig.smtpSsl,
      })
    }
  }
}

async function handleSubmit() {
  error.value = ''

  if (!form.value.name.trim()) {
    error.value = '请输入账号名称'
    return
  }

  if (!form.value.email.trim() || form.value.email.startsWith('@')) {
    error.value = '请输入完整的邮箱地址'
    return
  }

  // OAuth 模式下，如果还没有 token，不能提交
  if (form.value.authType === 'oauth' && !oauthToken.value) {
    error.value = '请先完成 OAuth 授权'
    return
  }

  // 密码模式下，必须输入密码
  if (form.value.authType === 'password' && !form.value.password.trim()) {
    error.value = '请输入密码'
    return
  }

  loading.value = true

  try {
    const accountData: any = {
      name: form.value.name,
      email: form.value.email,
      provider: form.value.provider,
      color: form.value.color,
    }

    if (form.value.authType === 'oauth' && oauthToken.value) {
      // OAuth 模式
      accountData.auth_type = 'oauth'
      accountData.oauth_provider = oauthProvider.value
      accountData.oauth_token = oauthToken.value.access_token
      accountData.oauth_refresh_token = oauthToken.value.refresh_token
      accountData.password = '' // OAuth 不需要密码
    } else {
      // 密码模式
      accountData.password = form.value.password
    }

    // 自定义配置
    if (isCustom.value) {
      accountData.imap_host = form.value.imapHost
      accountData.imap_port = form.value.imapPort
      accountData.imap_ssl = form.value.imapSsl
      accountData.smtp_host = form.value.smtpHost
      accountData.smtp_port = form.value.smtpPort
      accountData.smtp_ssl = form.value.smtpSsl
    }

    await invoke('add_account', {
      account: accountData
    })

    success.value = '账号添加成功！'
    setTimeout(async () => {
      // 刷新账号列表
      await accountStore.fetchAccounts()
      emit('success')
      uiStore.closeAddAccountModal()
      resetForm()
    }, 1000)
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

function resetForm() {
  form.value = {
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
    authType: 'password',
  }
  oauthToken.value = null
  success.value = ''
  error.value = ''
}

function startOAuthLogin() {
  if (form.value.provider === 'outlook' || form.value.provider === 'hotmail') {
    oauthProvider.value = 'microsoft'
  } else if (form.value.provider === 'gmail') {
    oauthProvider.value = 'google'
  }
  showOAuthModal.value = true
}

function handleOAuthSuccess(token: { access_token: string, refresh_token: string, expires_at: number }) {
  oauthToken.value = token
  success.value = 'OAuth 授权成功！'
  error.value = ''
}
</script>

<template>
  <NModal
    v-model:show="show"
    preset="card"
    title="添加邮箱账号"
    :style="{ width: '500px' }"
    :mask-closable="!loading"
    :close-on-esc="!loading"
    @update:show="uiStore.closeAddAccountModal"
  >
    <NForm @submit.prevent="handleSubmit">
      <!-- 账号名称 -->
      <NFormItem label="账号名称" path="name" :show-require-mark="true">
        <NInput
          v-model:value="form.name"
          placeholder="工作邮箱"
          :disabled="loading"
        />
      </NFormItem>

      <!-- 邮箱地址 -->
      <NFormItem label="邮箱地址" path="email" :show-require-mark="true">
        <NInput
          v-model:value="form.email"
          placeholder="user@example.com"
          :disabled="loading"
          @blur="handleEmailBlur"
        />
      </NFormItem>

      <!-- 服务商 -->
      <NFormItem label="邮箱服务商" path="provider" :show-require-mark="true">
        <NSelect
          v-model:value="form.provider"
          :options="providerOptions"
          :disabled="loading"
        />
      </NFormItem>

      <!-- 认证方式 -->
      <NFormItem label="认证方式" v-if="canUseOAuth">
        <NRadioGroup v-model:value="form.authType" :disabled="loading">
          <NRadio value="password">密码登录</NRadio>
          <NRadio value="oauth">OAuth 2.0 授权</NRadio>
        </NRadioGroup>
      </NFormItem>

      <!-- OAuth 登录按钮 -->
      <template v-if="canUseOAuth && form.authType === 'oauth'">
        <NFormItem>
          <NButton
            type="primary"
            @click="startOAuthLogin"
            :disabled="loading"
            block
          >
            使用 {{ form.provider === 'gmail' ? 'Google' : 'Microsoft' }} 账号授权
          </NButton>
        </NFormItem>

        <!-- OAuth 成功提示 -->
        <NAlert v-if="oauthToken" type="success" :title="success">
          授权成功！token 已获取，可以添加账号了。
        </NAlert>
      </template>

      <!-- 密码输入 -->
      <NFormItem
        v-if="form.authType === 'password'"
        label="密码"
        path="password"
        :show-require-mark="true"
      >
        <NInput
          v-model:value="form.password"
          type="password"
          placeholder="邮箱密码或应用专用密码"
          show-password-on="click"
          :disabled="loading"
        />
      </NFormItem>

      <!-- 自定义 IMAP/SMTP 配置 -->
      <template v-if="isCustom">
        <NFormItem label="IMAP 服务器">
          <NInput
            v-model:value="form.imapHost"
            placeholder="imap.example.com"
            :disabled="loading"
            style="flex: 1"
          />
        </NFormItem>

        <NFormItem label="IMAP 端口和加密">
          <div style="display: flex; gap: 12px; align-items: center; width: 100%">
            <NInputNumber
              v-model:value="form.imapPort"
              :min="1"
              :max="65535"
              placeholder="993"
              :disabled="loading"
              style="flex: 1"
            />
            <NSwitch
              v-model:value="form.imapSsl"
              :disabled="loading"
            >
              <template #checked>SSL</template>
              <template #unchecked>非加密</template>
            </NSwitch>
            <span style="color: var(--text-tertiary); font-size: 12px">
              {{ form.imapSsl ? '993' : '143' }}
            </span>
          </div>
        </NFormItem>

        <NFormItem label="SMTP 服务器">
          <NInput
            v-model:value="form.smtpHost"
            placeholder="smtp.example.com"
            :disabled="loading"
            style="flex: 1"
          />
        </NFormItem>

        <NFormItem label="SMTP 端口和加密">
          <div style="display: flex; gap: 12px; align-items: center; width: 100%">
            <NInputNumber
              v-model:value="form.smtpPort"
              :min="1"
              :max="65535"
              placeholder="465"
              :disabled="loading"
              style="flex: 1"
            />
            <NSwitch
              v-model:value="form.smtpSsl"
              :disabled="loading"
            >
              <template #checked>SSL</template>
              <template #unchecked>非加密</template>
            </NSwitch>
            <span style="color: var(--text-tertiary); font-size: 12px">
              {{ form.smtpSsl ? '465' : '25' }}
            </span>
          </div>
        </NFormItem>
      </template>

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

      <!-- 提示信息 -->
      <NAlert v-if="success" type="success" :title="success" closable @close="success = ''" />
      <div v-if="error" class="error-message">
        {{ error }}
      </div>

      <!-- 操作按钮 -->
      <div class="form-actions">
        <NButton @click="uiStore.closeAddAccountModal" :disabled="loading">
          取消
        </NButton>
        <NButton type="primary" attr-type="submit" :loading="loading" :disabled="form.authType === 'oauth' && !oauthToken">
          添加
        </NButton>
      </div>
    </NForm>

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
.color-options {
  display: flex;
  gap: 8px;
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

.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}

.error-message {
  padding: 12px;
  background: rgba(244, 63, 78, 0.1);
  border: 1px solid rgba(244, 63, 78, 0.3);
  border-radius: var(--radius-md);
  color: var(--error);
  margin-bottom: 16px;
}
</style>
