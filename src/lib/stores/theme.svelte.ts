/**
 * Postium Mail - 主题状态管理模块
 * theme.svelte.ts
 *
 * 本模块提供应用的主题切换功能，支持亮色、暗色和跟随系统三种模式。
 * 使用 Svelte 5 的 Runes API 实现响应式状态，配合 Svelte Context API 实现跨组件共享。
 *
 * ==================== 功能概述 ====================
 * 1. 管理应用的主题模式（亮色 / 暗色 / 跟随系统）
 * 2. 自动解析"跟随系统"模式为实际的亮色或暗色主题
 * 3. 监听操作系统主题变更，实时切换应用主题
 * 4. 将主题偏好持久化到 localStorage
 * 5. 操作 HTML 根元素的 CSS 类名，驱动 Tailwind CSS 暗色模式
 *
 * ==================== 支持的主题模式 ====================
 * - 'light'：亮色模式，始终使用浅色界面
 * - 'dark'：暗色模式，始终使用深色界面
 * - 'system'：跟随系统，自动匹配操作系统的主题设置
 *
 * ==================== 设计原则 ====================
 * 1. 响应式：使用 $state Runes 实现主题切换后自动更新 UI
 * 2. 持久化：主题设置保存到 localStorage，刷新页面后恢复
 * 3. 系统联动：监听 prefers-color-scheme 媒体查询，实时响应系统主题变化
 * 4. Tailwind 集成：通过切换 HTML 根元素的 dark 类名实现暗色模式
 * 5. 单例模式：通过 Svelte Context 确保全局只有一个 ThemeState 实例
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createThemeState } from '$lib/stores/theme.svelte';
 * const themeState = createThemeState();
 *
 * // 在子组件中获取状态
 * import { getThemeState } from '$lib/stores/theme.svelte';
 * const themeState = getThemeState();
 *
 * // 切换主题
 * themeState.setTheme('dark');
 *
 * // 读取当前解析后的主题
 * console.log(themeState.resolved); // 'dark'
 * ```
 */

// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

/**
 * 主题模式类型
 *
 * 定义应用支持的所有主题模式。
 *
 * ==================== 取值说明 ====================
 * - 'light'：亮色模式，使用浅色背景和深色文字
 * - 'dark'：暗色模式，使用深色背景和浅色文字
 * - 'system'：跟随系统，自动匹配操作系统的主题偏好设置
 *
 * ==================== 扩展说明 ====================
 * 如需添加新的主题模式（如高对比度模式），可以在此类型中添加新值，
 * 并在 ThemeState 中实现对应的解析逻辑。
 */
type Theme = "light" | "dark" | "system";

/**
 * 主题状态管理类
 *
 * 管理应用的主题设置，包括用户选择的主题模式和实际解析后的主题。
 * 使用 Svelte 5 的 Runes API ($state) 实现响应式状态管理。
 *
 * ==================== 状态字段 ====================
 * - `theme`：用户选择的主题模式（响应式），可以是 'light'、'dark' 或 'system'
 * - `resolved`：解析后的实际主题（响应式），只能是 'light' 或 'dark'
 *
 * ==================== 核心概念 ====================
 * **theme vs resolved 的区别：**
 * - `theme` 是用户的意图（"我想用暗色" 或 "我想跟随系统"）
 * - `resolved` 是实际应用的主题（"当前界面是暗色" 或 "当前界面是亮色"）
 * - 当 theme 为 'system' 时，resolved 根据操作系统设置动态决定
 * - 当 theme 为 'light' 或 'dark' 时，resolved 与 theme 相同
 *
 * ==================== 持久化策略 ====================
 * - 主题偏好保存在 localStorage 中，键名为 'postium-theme'
 * - 应用启动时自动读取并恢复上次的主题设置
 * - 切换主题时自动保存到 localStorage
 *
 * ==================== Tailwind CSS 暗色模式集成 ====================
 * 本模块通过操作 HTML 根元素（<html>）的 CSS 类名来控制暗色模式：
 * - 当 resolved 为 'dark' 时，添加 'dark' 类名
 * - 当 resolved 为 'light' 时，移除 'dark' 类名
 *
 * 这与 Tailwind CSS 的 class 策略配合使用：
 * ```html
 * <!-- Tailwind 配置 -->
 * darkMode: 'class'
 * ```
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 ThemeState 实例
 * 2. 构造函数中从 localStorage 恢复主题设置
 * 3. 注册系统主题变更监听器
 * 4. 应用初始主题类名到 HTML 根元素
 * 5. 通过 setContext 将实例存入 Svelte Context
 * 6. 子组件通过 getContext 获取同一个实例
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createThemeState() 创建
 * - resolved 是由 theme 和系统设置共同决定的，不要手动赋值
 * - 系统主题监听器只在 theme 为 'system' 时触发更新
 */
