/**
 * Postium Mail - 错误处理工具库
 * error.ts
 *
 * 本文件提供应用中的错误处理工具函数。
 * 错误处理是应用稳定性和用户体验的重要组成部分。
 *
 * ==================== 功能概述 ====================
 * 1. formatError(): 将各种类型的错误转换为用户友好的消息
 * 2. setupGlobalErrorHandler(): 设置全局未处理错误的监听器
 *
 * ==================== 设计原则 ====================
 * 1. 用户友好：将技术错误转换为用户可理解的中文消息
 * 2. 可靠性：确保任何错误都能被处理，不会导致应用崩溃
 * 3. 可调试：详细的错误日志帮助开发者定位问题
 * 4. 类型安全：使用 TypeScript 提供完整的类型检查
 *
 * ==================== 使用场景 ====================
 * - Tauri Command 调用失败时的错误处理
 * - Promise rejection 的全局捕获
 * - 未捕获异常的日志记录
 * - 用户界面中的错误提示
 */

/**
 * 将 Tauri Command 错误转换为用户友好消息
 *
 * 此函数负责将各种类型的错误（Error 对象、字符串、其他类型）
 * 转换为可以在用户界面中显示的友好消息字符串。
 *
 * ==================== 功能说明 ====================
 * 1. 处理 Error 对象：提取错误消息并尝试解析
 * 2. 解析结构化错误：如果消息是 JSON，尝试提取其中的 message 字段
 * 3. 处理字符串错误：直接返回字符串
 * 4. 处理未知类型：返回默认的"未知错误"消息
 *
 * ==================== 参数说明 ====================
 * @param error - 要格式化的错误，可以是任意类型
 *                - Error 对象：最常见的错误类型
 *                - string：简单的错误消息字符串
 *                - 其他类型：返回"未知错误"
 *
 * ==================== 返回值说明 ====================
 * @returns 格式化后的用户友好错误消息（字符串）
 *          - 如果是 Error 对象，返回其 message 属性
 *          - 如果 message 是 JSON，解析并提取 message 字段
 *          - 如果是字符串，直接返回
 *          - 其他情况返回"未知错误"
 *
 * ==================== 代码示例 ====================
 *
 * 基本用法：
 * ```typescript
 * // 处理 Error 对象
 * try {
 *   throw new Error('网络连接失败');
 * } catch (error) {
 *   const message = formatError(error);
 *   console.log(message);  // 输出: '网络连接失败'
 * }
 * ```
 *
 * 处理 Tauri Command 错误：
 * ```typescript
 * import { commands } from '$lib/bindings';
 *
 * async function loadEmails() {
 *   try {
 *     const result = await commands.listEmails(1, 'inbox', 0, 20);
 *     if (result.status === 'error') {
 *       // 使用 formatError 转换错误
 *       const message = formatError(result.error);
 *       alert(`加载邮件失败: ${message}`);
 *       return [];
 *     }
 *     return result.data.emails;
 *   } catch (error) {
 *     // 捕获意外的错误
 *     const message = formatError(error);
 *     alert(`发生错误: ${message}`);
 *     return [];
 *   }
 * }
 * ```
 *
 * 解析 JSON 格式的错误：
 * ```typescript
 * // Tauri 可能返回这样的错误：
 * // {"type":"AuthFailed","message":"用户名或密码错误"}
 * const error = new Error('{"type":"AuthFailed","message":"用户名或密码错误"}');
 * const message = formatError(error);
 * console.log(message);  // 输出: '用户名或密码错误'
 * ```
 *
 * 在组件中使用：
 * ```svelte
 * <script lang="ts">
 *   import { formatError } from '$lib/utils/error';
 *   import { commands } from '$lib/bindings';
 *   import { toast } from '$lib/stores/toast.svelte';
 *
 *   async function handleSendEmail() {
 *     try {
 *       const result = await commands.sendEmail(request);
 *       if (result.status === 'error') {
 *         // 格式化错误并显示 Toast
 *         const message = formatError(result.error);
 *         toast.error(`发送失败: ${message}`);
 *       } else {
 *         toast.success('邮件已发送');
 *       }
 *     } catch (error) {
 *       const message = formatError(error);
 *       toast.error(`发送失败: ${message}`);
 *     }
 *   }
 * </script>
 * ```
 *
 * ==================== 注意事项 ====================
 * 1. 防止 XSS：确保错误消息不会包含恶意脚本（Tauri 的错误通常安全）
 * 2. i18n：如果需要国际化，可以考虑在翻译前调用此函数
 * 3. 日志：在生产环境中，建议同时记录完整的错误对象到日志
 * 4. 敏感信息：避免将技术细节直接展示给最终用户
 *
 * ==================== 错误类型说明 ====================
 * 根据 bindings.ts 中的 MailError 类型，可能遇到的错误类型：
 * - AccountNotFound: 账户不存在
 * - AuthFailed: 认证失败（用户名密码错误）
 * - ImapConnectionFailed: IMAP 连接失败
 * - SmtpSendFailed: SMTP 发送失败
 * - SyncFailed: 同步失败
 * - DatabaseError: 数据库错误
 * - KeyringError: 密钥环错误
 * - ProviderNotSupported: 不支持的服务商
 * - InvalidParam: 参数无效
 * - EmailNotFound: 邮件不存在
 * - FolderNotFound: 文件夹不存在
 * - OAuthError: OAuth 错误
 * - InvalidProvider: 无效的服务商
 * - OAuth2Error: OAuth2 错误
 * - NotImplemented: 功能未实现
 * - LabelNotFound: 标签不存在
 * - ImapFolderMetadataFailed: IMAP 文件夹元数据失败
 * - ImapSearchFailed: IMAP 搜索失败
 * - BatchFetchHeadersFailed: 批量获取邮件头失败
 * - ImapError: 通用 IMAP 错误
 * - EmailMissingUid: 邮件缺少 UID
 */
