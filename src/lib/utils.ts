/**
 * Postium Mail - 通用工具函数库
 * utils.ts
 *
 * 本文件提供应用中常用的通用工具函数。
 * 这些函数都是纯函数，无副作用，可以在任何地方安全使用。
 *
 * ==================== 当前包含的工具函数 ====================
 * - cn(): 合并和去重 Tailwind CSS 类名
 *
 * ==================== 设计原则 ====================
 * 1. 纯函数：相同的输入总是产生相同的输出
 * 2. 无副作用：不修改输入参数，不改变外部状态
 * 3. 类型安全：使用 TypeScript 提供完整的类型检查
 * 4. 可测试：函数简单明确，易于单元测试
 */

/**
 * 导入 clsx 库
 *
 * clsx 是一个用于有条件地构建 className 字符串的工具库。
 * 它可以智能地处理各种输入类型：
 * - 字符串
 * - 对象（根据布尔值决定是否包含类名）
 * - 数组
 * - null/undefined
 *
 * 官方文档：https://github.com/lukeed/clsx
 *
 * 使用示例：
 * ```typescript
 * clsx('foo', 'bar');  // 'foo bar'
 * clsx('foo', { bar: true, baz: false });  // 'foo bar'
 * clsx(['foo', 'bar']);  // 'foo bar'
 * clsx(null, false, 'foo', undefined, 0, 1, { bar: null });  // 'foo'
 * ```
 */
import { clsx, type ClassValue } from "clsx";

/**
 * 导入 tailwind-merge 库
 *
 * tailwind-merge 是一个专门用于合并 Tailwind CSS 类名的工具。
 * 它可以智能地解决 Tailwind 类名冲突问题：
 * - 后面的类名会覆盖前面的冲突类名
 * - 只处理 Tailwind CSS 相关的类名
 * - 保持其他类名不变
 *
 * 官方文档：https://github.com/dcastil/tailwind-merge
 *
 * 使用示例：
 * ```typescript
 * twMerge('px-2 py-1', 'py-3');  // 'px-2 py-3' (py-3 覆盖了 py-1)
 * twMerge('text-xl', 'text-2xl');  // 'text-2xl'
 * ```
 */
import { twMerge } from "tailwind-merge";

/**
 * 合并和去重 Tailwind CSS 类名
 *
 * 此函数是 clsx 和 tailwind-merge 的组合，提供最佳的类名合并体验：
 * 1. 使用 clsx 处理条件类名（对象、数组、条件语句）
 * 2. 使用 tailwind-merge 解决 Tailwind 类名冲突
 *
 * ==================== 功能说明 ====================
 * - 接受任意数量的类名输入
 * - 自动去重冲突的 Tailwind 类名（后面的覆盖前面的）
 * - 智能处理各种输入类型（字符串、对象、数组等）
 * - 保持非 Tailwind 类名不变
 *
 * ==================== 参数说明 ====================
 * @param inputs - 可变参数，接受 ClassValue 类型
 *                 ClassValue 可以是：
 *                 - 字符串: 'px-4 py-2'
 *                 - 对象: { 'text-red-500': isError }
 *                 - 数组: ['px-4', isError && 'text-red-500']
 *                 - null/undefined: 会被忽略
 *                 - 上述类型的任意组合
 *
 * ==================== 返回值说明 ====================
 * @returns 合并后的类名字符串
 *          - 所有输入的类名合并为一个字符串
 *          - Tailwind 冲突的类名被正确处理
 *          - 多余的空格被去除
 *
 * ==================== 使用场景 ====================
 * 1. 条件类名：根据状态动态添加类名
 * 2. 组件 props 接受的默认类名
 * 3. 继承和覆盖父组件的类名
 * 4. 组合多个工具类
 *
 * ==================== 代码示例 ====================
 *
 * 基本用法：
 * ```typescript
 * cn('px-4 py-2', 'bg-blue-500', 'text-white');
 * // 输出: 'px-4 py-2 bg-blue-500 text-white'
 * ```
 *
 * 条件类名：
 * ```typescript
 * const isActive = true;
 * const isError = false;
 * cn('px-4 py-2', {
 *   'bg-blue-500': isActive,
 *   'bg-red-500': isError,
 *   'text-white': isActive
 * });
 * // 输出: 'px-4 py-2 bg-blue-500 text-white'
 * ```
 *
 * 解决冲突（后面的覆盖前面的）：
 * ```typescript
 * cn('px-4 py-2', 'py-8');
 * // 输出: 'px-4 py-8'  (py-8 覆盖了 py-2)
 *
 * cn('text-lg font-medium', 'text-xl');
 * // 输出: 'text-xl font-medium'  (text-xl 覆盖了 text-lg)
 * ```
 *
 * 数组和对象组合：
 * ```typescript
 * cn(
 *   ['px-4', 'py-2'],
 *   {
 *     'bg-blue-500': isActive,
 *     'hover:bg-blue-600': true
 *   },
 *   'text-white'
 * );
 * // 输出: 'px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white'
 * ```
 *
 * 在组件中使用：
 * ```svelte
 * <script lang="ts">
 *   interface Props {
 *     class?: string;
 *     variant?: 'primary' | 'secondary' | 'danger';
 *   }
 *   let { class: className = '', variant = 'primary' }: Props = $props();
 *
 *   const baseStyles = 'px-4 py-2 rounded font-medium';
 *   const variants = {
 *     primary: 'bg-blue-500 hover:bg-blue-600 text-white',
 *     secondary: 'bg-gray-200 hover:bg-gray-300 text-gray-900',
 *     danger: 'bg-red-500 hover:bg-red-600 text-white'
 *   };
 *
 *   $: buttonClass = cn(baseStyles, variants[variant], className);
 * </script>
 *
 * <button class={buttonClass}>点击我</button>
 * ```
 *
 * ==================== 注意事项 ====================
 * 1. 性能：在 Svelte 中，建议使用 $derived 或在模板外计算，避免重复计算
 * 2. 类型：TypeScript 会检查输入类型，提供类型安全
 * 3. Tailwind：只处理 Tailwind 类名，自定义类名会被保留
 * 4. 顺序：后面的参数中的类名会覆盖前面的冲突类名
 *
 * ==================== 为什么需要这个函数 ====================
 * Tailwind CSS 的类名冲突问题：
 * - 如果简单使用字符串拼接，可能会导致类名冲突
 * - 例如: 'px-4 py-2' + 'py-8' = 'px-4 py-2 py-8'（py-2 仍然有效）
 * - 使用 tailwind-merge 可以智能处理，只保留 py-8
 *
 * 条件类名的复杂性：
 * - 手动拼接条件类名会很繁琐
 * - clsx 提供了优雅的条件类名语法
 * - 但 clsx 不处理 Tailwind 冲突
 *
 * 因此，cn() 函数结合了两者的优点：
 * - clsx 的灵活语法
 * - tailwind-merge 的智能合并
 * - 简洁的 API：cn(...inputs)
 */
export function cn(...inputs: ClassValue[]) {
  // 使用 clsx 处理各种输入类型，生成类名字符串
  // 然后使用 twMerge 解决 Tailwind 类名冲突
  return twMerge(clsx(inputs));
}
