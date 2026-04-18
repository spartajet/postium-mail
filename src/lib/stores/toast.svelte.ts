/**
 * Postium Mail - 提示消息（Toast）状态管理模块
 * toast.svelte.ts
 *
 * 本模块提供应用全局的提示消息（Toast）管理功能。
 * 使用 Svelte 5 的 Runes API 实现响应式状态，配合 Svelte Context API 实现跨组件共享。
 *
 * ==================== 功能概述 ====================
 * 1. 管理全局 Toast 消息的创建、显示和关闭
 * 2. 支持四种消息类型：成功（success）、错误（error）、信息（info）、警告（warning）
 * 3. 每种消息类型有不同的默认显示时长
 * 4. 支持手动关闭消息
 * 5. 使用 Svelte Context API 实现跨组件状态共享
 *
 * ==================== 消息类型与默认时长 ====================
 * | 类型    | 默认时长 | 用途                         |
 * |---------|----------|------------------------------|
 * | success | 3000ms   | 操作成功提示（如保存成功）     |
 * | error   | 5000ms   | 错误提示（如网络请求失败）     |
 * | info    | 3000ms   | 一般信息提示                   |
 * | warning | 4000ms   | 警告提示（如输入验证未通过）   |
 *
 * ==================== 设计原则 ====================
 * 1. 轻量级：Toast 消息是短暂的通知，不阻断用户操作
 * 2. 自动消失：消息在指定时长后自动关闭（由 Toast 组件处理定时器）
 * 3. 类型安全：完整的 TypeScript 类型定义
 * 4. 响应式：使用 $state Runes 实现消息列表的自动更新
 * 5. 单例模式：通过 Svelte Context 确保全局只有一个 ToastState 实例
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createToastState } from '$lib/stores/toast.svelte';
 * const toastState = createToastState();
 *
 * // 在子组件中获取状态
 * import { getToastState } from '$lib/stores/toast.svelte';
 * const toast = getToastState();
 *
 * // 显示不同类型的消息
 * toast.success('保存成功');
 * toast.error('网络连接失败');
 * toast.info('新邮件到达');
 * toast.warning('磁盘空间不足');
 *
 * // 手动关闭消息
 * const id = toast.show('可关闭的消息', 'info');
 * toast.dismiss(id);
 * ```
 */

// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

/**
 * Toast 消息类型
 *
 * 定义 Toast 消息的所有支持类型，每种类型对应不同的视觉样式和默认显示时长。
 *
 * ==================== 取值说明 ====================
 * - 'success'：成功提示，通常显示为绿色，用于操作成功的反馈
 * - 'error'：错误提示，通常显示为红色，用于操作失败的反馈
 * - 'info'：信息提示，通常显示为蓝色，用于一般性的信息通知
 * - 'warning'：警告提示，通常显示为黄色/橙色，用于需要注意的情况
 *
 * ==================== UI 映射 ====================
 * 不同类型的 Toast 通常会有不同的图标和背景色：
 * - success: ✓ 图标 + 绿色背景
 * - error: ✗ 图标 + 红色背景
 * - info: ℹ 图标 + 蓝色背景
 * - warning: ⚠ 图标 + 黄色/橙色背景
 */
export type ToastType = "success" | "error" | "info" | "warning";

/**
 * Toast 消息接口
 *
 * 定义单条 Toast 消息的数据结构。
 * 每条消息都有唯一的 ID、类型、文本内容和显示时长。
 *
 * ==================== 字段说明 ====================
 * - `id`：消息的唯一标识符，用于手动关闭消息时引用
 * - `type`：消息类型，决定消息的视觉样式
 * - `message`：消息文本内容，显示给用户看的文字
 * - `duration`：显示时长（毫秒），消息在此时间后自动关闭
 *
 * ==================== 使用场景 ====================
 * 此接口主要用于 Toast 组件内部的消息列表管理。
 * 外部调用者通常通过 ToastState 的方法间接创建 Toast 对象。
 *
 * ```typescript
 * // 内部创建示例（由 ToastState.show() 方法执行）
 * const toast: Toast = {
 *   id: 0,
 *   type: 'success',
 *   message: '操作成功',
 *   duration: 3000
 * };
 * ```
 */
export interface Toast {
  /** 消息的唯一标识符，自增生成，用于 dismiss() 方法关闭指定消息 */
  id: number;
  /** 消息类型，决定消息的视觉样式（颜色、图标） */
  type: ToastType;
  /** 消息文本内容，支持纯文本 */
  message: string;
  /** 显示时长（毫秒），超时后消息自动关闭。0 表示不自动关闭 */
  duration: number;
}

