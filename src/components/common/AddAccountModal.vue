<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NButton, NSwitch } from 'naive-ui'

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
})

const loading = ref(false)
const error = ref('')

const isCustom = computed(() => form.value.provider === 'imap')

async function handleSubmit() {
  error.value = ''

  if (!form.value.name.trim()) {
    error.value = '请输入账号名称'
    return
  }

  if (!form.value.email.trim()) {
    error.value = '请输入邮箱地址'
    return
  }

  if (!form.value.password.trim()) {
    error.value = '请输入密码'
    return
  }

  loading.value = true

  try {
    await invoke('add_account', {
      account: {
        name: form.value.name,
        email: form.value.email,
        provider: form.value.provider,
        password: form.value.password,
        imapHost: isCustom.value ? form.value.imapHost : undefined,
        imapPort: isCustom.value ? form.value.imapPort : undefined,
        imapSsl: isCustom.value ? form.value.imapSsl : undefined,
        smtpHost: isCustom.value ? form.value.smtpHost : undefined,
        smtpPort: isCustom.value ? form.value.smtpPort : undefined,
        smtpSsl: isCustom.value ? form.value.smtpSsl : undefined,
        color: form.value.color,
      }
    })

    emit('success')
    emit('update:show', false)

    // 重置表单
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
    }
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
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

      <!-- 密码 -->
      <NFormItem label="密码" path="password" :show-require-mark="true">
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

      <!-- 错误提示 -->
      <div v-if="error" class="error-message">
        {{ error }}
      </div>

      <!-- 操作按钮 -->
      <div class="form-actions">
        <NButton @click="emit('update:show', false)" :disabled="loading">
          取消
        </NButton>
        <NButton type="primary" attr-type="submit" :loading="loading">
          添加
        </NButton>
      </div>
    </NForm>
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
