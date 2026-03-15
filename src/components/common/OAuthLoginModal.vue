<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NModal, NSpin, NAlert } from 'naive-ui'

const props = defineProps<{
  show: boolean
  provider: 'microsoft' | 'google'
}>()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
  (e: 'success', token: { access_token: string, refresh_token: string, expires_at: number }): void
}>()

const loading = ref(true)
const error = ref<string | null>(null)

// 存储csrf_state用于验证
let csrfState: string = ''

onMounted(async () => {
  if (props.show) {
    await startOAuthFlow()
  }
})

async function startOAuthFlow() {
  try {
    loading.value = true
    error.value = null

    // 1. 获取授权 URL和csrf_state
    const [authUrl, state] = await invoke<[string, string]>('get_oauth_auth_url', {
      provider: props.provider,
    })

    // 存储csrf_state用于后续验证
    csrfState = state

    // 2. 打开浏览器窗口
    const width = 500
    const height = 600
    const left = (window.screen.width - width) / 2
    const top = (window.screen.height - height) / 2

    const authWindow = window.open(
      authUrl,
      'OAuth Authorization',
      `width=${width},height=${height},left=${left},top=${top}`
    )

    if (!authWindow) {
      throw new Error('无法打开授权窗口，请检查浏览器弹窗设置')
    }

    // 3. 监听回调消息
    const messageHandler = (event: MessageEvent) => {
      // 验证来源（安全检查）
      if (event.origin !== window.location.origin) return

      const { code, state: returnedState, error: oauthError } = event.data

      // 验证state参数以防止CSRF攻击
      if (returnedState !== csrfState) {
        error.value = '状态验证失败，可能存在安全风险'
        loading.value = false
        authWindow.close()
        return
      }

      authWindow.close()

      if (oauthError) {
        error.value = oauthError
        loading.value = false
        return
      }

      if (code) {
        exchangeCodeForToken(code)
      }
    }

    window.addEventListener('message', messageHandler)

    // 设置超时
    setTimeout(() => {
      window.removeEventListener('message', messageHandler)
      if (loading.value) {
        error.value = '授权超时，请重试'
        loading.value = false
      }
    }, 300000) // 5 分钟超时
  } catch (e) {
    error.value = String(e)
    loading.value = false
  }
}

async function exchangeCodeForToken(code: string) {
  try {
    // 注意：后端会自动从Microsoft Graph API获取用户email
    const account = await invoke<{
      id: number
      name: string
      email: string
      provider: string
    }>('exchange_oauth_code', {
      provider: props.provider,
      code,
      csrf_state: csrfState
    })

    // 成功创建账号，触发成功事件
    emit('success', {
      access_token: '', // token已安全存储在后端，不需要传递到前端
      refresh_token: '',
      expires_at: 0
    })
    emit('update:show', false)
  } catch (e) {
    error.value = String(e)
    loading.value = false
  }
}

onUnmounted(() => {
  // 清理事件监听
})
</script>

<template>
  <NModal
    :show="show"
    @update:show="emit('update:show', $event)"
    preset="card"
    :title="provider === 'microsoft' ? 'Microsoft 登录' : 'Google 登录'"
    :style="{ width: '400px' }"
  >
    <div class="oauth-login">
      <NSpin :show="loading">
        <div class="content">
          <h3>{{ provider === 'microsoft' ? 'Microsoft' : 'Google' }} 账号授权</h3>

          <NAlert v-if="error" type="error" :title="error" class="mb-4" />

          <p v-if="!error">
            正在打开授权窗口，请在浏览器中完成授权...
          </p>

          <div class="steps">
            <div class="step">
              <span class="step-number">1</span>
              <span class="step-text">浏览器将打开授权页面</span>
            </div>
            <div class="step">
              <span class="step-number">2</span>
              <span class="step-text">登录并授权应用访问邮件</span>
            </div>
            <div class="step">
              <span class="step-number">3</span>
              <span class="step-text">授权完成后自动返回</span>
            </div>
          </div>
        </div>
      </NSpin>
    </div>
  </NModal>
</template>

<style scoped>
.oauth-login {
  padding: 20px 0;
}

.content {
  text-align: center;
}

.steps {
  margin-top: 20px;
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.step {
  display: flex;
  align-items: center;
  gap: 12px;
}

.step-number {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--primary-color);
  color: white;
  font-size: 14px;
  font-weight: bold;
  flex-shrink: 0;
}

.step-text {
  color: var(--text-color-2);
}
</style>