/**
 * Toast 状态管理类
 *
 * 管理应用中所有 Toast 消息的生命周期，包括创建、显示和关闭。
 * 使用 Svelte 5 的 Runes API ($state) 实现响应式状态管理。
 *
 * ==================== 状态字段 ====================
 * - `toasts`：当前显示中的所有 Toast 消息列表（响应式）
 * - `nextId`：下一个消息的 ID（私有，自增生成）
 *
 * ==================== 消息生命周期 ====================
 * 1. 调用 show() 或快捷方法（success/error/info/warning）创建消息
 * 2. 消息被添加到 toasts 列表，Toast 组件渲染该消息
 * 3. Toast 组件根据 duration 设置定时器
 * 4. 定时器到期后，Toast 组件调用 dismiss() 关闭消息
 * 5. 或者用户手动点击关闭按钮调用 dismiss() 关闭消息
 * 6. 消息从 toasts 列表中移除
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 ToastState 实例
 * 2. 通过 setContext 将实例存入 Svelte Context
 * 3. 子组件通过 getContext 获取同一个实例
 * 4. Toast 渲染组件监听 toasts 列表，自动显示新消息
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createToastState() 创建
 * - toasts 列表是响应式的，Toast 组件应使用 {#each} 遍历渲染
 * - duration 的自动关闭逻辑由 Toast UI 组件负责，此类只存储时长数据
 */
class ToastState {
  /**
   * 当前显示中的 Toast 消息列表（响应式状态）
   *
   * 所有活跃的 Toast 消息都存储在此数组中。
   * Toast 渲染组件应遍历此数组来显示每条消息。
   *
   * ==================== 响应式行为 ====================
   * - 添加新消息时（show()），Toast 组件自动渲染新消息
   * - 关闭消息时（dismiss()），Toast 组件自动移除对应消息
   *
   * ==================== 使用示例 ====================
   * ```svelte
   * <script>
   *   const toast = getToastState();
   * </script>
   *
   * {#each toast.toasts as item (item.id)}
   *   <div class="toast toast-{item.type}">
   *     {item.message}
   *     <button onclick={() => toast.dismiss(item.id)}>×</button>
   *   </div>
   * {/each}
   * ```
   */
  toasts = $state<Toast[]>([]);

  /**
   * 下一个消息的自增 ID（私有）
   *
   * 每次创建新消息时使用当前值，然后自增。
   * 从 0 开始，确保每个消息都有唯一的标识符。
   *
   * ==================== 为什么不用 UUID ====================
   * - ID 只用于组件内的 key 和 dismiss() 引用，不需要全局唯一
   * - 自增整数比 UUID 更高效，且更易于调试
   * - 在应用的生命周期内，不会产生 ID 冲突
   */
  private nextId = 0;

  /**
   * 显示一条 Toast 消息
   *
   * 创建一条新的 Toast 消息并添加到消息列表中。
   * 这是 Toast 系统的核心方法，其他快捷方法（success/error/info/warning）
   * 都是通过调用此方法实现的。
   *
   * ==================== 参数说明 ====================
   * @param message - 消息文本内容，显示给用户的提示文字
   * @param type - 消息类型，决定视觉样式。默认为 'info'
   *              - 'success'：成功提示（绿色）
   *              - 'error'：错误提示（红色）
   *              - 'info'：信息提示（蓝色）
   *              - 'warning'：警告提示（黄色）
   * @param duration - 显示时长（毫秒），超时后自动关闭。默认为 3000ms（3秒）
   *                   - success: 建议 3000ms
   *                   - error: 建议 5000ms（错误信息需要更多阅读时间）
   *                   - info: 建议 3000ms
   *                   - warning: 建议 4000ms
   *                   - 设为 0 可禁用自动关闭
   *
   * ==================== 返回值说明 ====================
   * @returns 消息的唯一 ID，可用于后续调用 dismiss() 关闭该消息
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   *
   * // 显示默认信息提示
   * toast.show('欢迎使用 Postium Mail');
   *
   * // 显示自定义类型的提示
   * toast.show('正在同步...', 'info', 5000);
   *
   * // 显示并记录 ID，稍后手动关闭
   * const id = toast.show('处理中...', 'info', 0); // 不自动关闭
   * // ... 操作完成后
   * toast.dismiss(id);
   * ```
   */
  show(message: string, type: ToastType = "info", duration = 3000) {
    // 生成唯一 ID 并自增计数器
    const id = this.nextId++;
    // 创建 Toast 对象并添加到消息列表
    this.toasts.push({ id, type, message, duration });
    return id;
  }

  /**
   * 显示成功提示消息
   *
   * 快捷方法，显示一条成功类型的 Toast 消息。
   * 默认显示时长为 3000ms（3秒）。
   *
   * ==================== 适用场景 ====================
   * - 表单提交成功
   * - 设置保存成功
   * - 邮件发送成功
   * - 账号添加成功
   * - 同步完成
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   * toast.success('邮件已发送');
   * toast.success('设置已保存');
   * ```
   *
   * @param message - 成功提示的文本内容
   * @returns 消息的唯一 ID
   */
  success(message: string) {
    return this.show(message, "success");
  }

