<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NButton, NSwitch, NAlert, NRadioGroup, NRadio } from 'naive-ui'
import OAuthLoginModal from './OAuthLoginModal.vue'

const emit = defineEmits<{
  (e: 'success'): void
  (e: 'update:show', value: boolean): void
}>()

const show = defineModel<boolean>('show', { default: false })

const providerOptions = [
  { label: 'Gmail', value: 'gmail' },
  { label: 'Outlook / Hotmail', value: 'outlook' },
  { label: 'iCloud Mail', value: 'icloud' },
  { label: 'Yahoo Mail', value: 'yahoo' },
  { label: '自定义 IMAP/SMTP', value: 'imap' },
]

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

// OAuth 相关
const showOAuthModal = ref(false)
const oauthProvider = ref<'microsoft' | 'google'>('microsoft')
const oauthToken = ref<{ access_token: string, refresh_token: string, expires_at: number } | null>(null)

const isCustom = computed(() => form.value.provider === 'imap')
const canUseOAuth = computed(() => {
  return form.value.provider === 'outlook' || form.value.provider === 'hotmail' || form.value.provider === 'gmail'
})

// 监听 provider 变化，自动设置邮箱前缀
watch(() => form.value.provider, (newProvider) => {
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

// 监听 show 变化，重置表单
watch(show, (newShow) => {
  if (!newShow) {
    error.value = ''
    success.value = ''
    oauthToken.value = null
  }
})

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
    setTimeout(() => {
      emit('success')
      emit('update:show', false)
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
        <NButton @click="emit('update:show', false)" :disabled="loading">
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
