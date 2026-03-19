<template>
  <div class="enterprise-config-form">
    <!-- 标题 -->
    <div class="form-header">
      <h4>企业邮箱配置</h4>
      <p class="description">配置 Microsoft 365 或 Google Workspace 企业邮箱</p>
    </div>

    <!-- 表单 -->
    <NForm ref="formRef" :model="form" :rules="rules" label-placement="top">
      <!-- 邮件服务商 -->
      <NFormItem label="邮件服务商" path="provider">
        <NRadioGroup v-model:value="form.provider" @update:value="onProviderChange">
          <NRadio value="microsoft365">Microsoft 365</NRadio>
          <NRadio value="google">Google Workspace</NRadio>
        </NRadioGroup>
      </NFormItem>

      <!-- Microsoft 365 配置 -->
      <template v-if="form.provider === 'microsoft365'">
        <NFormItem label="租户 ID" path="tenantId">
          <NInput
            v-model:value="form.tenantId"
            placeholder="contoso.onmicrosoft.com"
            @input="updatePreview"
          >
            <template #prefix>
              <span class="input-icon">🏢</span>
            </template>
          </NInput>
          <template #feedback>
            Microsoft 365 租户 ID，例如: contoso.onmicrosoft.com
          </template>
        </NFormItem>

        <NFormItem label="域名" path="domain">
          <NInput
            v-model:value="form.domain"
            placeholder="example.com"
            @input="updatePreview"
          >
            <template #prefix>
              <span class="input-icon">🌐</span>
            </template>
          </NInput>
          <template #feedback>
            组织的主域名，用于邮箱地址（如 user@example.com）
          </template>
        </NFormItem>
      </template>

      <!-- Google Workspace 配置 -->
      <template v-if="form.provider === 'google'">
        <NFormItem label="组织域名" path="domain">
          <NInput
            v-model:value="form.domain"
            placeholder="example.com"
            @input="updatePreview"
          >
            <template #prefix>
              <span class="input-icon">🌐</span>
            </template>
          </NInput>
          <template #feedback>
            Google Workspace 组织的主域名
          </template>
        </NFormItem>
      </template>

      <!-- 配置预览 -->
      <div v-if="preview.serverType" class="config-preview">
        <NAlert type="info" :bordered="false">
          <template #header>
            <span class="preview-header">配置预览</span>
          </template>
          <div class="preview-content">
            <div class="preview-item">
              <span class="label">IMAP 服务器:</span>
              <span class="value">{{ preview.imapHost }}:{{ preview.imapPort }}</span>
            </div>
            <div class="preview-item">
              <span class="label">SMTP 服务器:</span>
              <span class="value">{{ preview.smtpHost }}:{{ preview.smtpPort }}</span>
            </div>
            <div class="preview-item">
              <span class="label">SSL:</span>
              <span class="value">{{ preview.useSsl ? '是' : '否' }}</span>
            </div>
          </div>
        </NAlert>
      </div>

      <!-- 认证方式说明 -->
      <div class="auth-info">
        <NAlert type="warning" :bordered="false">
          <template #header>
            <span>认证方式</span>
          </template>
          <ul class="auth-methods">
            <li><strong>OAuth 2.0:</strong> 推荐，无需存储密码</li>
            <li><strong>应用密码:</strong> 需要在管理员控制台生成</li>
            <li><strong>SAML SSO:</strong> 适用于有 SAML SSO 配置的组织</li>
          </ul>
        </NAlert>
      </div>
    </NForm>

    <!-- 配置指南链接 -->
    <div class="config-guide">
      <NDropdown trigger="hover" :options="guideOptions" placement="bottom-start">
        <NButton text type="primary">
          <template #icon>
            <span>📖</span>
          </template>
          配置指南
        </NButton>
      </NDropdown>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import {
  NForm,
  NFormItem,
  NInput,
  NRadioGroup,
  NRadio,
  NAlert,
  NButton,
  NDropdown,
  type FormInst,
  type FormRules,
} from 'naive-ui'

interface EnterpriseConfig {
  provider: 'microsoft365' | 'google'
  tenantId?: string
  domain?: string
}

interface Props {
  modelValue?: EnterpriseConfig
}

interface Emits {
  (e: 'update:modelValue', value: EnterpriseConfig): void
  (e: 'change', value: EnterpriseConfig): void
}

const props = defineProps<Props>()
const emit = defineEmits<Emits>()

// 表单引用
const formRef = ref<FormInst | null>(null)

