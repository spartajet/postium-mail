/**
 * 将 Tauri Command 错误转换为用户友好消息
 */
export function formatError(error: unknown): string {
  if (error instanceof Error) {
    const msg = error.message;

    // 尝试解析结构化错误 JSON
    try {
      const parsed = JSON.parse(msg);
      if (parsed.message) {
        return parsed.message;
      }
    } catch {}

    return msg;
  }

  if (typeof error === 'string') {
    return error;
  }

  return '未知错误';
}

/**
 * 全局未处理错误提示
 */
export function setupGlobalErrorHandler() {
  window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
  });

  window.addEventListener('error', (event) => {
    console.error('Unhandled error:', event.error);
  });
}
