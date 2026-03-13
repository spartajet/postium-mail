<script setup lang="ts">
import { useUIStore } from '@/stores'
import { CheckCircleOutlined, ErrorOutlined, InfoOutlined, CloseOutlined } from '@vicons/material'

// Store
const uiStore = useUIStore()

// 关闭 Toast
function handleClose(id: number) {
  uiStore.removeToast(id)
}
</script>

<template>
  <Teleport to="body">
    <div class="toast-container">
      <TransitionGroup name="toast">
        <div
          v-for="toast in uiStore.toasts"
          :key="toast.id"
          :class="['toast', toast.type]"
        >
          <CheckCircleOutlined v-if="toast.type === 'success'" class="toast-icon" :size="20" />
          <ErrorOutlined v-else-if="toast.type === 'error'" class="toast-icon" :size="20" />
          <InfoOutlined v-else class="toast-icon" :size="20" />
          <span class="toast-message">{{ toast.message }}</span>
          <button class="toast-close" @click="handleClose(toast.id)">
            <CloseOutlined :size="16" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 48px;
  right: 24px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 800;
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  pointer-events: auto;
  min-width: 280px;
  max-width: 400px;
}

.toast.success {
  border-left: 3px solid #10B981;
}

.toast.error {
  border-left: 3px solid #EF4444;
}

.toast.info {
  border-left: 3px solid #3B82F6;
}

.toast-icon {
  flex-shrink: 0;
}

.toast.success .toast-icon {
  color: #10B981;
}

.toast.error .toast-icon {
  color: #EF4444;
}

.toast.info .toast-icon {
  color: #3B82F6;
}

.toast-message {
  flex: 1;
  font-size: 14px;
  color: var(--text-primary);
  line-height: 1.4;
}

.toast-close {
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  border-radius: 4px;
  transition: all 0.15s ease;
  flex-shrink: 0;
}

.toast-close:hover {
  background: var(--bg-glass-hover);
  color: var(--text-primary);
}

/* Transition animations */
.toast-enter-active {
  animation: slideIn 0.3s ease;
}

.toast-leave-active {
  animation: slideOut 0.3s ease;
}

@keyframes slideIn {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes slideOut {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}

/* Responsive */
@media (max-width: 640px) {
  .toast-container {
    left: 16px;
    right: 16px;
    bottom: 48px;
  }

  .toast {
    min-width: auto;
    max-width: none;
  }
}
</style>
