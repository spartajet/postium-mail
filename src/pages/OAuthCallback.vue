<script setup lang="ts">
import { onMounted } from 'vue'

onMounted(() => {
  // 从 URL 查询参数获取授权码
  const params = new URLSearchParams(window.location.search)
  const code = params.get('code')
  const state = params.get('state')
  const error = params.get('error')

  // 向父窗口发送消息
  if (window.opener) {
    window.opener.postMessage({
      code,
      state,
      error: error || undefined
    }, window.location.origin)
  }

  // 关闭当前窗口
  window.close()
})
</script>

<template>
  <div class="oauth-callback">
    <h2>授权成功</h2>
    <p>窗口即将关闭...</p>
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
}

.oauth-callback h2 {
  font-size: 24px;
  font-weight: 600;
  margin-bottom: 12px;
}

.oauth-callback p {
  font-size: 16px;
  opacity: 0.9;
}
</style>