// 表单数据
const form = reactive<EnterpriseConfig>({
  provider: 'microsoft365',
  tenantId: '',
  domain: '',
})

// 表单验证规则
const rules: FormRules = {
  domain: [
    {
      required: true,
      message: '请输入组织域名',
      trigger: ['blur', 'input'],
    },
    {
      pattern: /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/,
      message: '请输入有效的域名',
      trigger: ['blur', 'input'],
    },
  ],
}

// 配置预览
const preview = reactive({
  serverType: '',
  imapHost: '',
  imapPort: 993,
  smtpHost: '',
  smtpPort: 587,
  useSsl: true,
})

// 配置指南选项
const guideOptions = [
  {
    label: 'Microsoft 365 配置指南',
    key: 'microsoft365',
    props: {
      onClick: () => {
        openGuide('https://learn.microsoft.com/zh-cn/exchange/clients-and-mobile-in-exchange-online/pop3-and-imap4')
      },
    },
  },
  {
    label: 'Google Workspace 配置指南',
    key: 'google',
    props: {
      onClick: () => {
        openGuide('https://support.google.com/a/answer/7124802')
      },
    },
  },
]

// ========================================
// Methods
// ========================================

/**
 * 更新配置预览
 */
function updatePreview() {
  // 重置预览
  preview.serverType = ''
  preview.imapHost = ''
  preview.smtpHost = ''

  if (form.provider === 'microsoft365') {
    preview.serverType = 'Microsoft 365'
    preview.imapHost = 'outlook.office365.com'
    preview.smtpHost = 'smtp.office365.com'
    preview.imapPort = 993
    preview.smtpPort = 587
    preview.useSsl = true
  } else if (form.provider === 'google') {
    preview.serverType = 'Google Workspace'
    preview.imapHost = 'imap.gmail.com'
    preview.smtpHost = 'smtp.gmail.com'
    preview.imapPort = 993
    preview.smtpPort = 587
    preview.useSsl = true
  }

  emitChange()
}

/**
 * 邮件服务商变化
 */
function onProviderChange() {
  // 清空租户 ID（仅 Microsoft 365 需要）
  if (form.provider === 'google') {
    form.tenantId = ''
  }

  updatePreview()
}

/**
 * 打开配置指南
 */
function openGuide(url: string) {
  window.open(url, '_blank')
}

/**
 * 触发变化事件
 */
function emitChange() {
  const value: EnterpriseConfig = {
    provider: form.provider,
    tenantId: form.tenantId,
    domain: form.domain,
  }
  emit('update:modelValue', value)
  emit('change', value)
}

/**
 * 验证表单
 */
async function validate(): Promise<boolean> {
  if (!formRef.value) return false

  try {
    await formRef.value.validate()
    return true
  } catch {
    return false
  }
}

/**
 * 获取表单值
 */
function getValue(): EnterpriseConfig {
  return {
    provider: form.provider,
    tenantId: form.tenantId,
    domain: form.domain,
  }
}

/**
 * 重置表单
 */
function reset() {
  form.provider = 'microsoft365'
  form.tenantId = ''
  form.domain = ''
  updatePreview()
}

// ========================================
// Watchers
// ========================================

// 监听外部值变化
watch(
  () => props.modelValue,
  (value) => {
    if (value) {
      form.provider = value.provider
      form.tenantId = value.tenantId || ''
      form.domain = value.domain || ''
      updatePreview()
    }
  },
  { immediate: true }
)

// ========================================
// Expose
// ========================================

defineExpose({
  validate,
  getValue,
  reset,
})
</script>

<style scoped>
.enterprise-config-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 4px;
}

.form-header {
  text-align: center;
}

.form-header h4 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--n-text-color);
}

.form-header .description {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--n-text-color-3);
}

.input-icon {
  font-size: 14px;
}

.config-preview {
  margin-top: 8px;
}

.preview-header {
  font-weight: 500;
}

.preview-content {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
}

.preview-item {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}

.preview-item .label {
  color: var(--n-text-color-3);
  font-weight: 500;
}

.preview-item .value {
  color: var(--n-text-color);
  font-weight: 600;
}

.auth-info {
  margin-top: 4px;
}

.auth-methods {
  margin: 8px 0 0;
  padding-left: 20px;
  font-size: 12px;
  color: var(--n-text-color-2);
}

.auth-methods li {
  margin: 4px 0;
}

.config-guide {
  display: flex;
  justify-content: center;
  padding: 8px 0 4px;
}
</style>
