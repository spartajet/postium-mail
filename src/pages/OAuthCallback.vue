<script setup lang="ts">
import { ref, onMounted } from 'vue'

const code = ref('')
const state = ref('')
const error = ref('')
const copied = ref(false)

onMounted(() => {
  console.log('[OAuthCallback] 页面加载')
  console.log('[OAuthCallback] 当前URL:', window.location.href)

  // 从 URL 查询参数获取授权码
  const params = new URLSearchParams(window.location.search)
  code.value = params.get('code') || ''
  state.value = params.get('state') || ''
  error.value = params.get('error') || ''

  console.log('[OAuthCallback] 解析参数:', {
    hasCode: !!code.value,
    hasState: !!state.value,
    error: error.value
  })

  // 尝试向父窗口发送消息（如果存在）
  if (window.opener) {
    console.log('[OAuthCallback] 发送 postMessage 到父窗口')
    try {
      window.opener.postMessage({
        code: code.value,
        state: state.value,
        error: error.value || undefined
      }, window.location.origin)
      console.log('[OAuthCallback] postMessage 已发送')
    } catch (e) {
      console.error('[OAuthCallback] postMessage 发送失败:', e)
    }
  } else {
    console.log('[OAuthCallback] 没有父窗口（系统浏览器模式），显示授权码供用户复制')
  }

  // 不再自动关闭窗口，让用户复制授权码
})

function copyCode() {
  if (code.value) {
    navigator.clipboard.writeText(code.value)
      .then(() => {
        copied.value = true
        setTimeout(() => {
          copied.value = false
        }, 2000)
      })
      .catch(err => {
        console.error('[OAuthCallback] 复制失败:', err)
      })
  }
}
</script>

<template>
  <div class="oauth-callback">
    <div class="content-box">
      <h2>{{ error ? '授权失败' : '授权成功' }}</h2>

      <div v-if="error" class="error-section">
        <p class="error-message">{{ error }}</p>
        <p>请关闭此窗口并重试。</p>
      </div>

      <div v-else class="success-section">
        <p class="instruction">请复制以下授权码并返回应用粘贴：</p>

        <div class="code-container">
          <code class="auth-code">{{ code || '未获取到授权码' }}</code>
          <button
            v-if="code"
            @click="copyCode"
            class="copy-button"
          >
            {{ copied ? '已复制！' : '复制授权码' }}
          </button>
        </div>

        <p class="hint">提示：点击"复制授权码"按钮，然后返回应用粘贴</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.oauth-callback {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  font-family: system-ui, -apple-system, sans-serif;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  padding: 20px;
}

.content-box {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 16px;
  padding: 40px;
  max-width: 500px;
  width: 100%;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  color: #333;
}

.oauth-callback h2 {
  font-size: 24px;
  font-weight: 600;
  margin-bottom: 24px;
  text-align: center;
}

.success-section,
.error-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.instruction {
  font-size: 16px;
  color: #555;
  text-align: center;
  margin-bottom: 8px;
}

.code-container {
  background: #f8f9fa;
  border: 2px dashed #dee2e6;
  border-radius: 8px;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  align-items: center;
}

.auth-code {
  background: white;
  padding: 16px;
  border-radius: 6px;
  font-family: 'Courier New', monospace;
  font-size: 14px;
  color: #333;
  word-break: break-all;
  user-select: all;
  width: 100%;
  text-align: center;
  border: 1px solid #e0e0e0;
}

.copy-button {
  background: #667eea;
  color: white;
  border: none;
  padding: 12px 24px;
  border-radius: 8px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}

.copy-button:hover {
  background: #5568d3;
  transform: translateY(-1px);
}

.copy-button:active {
  transform: translateY(0);
}

.hint {
  font-size: 14px;
  color: #888;
  text-align: center;
  font-style: italic;
}

.error-section {
  text-align: center;
}

.error-message {
  color: #dc3545;
  font-weight: 600;
  font-size: 16px;
}
</style>