export function formatError(error: unknown): string {
  // 检查错误是否是 Error 对象的实例
  // 这是最常见的错误类型，包括 Tauri 抛出的错误
  if (error instanceof Error) {
    // 获取错误的消息字符串
    const msg = error.message;

    // 尝试解析结构化错误 JSON
    // Tauri 的错误可能以 JSON 格式返回，包含详细的错误信息
    // 例如: {"type":"AuthFailed","message":"用户名或密码错误"}
    try {
      // 使用 JSON.parse 尝试解析消息
      const parsed = JSON.parse(msg);

      // 如果解析成功且包含 message 字段，返回该消息
      // 这样可以提取 JSON 中的人类可读错误描述
      if (parsed.message) {
        return parsed.message;
      }
    } catch {
      // JSON 解析失败，说明消息不是 JSON 格式
      // 继续执行，直接返回原始消息
      // 使用空的 catch 块，因为这是预期的正常流程
    }

    // 如果不是 JSON 或解析失败，直接返回原始消息
    return msg;
  }

  // 如果错误是字符串类型，直接返回
  // 这可能是手动抛出的字符串错误
  if (typeof error === "string") {
    return error;
  }

  // 对于其他未知类型的错误（null、undefined、对象等）
  // 返回默认的通用错误消息
  // 这样可以确保函数总是返回一个字符串，不会导致后续代码出错
  return "未知错误";
}

