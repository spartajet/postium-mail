<script setup lang="ts">
import { useUIStore } from '@/stores'

// Store
const uiStore = useUIStore()

// 关闭 Toast
function handleClose(id: number) {
  uiStore.removeToast(id)
}

// 获取图标
function getIcon(type: string): string {
  switch (type) {
    case 'success':
      return 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z'
    case 'error':
      return 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z'
    case 'info':
      return 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z'
    default:
      return 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z'
  }
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
          <svg class="toast-icon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
            <path :d="getIcon(toast.type)" />
          </svg>
          <span class="toast-message">{{ toast.message }}</span>
          <button class="toast-close" @click="handleClose(toast.id)">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
              <path d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"/>
            </svg>
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
  width: 20px;
  height: 20px;
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

.toast-close svg {
  width: 16px;
  height: 16px;
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
