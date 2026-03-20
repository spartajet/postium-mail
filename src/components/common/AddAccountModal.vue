<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue'
import { useUIStore, useAccountStore } from '@/stores'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { NModal, NForm, NFormItem, NInput, NInputNumber, NSelect, NButton, NSwitch, NAlert, NRadioGroup, NRadio, NProgress, NCard, NCollapse, NCollapseItem } from 'naive-ui'
import OAuthLoginModal from './OAuthLoginModal.vue'
import { extractAutoFillInfo, parseEmail, detectProviderFromEmail } from '@/utils/emailHelper'
import { AccountType, AuthType } from '@/types'

const uiStore = useUIStore()
const accountStore = useAccountStore()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
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
  accountType: AccountType.Personal,
  authType: AuthType.Password,
  imapHost: '',
  imapPort: 993,
  imapSsl: true,
  smtpHost: '',
  smtpPort: 587,
  smtpSsl: true,
  color: '#7C3AED',
  // 企业邮箱配置
  enterpriseTenantId: '',
  enterpriseDomain: '',
})

const loading = ref(false)
const error = ref('')
const success = ref('')

// 自动判断相关状态
const isProviderManuallySet = ref(false) // 是否由用户手动设置的服务商
const isAuthTypeManuallySet = ref(false) // 是否由用户手动修改了认证方式
const lastValidEmail = ref('') // 上一次有效的邮箱地址
const isInitializing = ref(true) // 是否正在初始化（避免初始化时触发watch）

// OAuth 相关
const showOAuthModal = ref(false)
const oauthProvider = ref<'microsoft' | 'google'>('microsoft')
const oauthToken = ref<{ access_token: string, refresh_token: string, expires_at: number } | null>(null)

// 企业配置预览
const enterprisePreview = ref({
  imapHost: '',
  smtpHost: '',
})

// 更新企业配置预览
function updateEnterprisePreview() {
  if (form.value.enterpriseTenantId || form.value.enterpriseDomain) {
    // Microsoft 365 / Outlook
    if (form.value.provider === 'outlook' || form.value.enterpriseTenantId?.includes('onmicrosoft')) {
      enterprisePreview.value = {
        imapHost: 'outlook.office365.com',
        smtpHost: 'smtp.office365.com',
      }
    }
    // Google Workspace / Gmail
    else if (form.value.provider === 'gmail' || form.value.enterpriseTenantId?.includes('gmail')) {
      enterprisePreview.value = {
        imapHost: 'imap.gmail.com',
        smtpHost: 'smtp.gmail.com',
      }
    }
  } else {
    enterprisePreview.value = {
      imapHost: '',
      smtpHost: '',
    }
  }
}

// 进度状态
const syncProgress = ref({
  stage: 'idle' as 'idle' | 'authenticating' | 'validating' | 'syncing' | 'completed' | 'error',
  currentStep: 0,
  totalSteps: 3,
  message: '',
  percentage: 0,
})

// 事件监听器清理
let unlistenProgress: (() => void) | null = null

const isCustom = computed(() => form.value.provider === 'imap')
const canUseOAuth = computed(() => {
  return form.value.provider === 'outlook' || form.value.provider === 'hotmail' || form.value.provider === 'gmail'
})
const isEnterprise = computed(() => form.value.accountType === AccountType.Enterprise)

// 监听 provider 变化，自动设置邮箱前缀
watch(() => form.value.provider, (newProvider, oldProvider) => {
  // 如果是初始化阶段，不标记为手动设置
  if (isInitializing.value) {
    return
  }

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
      ? AuthType.OAuth2
      : AuthType.Password
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
  console.log('#################### [AddAccountModal] PROVIDER WATCHER END ####################\n')
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
    // 将字符串转换为 AuthType 枚举
    form.value.authType = autoFillInfo.authType === 'oauth' ? AuthType.OAuth2 : AuthType.Password
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
    // 重置进度状态
    syncProgress.value = {
      stage: 'idle',
      currentStep: 0,
      totalSteps: 3,
      message: '',
      percentage: 0,
    }
    // 清理事件监听器
    if (unlistenProgress) {
      unlistenProgress()
      unlistenProgress = null
    }
  } else {
    // 打开时，重置为默认状态
    isProviderManuallySet.value = false
    isAuthTypeManuallySet.value = false
    lastValidEmail.value = ''
    isInitializing.value = true

    // 延迟将 isInitializing 设置为 false，确保初始化完成
    setTimeout(() => {
      isInitializing.value = false
    }, 100)
  }
})

