/**
 * Postium Mail - 国际化（i18n）状态管理模块
 * i18n.svelte.ts
 *
 * 本模块提供应用的国际化状态管理功能，支持多语言切换。
 * 使用 Svelte 5 的 Runes API 实现响应式状态，配合 Svelte Context API 实现跨组件共享。
 *
 * ==================== 功能概述 ====================
 * 1. 管理当前语言设置（zh-CN / en-US）
 * 2. 提供响应式的翻译函数 t，自动根据语言切换显示文本
 * 3. 将语言偏好持久化到 localStorage
 * 4. 使用 Svelte Context API 实现全局状态共享
 *
 * ==================== 支持的语言 ====================
 * - zh-CN：简体中文
 * - en-US：英语（美国）
 *
 * ==================== 设计原则 ====================
 * 1. 类型安全：翻译文本使用 const assertion，确保所有键名在编译时检查
 * 2. 宽松类型：使用 LooseLiteral 将字面量类型转换为 string，便于赋值
 * 3. 响应式：使用 $state 和 $derived Runes 实现自动更新
 * 4. 持久化：语言设置自动保存到 localStorage，刷新页面后恢复
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createI18nState } from '$lib/stores/i18n.svelte';
 * const i18n = createI18nState();
 *
 * // 在子组件中获取状态
 * import { getI18nState } from '$lib/stores/i18n.svelte';
 * const i18n = getI18nState();
 *
 * // 在模板中使用翻译
 * <h1>{i18n.t.sidebar.inbox}</h1>
 *
 * // 切换语言
 * i18n.setLocale('en-US');
 * ```
 */

// 导入中文翻译文件
import zhCN from "$lib/i18n/zh-CN";
// 导入英文翻译文件
import enUS from "$lib/i18n/en-US";
// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

/**
 * 语言区域类型
 *
 * 定义应用支持的所有语言标识符。
 * 每个值对应一个翻译文件。
 *
 * ==================== 取值说明 ====================
 * - 'zh-CN'：简体中文（中国大陆）
 * - 'en-US'：英语（美国）
 *
 * 扩展新语言时，需要：
 * 1. 在此类型中添加新的标识符
 * 2. 创建对应的翻译文件（如 ja-JP.ts）
 * 3. 在 translations 对象中注册新语言
 */
export type Locale = "zh-CN" | "en-US";

/**
 * 递归宽松字面量类型
 *
 * 将嵌套对象结构中的所有字符串字面量类型转换为 string 类型，
 * 同时保持对象的结构不变。
 *
 * ==================== 类型转换规则 ====================
 * - string 字面量类型（如 '收件箱'）→ string
 * - 数组类型 → 递归转换数组元素类型
 * - 对象类型 → 递归转换所有属性值类型
 * - 其他类型（number, boolean 等）→ 保持不变
 *
 * ==================== 为什么需要这个类型 ====================
 * 翻译文件使用 `as const` 断言，所有字符串值都会被推断为字面量类型
 * （例如 `'收件箱'` 而不是 `string`）。虽然这对类型安全有好处，
 * 但在赋值时会受到严格限制。
 *
 * 使用 LooseLiteral 将字面量类型"松开"为 string，使得翻译对象
 * 可以自由赋值，同时保持嵌套结构不变。
 *
 * ==================== 类型推断示例 ====================
 * ```typescript
 * // 原始类型（as const）
 * { app: { name: 'Postium Mail' } }
 * // → { readonly app: { readonly name: 'Postium Mail' } }
 *
 * // 经过 LooseLiteral 转换后
 * // → { app: { name: string } }
 * ```
 *
 * @typeParam T - 要转换的输入类型
 */
type LooseLiteral<T> = T extends string
  ? string
  : T extends readonly (infer U)[]
    ? LooseLiteral<U>[]
    : T extends object
      ? { [K in keyof T]: LooseLiteral<T[K]> }
      : T;

/**
 * 翻译文本的类型定义
 *
 * 基于中文翻译文件的结构，使用 LooseLiteral 将字面量类型转换为 string。
 * 所有翻译文件必须遵循此类型定义的结构。
 *
 * 这确保了：
 * 1. 所有语言的翻译文件结构一致
 * 2. 可以通过点号语法访问嵌套的翻译键（如 t.sidebar.inbox）
 * 3. 值的类型为 string，便于赋值和使用
 */
type Translation = LooseLiteral<typeof zhCN>;

/**
 * 翻译文本映射表
 *
 * 将每个语言标识符映射到对应的翻译文本对象。
 * 在切换语言时，从此对象中获取对应的翻译文本。
 *
 * ==================== 注册新语言 ====================
 * 添加新语言时，需要在此对象中添加新的键值对：
 * ```typescript
 * const translations: Record<Locale, Translation> = {
 *   'zh-CN': zhCN,
 *   'en-US': enUS,
 *   'ja-JP': jaJP,  // 新增日语
 * };
 * ```
 */
const translations: Record<Locale, Translation> = {
  "zh-CN": zhCN,
  "en-US": enUS,
};