class ThemeState {
  /**
   * 用户选择的主题模式（响应式状态）
   *
   * 表示用户的主题偏好设置，默认值为 'system'（跟随系统）。
   * 在构造函数中会尝试从 localStorage 恢复用户上次的选择。
   *
   * ==================== 取值说明 ====================
   * - 'light'：亮色模式
   * - 'dark'：暗色模式
   * - 'system'：跟随操作系统设置（默认值）
   */
  theme = $state<Theme>("system");

  /**
   * 解析后的实际主题（响应式状态）
   *
   * 根据 theme 和操作系统设置计算出的实际主题。
   * UI 组件应根据此值决定使用亮色还是暗色样式。
   *
   * ==================== 计算逻辑 ====================
   * - theme 为 'light' → resolved 为 'light'
   * - theme 为 'dark' → resolved 为 'dark'
   * - theme 为 'system' → 根据操作系统偏好决定
   *   - 系统为暗色 → resolved 为 'dark'
   *   - 系统为亮色 → resolved 为 'light'
   *
   * ==================== 使用场景 ====================
   * 用于在模板中条件渲染不同主题的样式：
   * ```svelte
   * {#if themeState.resolved === 'dark'}
   *   <DarkIcon />
   * {:else}
   *   <LightIcon />
   * {/if}
   * ```
   */
  resolved = $state<"light" | "dark">("light");

  /**
   * 构造函数
   *
   * 初始化主题状态，恢复用户保存的主题偏好，并设置系统主题监听器。
   *
   * ==================== 工作流程 ====================
   * 1. 从 localStorage 读取保存的主题偏好
   * 2. 如果有保存的设置，应用该主题
   * 3. 获取系统的 prefers-color-scheme 媒体查询对象
   * 4. 计算当前解析后的主题
   * 5. 注册系统主题变更监听器（仅 theme 为 'system' 时生效）
   * 6. 将主题类名应用到 HTML 根元素
   *
   * ==================== 系统主题监听 ====================
   * 当用户的操作系统主题发生变化时（如在系统设置中切换亮/暗色），
   * 如果应用设置为 'system' 模式，会自动更新 resolved 并刷新 UI。
   */
  constructor() {
    // 从 localStorage 读取保存的主题偏好
    const saved = localStorage.getItem("postium-theme") as Theme | null;

    // 如果有保存的设置，恢复该主题
    if (saved) {
      this.theme = saved;
    }

    // 获取系统主题偏好的媒体查询对象
    // prefers-color-scheme: dark 在系统为暗色模式时匹配
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

    // 根据 theme 设置计算初始的 resolved 值
    this.resolved = this.getResolved();

    // 监听系统主题变更
    mediaQuery.addEventListener("change", () => {
      // 仅在 'system' 模式下响应系统主题变更
      // 'light' 和 'dark' 模式不受系统设置影响
      if (this.theme === "system") {
        this.resolved = this.getResolved();
        this.applyClass();
      }
    });

    // 应用初始主题类名到 HTML 根元素
    this.applyClass();
  }