/**
 * 全局未处理错误处理器设置
 *
 * 此函数设置全局的事件监听器，用于捕获和处理未被其他代码
 * 处理的错误和 Promise rejection。这是应用的最后一道错误防线。
 *
 * ==================== 功能说明 ====================
 * 1. 监听 unhandledrejection 事件：捕获未处理的 Promise rejection
 * 2. 监听 error 事件：捕获未捕获的异常
 * 3. 将错误记录到控制台：便于开发者调试
 * 4. 防止应用崩溃：确保即使有未处理的错误，应用也能继续运行
 *
 * ==================== 使用方法 ====================
 * 通常在应用的根布局组件的 onMount 中调用：
 * ```typescript
 * // 在 src/routes/+layout.svelte 中
 * import { onMount } from 'svelte';
 * import { setupGlobalErrorHandler } from '$lib/utils/error';
 *
 * onMount(() => {
 *   setupGlobalErrorHandler();
 *   // 其他初始化代码...
 * });
 * ```
 *
 * ==================== 工作原理 ====================
 *
 * 1. unhandledrejection 事件：
 *    - 当 Promise 被 reject 但没有 .catch() 处理时触发
 *    - event.reason 包含被 reject 的值
 *    - 在现代浏览器和 Node.js 中都支持
 *
 * 2. error 事件：
 *    - 当 JavaScript 运行时错误未被捕获时触发
 *    - event.error 包含错误对象
 *    - 这是传统的全局错误处理机制
 *
 * ==================== 代码示例 ====================
 *
 * 基本用法（在应用入口调用）：
 * ```typescript
 * import { setupGlobalErrorHandler } from '$lib/utils/error';
 *
 * // 在应用启动时调用
 * setupGlobalErrorHandler();
 *
 * // 这里的错误会被捕获：
 * Promise.reject('测试错误');  // 没有 .catch()
 *
 * throw new Error('未捕获的异常');  // 没有 try-catch
 * ```
 *
 * 在 SvelteKit 应用中：
 * ```typescript
 * // src/routes/+layout.svelte
 * <script lang="ts">
 *   import { onMount } from 'svelte';
 *   import { setupGlobalErrorHandler } from '$lib/utils/error';
 *
 *   onMount(() => {
 *     // 设置全局错误处理
 *     setupGlobalErrorHandler();
 *   });
 * </script>
 * ```
 *
 * 与 Toast 提示结合：
 * ```typescript
 * import { toast } from '$lib/stores/toast.svelte';
 * import { formatError } from './formatError';
 *
 * export function setupGlobalErrorHandler() {
 *   // 处理 Promise rejection
 *   window.addEventListener('unhandledrejection', (event) => {
 *     console.error('Unhandled promise rejection:', event.reason);
 *
 *     // 显示用户提示
 *     const message = formatError(event.reason);
 *     toast.error(`操作失败: ${message}`);
 *
 *     // 阻止默认的控制台警告（已手动记录）
 *     event.preventDefault();
 *   });
 *
 *   // 处理未捕获的异常
 *   window.addEventListener('error', (event) => {
 *     console.error('Unhandled error:', event.error);
 *
 *     // 显示用户提示
 *     const message = formatError(event.error);
 *     toast.error(`发生错误: ${message}`);
 *   });
 * }
 * ```
 *
 * 与错误上报服务结合：
 * ```typescript
 * import * as Sentry from '@sentry/svelte';
 *
 * export function setupGlobalErrorHandler() {
 *   window.addEventListener('unhandledrejection', (event) => {
 *     console.error('Unhandled promise rejection:', event.reason);
 *
 *     // 发送到 Sentry 等错误监控服务
 *     Sentry.captureException(event.reason);
 *   });
 *
 *   window.addEventListener('error', (event) => {
 *     console.error('Unhandled error:', event.error);
 *
 *     // 发送到 Sentry 等错误监控服务
 *     Sentry.captureException(event.error);
 *   });
 * }
 * ```
 *
 * ==================== 注意事项 ====================
 * 1. 只调用一次：避免重复添加事件监听器，最好在应用启动时调用一次
 * 2. 性能影响：错误事件监听器不会影响正常性能，只会在错误时触发
 * 3. 开发环境：在生产环境中，应该将错误上报到监控系统
 * 4. 用户体验：可以考虑在错误发生时显示用户友好的提示
 * 5. 调试信息：console.error 会包含完整的堆栈跟踪，便于调试
 *
 * ==================== 最佳实践 ====================
 * 1. 在应用入口处调用（如 +layout.svelte 的 onMount）
 * 2. 结合错误监控服务（如 Sentry）记录生产环境错误
 * 3. 提供用户反馈机制（如 Toast 提示）
 * 4. 区分开发和生产环境的错误处理策略
 * 5. 定期检查未处理错误，修复潜在的 bug
 *
 * ==================== 为什么需要这个函数 ====================
 * 1. 防止应用崩溃：未处理的错误可能导致应用无响应
 * 2. 提升用户体验：即使出错，也要让用户知道发生了什么
 * 3. 便于调试：所有错误都会被记录，便于定位问题
 * 4. 合规性：某些应用框架要求必须处理全局错误
 * 5. 错误监控：可以集成第三方服务进行错误追踪和分析
 */
export function setupGlobalErrorHandler() {
  // 监听未处理的 Promise rejection
  // 当 Promise 被 reject 但没有 .catch() 处理时触发
  // 这是处理异步代码错误的重要机制
  window.addEventListener("unhandledrejection", (event) => {
    // 将完整的错误信息输出到控制台
    // 包含错误原因和堆栈跟踪
    console.error("Unhandled promise rejection:", event.reason);

    // 注意：这里可以添加更多的错误处理逻辑
    // 例如：显示 Toast 提示、发送到错误监控服务等
    // event.preventDefault();  // 阻止默认行为（可选）
  });

  // 监听未捕获的异常
  // 当 JavaScript 运行时错误未被 try-catch 捕获时触发
  // 这是处理同步代码错误的重要机制
  window.addEventListener("error", (event) => {
    // 将完整的错误信息输出到控制台
    // 包含错误对象、文件名、行号、列号等
    console.error("Unhandled error:", event.error);

    // 注意：这里可以添加更多的错误处理逻辑
    // 例如：显示 Toast 提示、发送到错误监控服务等
  });
}