/**
 * 国际化状态管理类
 *
 * 管理应用的语言设置和翻译文本。使用 Svelte 5 的 Runes API
 * 实现响应式状态，语言切换后 UI 自动更新。
 *
 * ==================== 状态字段 ====================
 * - `locale`：当前语言标识符（响应式）
 * - `t`：当前语言的翻译文本对象（派生状态，自动计算）
 *
 * ==================== 持久化策略 ====================
 * - 语言偏好保存在 localStorage 中，键名为 'postium-locale'
 * - 应用启动时自动读取并恢复上次的语言设置
 * - 切换语言时自动保存到 localStorage
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 I18nState 实例
 * 2. 构造函数中从 localStorage 读取保存的语言偏好
 * 3. 通过 setContext 将实例存入 Svelte Context
 * 4. 子组件通过 getContext 获取同一个实例
 * 5. 语言切换时，t 属性自动更新，UI 随之刷新
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createI18nState() 创建
 * - t 是派生状态，会根据 locale 自动计算，不要手动赋值
 */
class I18nState {
  /**
   * 当前语言标识符（响应式状态）
   *
   * 默认值为 'zh-CN'（简体中文）。
   * 在构造函数中会尝试从 localStorage 恢复用户上次的选择。
   *
   * 切换此值后，t（翻译文本）会自动更新为对应语言的内容。
   */
  locale = $state<Locale>("zh-CN");

  /**
   * 当前语言的翻译文本对象（派生状态）
   *
   * 根据 locale 自动从 translations 中获取对应语言的翻译文本。
   * 在模板中使用点号语法访问嵌套的翻译键。
   *
   * ==================== 使用示例 ====================
   * ```svelte
   * <script>
   *   const i18n = getI18nState();
   * </script>
   *
   * <h1>{i18n.t.sidebar.inbox}</h1>       <!-- 收件箱 -->
   * <p>{i18n.t.email.search}</p>          <!-- 搜索邮件... -->
   * <button>{i18n.t.common.save}</button> <!-- 保存 -->
   * ```
   *
   * @returns 当前语言的翻译文本对象
   */
  t = $derived(translations[this.locale]);

  /**
   * 构造函数
   *
   * 初始化国际化状态，尝试从 localStorage 恢复用户上次的语言偏好。
   * 如果 localStorage 中没有保存的设置，使用默认语言（zh-CN）。
   *
   * ==================== 工作流程 ====================
   * 1. 从 localStorage 读取 'postium-locale' 的值
   * 2. 检查该值是否为有效的语言标识符（存在于 translations 中）
   * 3. 如果有效，设置为该语言；否则保持默认值
   */
  constructor() {
    // 从 localStorage 读取保存的语言偏好
    const saved = localStorage.getItem("postium-locale") as Locale | null;

    // 验证保存的语言标识符是否有效（对应的翻译文件存在）
    if (saved && translations[saved]) {
      this.locale = saved;
    }
    // 如果没有保存的设置或设置无效，保持默认值 'zh-CN'
  }

  /**
   * 切换语言
   *
   * 切换应用界面显示的语言，并将选择持久化到 localStorage。
   * 切换后，所有使用 t 属性的 UI 文本会自动更新。
   *
   * ==================== 参数说明 ====================
   * @param locale - 目标语言标识符，必须是 Locale 类型中定义的值
   *                目前支持：'zh-CN'（中文）和 'en-US'（英文）
   *
   * ==================== 副作用 ====================
   * - 更新 locale 响应式状态
   * - t 派生状态自动重新计算
   * - 所有绑定到 t 的 UI 元素自动更新
   * - 语言偏好保存到 localStorage
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const i18n = getI18nState();
   *
   * // 切换到英文
   * i18n.setLocale('en-US');
   * console.log(i18n.t.sidebar.inbox); // 'Inbox'
   *
   * // 切换回中文
   * i18n.setLocale('zh-CN');
   * console.log(i18n.t.sidebar.inbox); // '收件箱'
   * ```
   */
  setLocale(locale: Locale) {
    // 更新响应式语言状态
    this.locale = locale;
    // 持久化到 localStorage，键名为 'postium-locale'
    localStorage.setItem("postium-locale", locale);
  }
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const I18N_KEY = Symbol("i18n");

/**
 * 创建国际化状态管理器
 *
 * 创建一个新的 I18nState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getI18nState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createI18nState } from '$lib/stores/i18n.svelte';
 *
 *   // 创建并设置 Context
 *   const i18n = createI18nState();
 * </script>
 *
 * <!-- 使用翻译文本 -->
 * <h1>{i18n.t.app.name}</h1>
 * ```
 *
 * @returns I18nState 实例
 */
export function createI18nState() {
  const state = new I18nState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(I18N_KEY, state);
  return state;
}

/**
 * 获取国际化状态管理器
 *
 * 从 Svelte Context 中获取由 createI18nState() 创建的 I18nState 实例。
 * 此函数应在子组件中调用，以访问全局的国际化状态和翻译文本。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createI18nState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createI18nState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- SomeComponent.svelte -->
 * <script lang="ts">
 *   import { getI18nState } from '$lib/stores/i18n.svelte';
 *
 *   // 获取全局国际化状态
 *   const i18n = getI18nState();
 * </script>
 *
 * <button>{i18n.t.common.save}</button>
 * <select onchange={(e) => i18n.setLocale(e.target.value)}>
 *   <option value="zh-CN">中文</option>
 *   <option value="en-US">English</option>
 * </select>
 * ```
 *
 * @returns I18nState 实例
 * @throws 如果在 createI18nState() 之前调用，会抛出 Context 错误
 */
export function getI18nState() {
  return getContext<I18nState>(I18N_KEY);
}