  /**
   * 获取解析后的实际主题
   *
   * 根据当前的 theme 设置和系统偏好，计算出实际应该使用的主题。
   *
   * ==================== 解析逻辑 ====================
   * - theme 为 'system'：查询操作系统的 prefers-color-scheme 媒体查询
   *   - 系统偏好为暗色 → 返回 'dark'
   *   - 系统偏好为亮色（或无法检测）→ 返回 'light'
   * - theme 为 'light' 或 'dark'：直接返回 theme 值
   *
   * ==================== 技术细节 ====================
   * 使用 window.matchMedia('(prefers-color-scheme: dark)') 检测系统主题：
   * - matches 为 true：系统当前为暗色模式
   * - matches 为 false：系统当前为亮色模式
   *
   * @returns 解析后的实际主题，'light' 或 'dark'
   */
  private getResolved(): "light" | "dark" {
    if (this.theme === "system") {
      // 查询系统主题偏好，如果系统为暗色模式则返回 'dark'，否则返回 'light'
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    // 非系统模式下，直接返回用户选择的主题
    return this.theme;
  }

  /**
   * 将主题类名应用到 HTML 根元素
   *
   * 通过切换 <html> 元素上的 'dark' CSS 类名来控制 Tailwind CSS 的暗色模式。
   *
   * ==================== 工作原理 ====================
   * - resolved 为 'dark' 时：添加 'dark' 类名
   * - resolved 为 'light' 时：移除 'dark' 类名
   *
   * 这使得 Tailwind CSS 的 dark: 变体生效：
   * ```html
   * <!-- resolved 为 'dark' 时 -->
   * <html class="dark">
   *   <div class="bg-white dark:bg-gray-900">...</div>
   *   <!-- 渲染为深色背景 -->
   * </html>
   *
   * <!-- resolved 为 'light' 时 -->
   * <html>
   *   <div class="bg-white dark:bg-gray-900">...</div>
   *   <!-- 渲染为白色背景 -->
   * </html>
   * ```
   *
   * ==================== 注意事项 ====================
   * - 使用 classList.toggle 方法，第二个参数控制添加/移除
   * - 操作的是 document.documentElement（即 <html> 元素）
   * - 此方法是私有的，外部通过 setTheme() 间接调用
   */
  private applyClass() {
    // toggle('dark', true) 添加 dark 类名
    // toggle('dark', false) 移除 dark 类名
    document.documentElement.classList.toggle("dark", this.resolved === "dark");
  }

  /**
   * 切换主题模式
   *
   * 设置新的主题模式并立即应用。主题偏好会持久化到 localStorage，
   * 刷新页面后自动恢复。
   *
   * ==================== 参数说明 ====================
   * @param theme - 目标主题模式
   *               - 'light'：切换到亮色模式
   *               - 'dark'：切换到暗色模式
   *               - 'system'：切换到跟随系统模式
   *
   * ==================== 副作用 ====================
   * - 更新 theme 响应式状态
   * - 重新计算 resolved 派生状态
   * - 更新 HTML 根元素的 CSS 类名
   * - 将主题偏好保存到 localStorage
   * - 所有绑定到 theme/resolved 的 UI 元素自动更新
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const themeState = getThemeState();
   *
   * // 切换到暗色模式
   * themeState.setTheme('dark');
   * console.log(themeState.theme);    // 'dark'
   * console.log(themeState.resolved); // 'dark'
   *
   * // 切换到跟随系统模式
   * themeState.setTheme('system');
   * // resolved 取决于当前系统设置
   *
   * // 切换到亮色模式
   * themeState.setTheme('light');
   * console.log(themeState.resolved); // 'light'
   * ```
   */
  setTheme(theme: Theme) {
    // 更新用户选择的主题模式
    this.theme = theme;
    // 持久化到 localStorage，键名为 'postium-theme'
    localStorage.setItem("postium-theme", theme);
    // 重新计算解析后的实际主题
    this.resolved = this.getResolved();
    // 将主题类名应用到 HTML 根元素
    this.applyClass();
  }
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const THEME_KEY = Symbol("theme");

/**
 * 创建主题状态管理器
 *
 * 创建一个新的 ThemeState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getThemeState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createThemeState } from '$lib/stores/theme.svelte';
 *
 *   // 创建并设置 Context
 *   const themeState = createThemeState();
 * </script>
 *
 * <!-- 使用主题状态 -->
 * <div class="{themeState.resolved === 'dark' ? 'dark-theme' : 'light-theme'}">
 *   ...
 * </div>
 * ```
 *
 * @returns ThemeState 实例
 */
export function createThemeState() {
  const state = new ThemeState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(THEME_KEY, state);
  return state;
}

/**
 * 获取主题状态管理器
 *
 * 从 Svelte Context 中获取由 createThemeState() 创建的 ThemeState 实例。
 * 此函数应在子组件中调用，以访问全局的主题状态。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createThemeState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createThemeState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- ThemeToggle.svelte -->
 * <script lang="ts">
 *   import { getThemeState } from '$lib/stores/theme.svelte';
 *
 *   // 获取全局主题状态
 *   const themeState = getThemeState();
 *
 *   function toggleTheme() {
 *     // 在亮色和暗色之间切换
 *     themeState.setTheme(themeState.resolved === 'dark' ? 'light' : 'dark');
 *   }
 * </script>
 *
 * <button onclick={toggleTheme}>
 *   {themeState.resolved === 'dark' ? '☀️' : '🌙'}
 * </button>
 * ```
 *
 * @returns ThemeState 实例
 * @throws 如果在 createThemeState() 之前调用，会抛出 Context 错误
 */
export function getThemeState() {
  return getContext<ThemeState>(THEME_KEY);
}