  /**
   * 显示错误提示消息
   *
   * 快捷方法，显示一条错误类型的 Toast 消息。
   * 默认显示时长为 5000ms（5秒），比其他类型更长，
   * 因为错误信息通常需要更多时间让用户阅读和理解。
   *
   * ==================== 适用场景 ====================
   * - 网络请求失败
   * - 表单验证失败
   * - 认证失败（密码错误、Token 过期）
   * - 同步失败
   * - 文件操作失败
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   * toast.error('网络连接失败，请检查网络设置');
   * toast.error('登录失败：密码错误');
   * ```
   *
   * @param message - 错误提示的文本内容
   * @returns 消息的唯一 ID
   */
  error(message: string) {
    return this.show(message, "error", 5000);
  }

  /**
   * 显示信息提示消息
   *
   * 快捷方法，显示一条信息类型的 Toast 消息。
   * 默认显示时长为 3000ms（3秒）。
   *
   * ==================== 适用场景 ====================
   * - 新邮件到达通知
   * - 操作进行中提示
   * - 系统状态变化通知
   * - 一般性的用户提示
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   * toast.info('您有 3 封新邮件');
   * toast.info('正在后台同步...');
   * ```
   *
   * @param message - 信息提示的文本内容
   * @returns 消息的唯一 ID
   */
  info(message: string) {
    return this.show(message, "info");
  }

  /**
   * 显示警告提示消息
   *
   * 快捷方法，显示一条警告类型的 Toast 消息。
   * 默认显示时长为 4000ms（4秒），介于信息和错误之间。
   *
   * ==================== 适用场景 ====================
   * - 磁盘空间不足
   * - 附件大小超过限制
   * - 未保存的更改即将丢失
   * - 不安全的连接警告
   * - 密码即将过期
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   * toast.warning('存储空间不足，请清理邮件');
   * toast.warning('附件大小超过 25MB 限制');
   * ```
   *
   * @param message - 警告提示的文本内容
   * @returns 消息的唯一 ID
   */
  warning(message: string) {
    return this.show(message, "warning", 4000);
  }

  /**
   * 关闭指定的 Toast 消息
   *
   * 从消息列表中移除指定 ID 的 Toast 消息。
   * 通常由 Toast UI 组件的关闭按钮调用，或在自动关闭定时器到期时调用。
   *
   * ==================== 工作原理 ====================
   * 使用 Array.filter() 创建一个新数组，排除指定 ID 的消息。
   * 由于 toasts 是 $state 响应式状态，赋值新数组后 UI 自动更新。
   *
   * ==================== 参数说明 ====================
   * @param id - 要关闭的消息 ID，由 show() 或快捷方法返回
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const toast = getToastState();
   *
   * // 显示消息并记录 ID
   * const id = toast.info('正在处理...');
   *
   * // 稍后手动关闭
   * toast.dismiss(id);
   * ```
   *
   * ==================== 注意事项 ====================
   * - 如果指定的 ID 不存在（消息已被关闭），操作会安全地忽略
   * - 关闭消息后，Toast 组件应播放退出动画后再移除 DOM 元素
   */
  dismiss(id: number) {
    // 过滤掉指定 ID 的消息，创建新数组
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const TOAST_KEY = Symbol("toast");

/**
 * 创建 Toast 状态管理器
 *
 * 创建一个新的 ToastState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getToastState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createToastState } from '$lib/stores/toast.svelte';
 *
 *   // 创建并设置 Context
 *   const toastState = createToastState();
 * </script>
 *
 * <!-- Toast 渲染组件 -->
 * {#each toastState.toasts as item (item.id)}
 *   <div class="toast toast-{item.type}">
 *     {item.message}
 *   </div>
 * {/each}
 * ```
 *
 * @returns ToastState 实例
 */
export function createToastState() {
  const state = new ToastState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(TOAST_KEY, state);
  return state;
}

/**
 * 获取 Toast 状态管理器
 *
 * 从 Svelte Context 中获取由 createToastState() 创建的 ToastState 实例。
 * 此函数应在子组件中调用，以访问全局的 Toast 消息管理功能。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createToastState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createToastState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- EmailActions.svelte -->
 * <script lang="ts">
 *   import { getToastState } from '$lib/stores/toast.svelte';
 *
 *   // 获取全局 Toast 状态
 *   const toast = getToastState();
 *
 *   async function handleSend() {
 *     try {
 *       await sendEmail(emailData);
 *       toast.success('邮件发送成功');
 *     } catch (error) {
 *       toast.error('发送失败: ' + error.message);
 *     }
 *   }
 * </script>
 *
 * <button onclick={handleSend}>发送邮件</button>
 * ```
 *
 * @returns ToastState 实例
 * @throws 如果在 createToastState() 之前调用，会抛出 Context 错误
 */
export function getToastState() {
  return getContext<ToastState>(TOAST_KEY);
}