// 组件销毁时清理监听器
onUnmounted(() => {
  if (unlistenProgress) {
    unlistenProgress()
    unlistenProgress = null
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
  console.log('[AddAccountModal] handleSubmit 开始')
  console.log('[AddAccountModal] 表单数据:', {
    name: form.value.name,
    email: form.value.email,
    provider: form.value.provider,
    authType: form.value.authType,
    hasOAuthToken: !!oauthToken.value,
  })

  error.value = ''
  success.value = ''

  try {
    // 基础验证
    if (!form.value.name.trim()) {
      error.value = '请输入账号名称'
      console.log('[AddAccountModal] 验证失败：账号名称为空')
      return
    }

    if (!form.value.email.trim() || form.value.email.startsWith('@')) {
      error.value = '请输入完整的邮箱地址'
      console.log('[AddAccountModal] 验证失败：邮箱地址不完整')
      return
    }

    console.log('[AddAccountModal] 基础验证通过')

    // OAuth 模式：检查 token，如果没有则自动触发授权
    if (form.value.authType === AuthType.OAuth2) {
      console.log('[AddAccountModal] 处理OAuth模式')

      if (!oauthToken.value) {
        console.log('[AddAccountModal] 没有OAuth token，触发授权')
        // 自动触发 OAuth 授权
        startOAuthLogin()
        return
      }

      console.log('[AddAccountModal] 有OAuth token，开始验证')

      // 有 token，验证是否有效
      try {
        syncProgress.value = {
          stage: 'authenticating',
          currentStep: 1,
          totalSteps: 3,
          message: '验证授权信息...',
          percentage: 20,
        }

        console.log('[AddAccountModal] 调用 validate_oauth_token')
        const isValid = await invoke('validate_oauth_token', {
          provider: oauthProvider.value,
          token: oauthToken.value.access_token,
        })

        console.log('[AddAccountModal] validate_oauth_token 结果:', isValid)

        if (!isValid) {
          // token 无效，重新授权
          error.value = '授权已过期，请重新授权'
          syncProgress.value.stage = 'idle'
          startOAuthLogin()
          return
        }
      } catch (e: any) {
        console.error('[AddAccountModal] validate_oauth_token 出错:', e)
        error.value = `授权验证失败：${e}`
        syncProgress.value.stage = 'idle'
        return
      }
    }

    // 密码模式：验证连接
    if (form.value.authType === AuthType.Password) {
      console.log('[AddAccountModal] 处理密码模式')

      if (!form.value.password.trim()) {
        error.value = '请输入密码'
        console.log('[AddAccountModal] 验证失败：密码为空')
        return
      }

      try {
        syncProgress.value = {
          stage: 'validating',
          currentStep: 1,
          totalSteps: 3,
          message: '验证邮箱连接...',
          percentage: 20,
        }

        console.log('[AddAccountModal] 调用 test_email_connection')
        await invoke('test_email_connection', {
          email: form.value.email,
          password: form.value.password,
          provider: form.value.provider,
          imapHost: isCustom.value ? form.value.imapHost : null,
          imapPort: isCustom.value ? form.value.imapPort : null,
          imapSsl: isCustom.value ? form.value.imapSsl : null,
          smtpHost: isCustom.value ? form.value.smtpHost : null,
          smtpPort: isCustom.value ? form.value.smtpPort : null,
          smtpSsl: isCustom.value ? form.value.smtpSsl : null,
        })

        console.log('[AddAccountModal] test_email_connection 成功')
      } catch (e: any) {
        console.error('[AddAccountModal] test_email_connection 出错:', e)
        error.value = `连接验证失败：${e}`
        syncProgress.value.stage = 'idle'
        return
      }
    }

    console.log('[AddAccountModal] 验证完成，开始创建账号并同步')

    // 创建账号并同步
    await createAccountAndSync()
  } catch (e: any) {
    console.error('[AddAccountModal] handleSubmit 全局错误:', e)
    error.value = `操作失败：${e?.message || String(e)}`
    syncProgress.value.stage = 'idle'
    loading.value = false
  }
}

async function createAccountAndSync() {
  console.log('[AddAccountModal] createAccountAndSync 开始')
  loading.value = true

  try {
    syncProgress.value = {
      stage: 'syncing',
      currentStep: 2,
      totalSteps: 3,
      message: '正在创建账号...',
      percentage: 40,
    }

    const accountData: any = {
      name: form.value.name,
      email: form.value.email,
      provider: form.value.provider,
      color: form.value.color,
      accountType: form.value.accountType,
      authType: form.value.authType,
    }

    console.log('[AddAccountModal] 准备账号数据:', {
      ...accountData,
      authType: form.value.authType,
      isCustom: isCustom.value,
    })

    if (form.value.authType === AuthType.OAuth2 && oauthToken.value) {
      console.log('[AddAccountModal] 使用OAuth认证')
      accountData.auth_type = 'oauth2'
      accountData.oauth_provider = oauthProvider.value
      accountData.oauth_token = oauthToken.value.access_token
      accountData.oauth_refresh_token = oauthToken.value.refresh_token
      accountData.password = '' // OAuth 不需要密码
    } else {
      console.log('[AddAccountModal] 使用密码认证')
      accountData.password = form.value.password
    }

    if (isCustom.value) {
      console.log('[AddAccountModal] 自定义服务器配置')
      accountData.imap_host = form.value.imapHost
      accountData.imap_port = form.value.imapPort
      accountData.imap_ssl = form.value.imapSsl
      accountData.smtp_host = form.value.smtpHost
      accountData.smtp_port = form.value.smtpPort
      accountData.smtp_ssl = form.value.smtpSsl
    }

    // 企业邮箱配置
    if (isEnterprise.value) {
      console.log('[AddAccountModal] 企业邮箱配置')
      accountData.enterprise_tenant_id = form.value.enterpriseTenantId || null
      accountData.enterprise_domain = form.value.enterpriseDomain || null
    }

    // 1. 检查邮箱是否已存在于本地列表
    const existingAccount = accountStore.accounts.find(a => a.email === form.value.email)
    if (existingAccount) {
      throw new Error(`邮箱地址 ${form.value.email} 已存在于账号列表中（账号名：${existingAccount.name}）。请先在设置中删除旧账号，或使用不同的邮箱地址。`)
    }

    // 2. 创建账号
    console.log('[AddAccountModal] 调用 add_account')
    const accountResult = await invoke('add_account', { account: accountData }) as { id: number }
    const accountId = accountResult.id

    console.log('[AddAccountModal] 账号创建成功, ID:', accountId)

    // 3. 刷新账号列表（从后端获取最新数据，避免重复添加）
    await accountStore.fetchAccounts()

    // 3.1 切换到新创建的账号
    accountStore.selectAccountById(String(accountId))

    // 3.2 重置加载状态
    loading.value = false

    // 4. 立即关闭对话框并返回成功
    syncProgress.value = {
      stage: 'idle',
      currentStep: 3,
      totalSteps: 3,
      message: '账号已添加，正在后台同步...',
      percentage: 100,
    }

    // 5. 注册同步进度监听器（用于状态栏显示）
    console.log('[AddAccountModal] 注册同步进度监听器（用于状态栏）')
    await listenToSyncProgress(accountId)

    // 6. 关闭对话框（通过 uiStore 而不是 emit）
    uiStore.closeAddAccountModal()

    // 7. 触发后台同步（不等待完成）
    console.log('[AddAccountModal] 触发后台同步, accountId:', accountId)
    invoke('sync_account_with_progress', { accountId })
      .then(() => {
        console.log('[AddAccountModal] 后台同步完成')
      })
      .catch((error) => {
        console.error('[AddAccountModal] 后台同步失败:', error)
      })

    console.log('[AddAccountModal] 账号添加成功，对话框已关闭，同步在后台进行')
  } catch (e: any) {
    console.error('[AddAccountModal] createAccountAndSync 出错:', e)
    console.error('[AddAccountModal] 错误详情:', {
      message: e?.message,
      stack: e?.stack,
      string: String(e),
      json: JSON.stringify(e),
    })

    syncProgress.value = {
      stage: 'error',
      currentStep: 0,
      totalSteps: 3,
      message: '',
      percentage: 0,
    }

    // 提供更详细的错误信息
    let errorMessage = '创建账号失败'
    if (typeof e === 'string') {
      errorMessage = e
    } else if (e?.message) {
      errorMessage = e.message
    } else if (e?.toString) {
      errorMessage = e.toString()
    }

    error.value = errorMessage
    loading.value = false
  }
}

async function listenToSyncProgress(accountId: number) {
  const eventName = `sync-progress-${accountId}`

  // 保存 unlisten 函数以便后续清理
  unlistenProgress = await listen(eventName, (event: any) => {
    const progress = event.payload as {
      stage: string
      current: number
      total: number
      message: string
    }

    // 计算：如果 total 为 0，显示不确定进度（50%）
    const hasTotal = progress.total > 0
    const percentage = hasTotal
      ? Math.floor((progress.current / progress.total) * 100)
      : 50 // 不确定进度时显示 50%
    const currentStep = hasTotal
      ? Math.floor((progress.current / progress.total) * 3) + 1
      : 2

    syncProgress.value = {
      stage: 'syncing',
      currentStep,
      totalSteps: 3,
      message: progress.message,
      percentage,
    }

    if (progress.stage === 'completed') {
      // 同步完成
      syncProgress.value.stage = 'completed'
      syncProgress.value.percentage = 100
      syncProgress.value.message = '同步完成！'

      setTimeout(async () => {
        await accountStore.fetchAccounts()
        emit('success')
        uiStore.closeAddAccountModal()
        resetForm()
        loading.value = false
        if (unlistenProgress) {
          unlistenProgress()
          unlistenProgress = null
        }
      }, 1000)
    } else if (progress.stage === 'error') {
      // 同步出错
      syncProgress.value.stage = 'error'
      error.value = progress.message
      loading.value = false
      if (unlistenProgress) {
        unlistenProgress()
        unlistenProgress = null
      }
    }
  })
}

// 辅助函数：获取阶段标题
function getStageTitle(stage: string): string {
  switch (stage) {
    case 'authenticating': return '正在验证授权'
    case 'validating': return '正在验证连接'
    case 'syncing': return '正在同步'
    case 'completed': return '完成'
    case 'error': return '失败'
    default: return '准备中'
  }
}

function resetForm() {
  form.value = {
    name: '',
    email: '',
    provider: 'gmail',
    accountType: AccountType.Personal,
    authType: AuthType.Password,
    password: '',
    imapHost: '',
    imapPort: 993,
    imapSsl: true,
    smtpHost: '',
    smtpPort: 587,
    smtpSsl: true,
    color: '#7C3AED',
    // 企业邮箱配置
    enterpriseTenantId: '',
    enterpriseDomain: '',
  }
  oauthToken.value = null
  success.value = ''
  error.value = ''
  // 重置进度状态
  syncProgress.value = {
    stage: 'idle',
    currentStep: 0,
    totalSteps: 3,
    message: '',
    percentage: 0,
  }
}

function startOAuthLogin() {
  console.log('[AddAccountModal] startOAuthLogin 被调用')
  console.log('[AddAccountModal] 当前 provider:', form.value.provider)

  if (form.value.provider === 'outlook' || form.value.provider === 'hotmail') {
    oauthProvider.value = 'microsoft'
    console.log('[AddAccountModal] 设置 oauthProvider 为 microsoft')
  } else if (form.value.provider === 'gmail') {
    oauthProvider.value = 'google'
    console.log('[AddAccountModal] 设置 oauthProvider 为 google')
  }

  console.log('[AddAccountModal] 打开 OAuth 弹窗')
  showOAuthModal.value = true
  console.log('[AddAccountModal] showOAuthModal.value:', showOAuthModal.value)
}

function handleOAuthSuccess(token: { access_token: string, refresh_token: string, expires_at: number }) {
  oauthToken.value = token
  success.value = 'OAuth 授权成功！'
  error.value = ''
}
</script>

<template>
  <NModal
    :show="show"
    @update:show="uiStore.closeAddAccountModal"
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

      <!-- 账号类型 -->
      <NFormItem label="账号类型">
        <NRadioGroup v-model:value="form.accountType" :disabled="loading">
          <NRadio :value="AccountType.Personal">个人邮箱</NRadio>
          <NRadio :value="AccountType.Enterprise">企业邮箱</NRadio>
        </NRadioGroup>
      </NFormItem>

      <!-- 认证方式 -->
      <NFormItem label="认证方式" v-if="canUseOAuth">
        <NRadioGroup v-model:value="form.authType" :disabled="loading">
          <NRadio :value="AuthType.Password">密码登录</NRadio>
          <NRadio :value="AuthType.OAuth2">OAuth 2.0 授权</NRadio>
        </NRadioGroup>
      </NFormItem>

      <!-- OAuth 登录按钮 -->
      <template v-if="canUseOAuth && form.authType === AuthType.OAuth2">
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
        v-if="form.authType === AuthType.Password"
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

      <!-- 企业邮箱配置 -->
      <NCollapse v-if="isEnterprise">
        <NCollapseItem title="企业邮箱配置" name="enterprise">
          <NFormItem label="说明">
            <NAlert type="info" title="企业邮箱配置说明">
              如果您的组织使用 Microsoft 365 或 Google Workspace，请填写以下信息以获得更好的配置体验。
            </NAlert>
          </NFormItem>

          <NFormItem label="租户 ID / 域名">
            <NInput
              v-model:value="form.enterpriseTenantId"
              placeholder="例如: contoso.onmicrosoft.com 或 example.com"
              @input="updateEnterprisePreview"
            />
          </NFormItem>

          <NFormItem label="域名（可选）">
            <NInput
              v-model:value="form.enterpriseDomain"
              placeholder="例如: example.com"
            />
          </NFormItem>

          <!-- 配置预览 -->
          <NFormItem v-if="enterprisePreview.imapHost" label="配置预览">
            <div class="enterprise-preview">
              <p><strong>IMAP 服务器:</strong> {{ enterprisePreview.imapHost }}</p>
              <p><strong>SMTP 服务器:</strong> {{ enterprisePreview.smtpHost }}</p>
            </div>
          </NFormItem>
        </NCollapseItem>
      </NCollapse>

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

      <!-- 进度显示 -->
      <div v-if="syncProgress.stage !== 'idle'" class="sync-progress">
        <NCard :bordered="false" class="progress-card">
          <div class="progress-header">
            <span class="progress-title">{{ getStageTitle(syncProgress.stage) }}</span>
            <span class="progress-percentage">{{ syncProgress.percentage }}%</span>
          </div>

          <NProgress
            type="line"
            :percentage="syncProgress.percentage"
            :processing="syncProgress.stage === 'syncing'"
            :style="{ marginBottom: '12px' }"
          />

          <div class="progress-message">
            {{ syncProgress.message }}
          </div>

          <div v-if="syncProgress.stage === 'syncing'" class="progress-steps">
            <div
              v-for="(step, index) in ['验证账号', '创建记录', '同步邮件']"
              :key="step"
              class="progress-step"
              :class="{ active: index + 1 === syncProgress.currentStep }"
            >
              {{ step }}
            </div>
          </div>
        </NCard>
      </div>

      <!-- 操作按钮 -->
      <div class="form-actions">
        <NButton @click="uiStore.closeAddAccountModal" :disabled="loading">
          取消
        </NButton>
        <NButton type="primary" attr-type="submit" :loading="loading" :disabled="form.authType === AuthType.OAuth2 && !oauthToken">
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

.sync-progress {
  margin: 16px 0;
}

.progress-card {
  background: var(--bg-secondary);
}

.progress-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.progress-title {
  font-weight: 500;
  font-size: 14px;
}

.progress-percentage {
  font-size: 14px;
  color: var(--text-secondary);
}

.progress-message {
  font-size: 13px;
  color: var(--text-secondary);
  margin-top: 8px;
}

.progress-steps {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.progress-step {
  font-size: 12px;
  padding: 4px 8px;
  border-radius: 4px;
  background: var(--bg-tertiary);
  color: var(--text-tertiary);
  transition: all 0.2s;
}

.progress-step.active {
  background: var(--primary-bg);
  color: var(--primary-fg);
}

.enterprise-preview {
  padding: 12px;
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  font-size: 13px;
}

.enterprise-preview p {
  margin: 4px 0;
}

.enterprise-preview strong {
  color: var(--text-primary);
}
</style>
