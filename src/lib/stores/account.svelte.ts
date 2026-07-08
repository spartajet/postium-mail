/**
 * Postium Mail - 账号状态管理模块
 * account.svelte.ts
 *
 * 本模块提供邮件账号的状态管理功能，使用 Svelte 5 的 Runes API 实现响应式状态。
 *
 * ==================== 功能概述 ====================
 * 1. 管理用户的邮件账号列表
 * 2. 跟踪当前活跃的账号
 * 3. 提供账号的 CRUD 操作（创建、读取、更新、删除）
 * 4. 使用 Svelte Context API 实现跨组件状态共享
 *
 * ==================== 设计原则 ====================
 * 1. 单例模式：通过 Svelte Context 确保全局只有一个 AccountState 实例
 * 2. 响应式：使用 $state 和 $derived Runes 实现自动更新
 * 3. 错误处理：统一处理 API 调用错误，提供用户友好的错误消息
 * 4. 类型安全：完整的 TypeScript 类型定义
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createAccountState } from '$lib/stores/account.svelte';
 * const accountState = createAccountState();
 *
 * // 在子组件中获取状态
 * import { getAccountState } from '$lib/stores/account.svelte';
 * const accountState = getAccountState();
 *
 * // 加载账号列表
 * await accountState.loadAccounts();
 *
 * // 切换活跃账号
 * accountState.setActive(1);
 *
 * // 删除账号
 * await accountState.deleteAccount(1);
 * ```
 */

// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

// 导入 Tauri 命令绑定，用于调用后端 API
import { commands } from "$lib/bindings";

// 导入账号数据传输对象类型
import type { AccountDto } from "$lib/bindings";

// 导入错误格式化工具
import { formatError } from "$lib/utils/error.js";

const ACCOUNT_SCOPE_STORAGE_KEY = "postium-account-scope";

export type AccountScope =
  | { kind: "all" }
  | { kind: "account"; accountId: number };

function isBrowser() {
  return typeof window !== "undefined" && typeof localStorage !== "undefined";
}

function safeGetLocalStorageItem(key: string): string | null {
  if (!isBrowser()) {
    return null;
  }

  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function safeSetLocalStorageItem(key: string, value: string) {
  if (!isBrowser()) {
    return;
  }

  try {
    localStorage.setItem(key, value);
  } catch {
    // Ignore persistence failures in restricted or quota-limited environments.
  }
}

function readPersistedAccountScope(): AccountScope {
  const raw = safeGetLocalStorageItem(ACCOUNT_SCOPE_STORAGE_KEY);
  if (!raw) {
    return { kind: "all" };
  }

  try {
    const parsed = JSON.parse(raw) as
      | { kind?: unknown; accountId?: unknown }
      | null;
    if (parsed?.kind === "all") {
      return { kind: "all" };
    }
    if (
      parsed?.kind === "account" &&
      typeof parsed.accountId === "number" &&
      Number.isInteger(parsed.accountId)
    ) {
      return { kind: "account", accountId: parsed.accountId };
    }
  } catch {
    // Ignore malformed persisted data and fall back to the default scope.
  }

  return { kind: "all" };
}

/**
 * 账号状态管理类
 *
 * 管理用户所有邮件账号的状态，包括账号列表、当前活跃账号、加载状态和错误信息。
 * 使用 Svelte 5 的 Runes API ($state, $derived) 实现响应式状态管理。
 *
 * ==================== 状态字段 ====================
 * - `accounts`: 所有账号的列表（响应式）
 * - `activeAccountId`: 当前选中的账号 ID（响应式）
 * - `loading`: 是否正在加载（响应式）
 * - `error`: 错误信息（响应式）
 * - `activeAccount`: 当前活跃的账号对象（派生状态，自动计算）
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 AccountState 实例
 * 2. 通过 setContext 将实例存入 Svelte Context
 * 3. 子组件通过 getContext 获取同一个实例
 * 4. 所有状态变更自动触发 UI 更新
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createAccountState() 创建
 * - 状态字段使用 $state 标记，支持响应式更新
 * - activeAccount 是派生状态，根据 activeAccountId 自动计算
 */
export class AccountState {
  /** 所有账号列表（响应式状态） */
  accounts = $state<AccountDto[]>([]);

  /** 当前账号范围（响应式状态） */
  accountScope = $state<AccountScope>(readPersistedAccountScope());

  /** 最近一次选择的具体账号 ID（响应式状态） */
  lastConcreteAccountId = $state<number | null>(
    this.accountScope.kind === "account" ? this.accountScope.accountId : null,
  );

  /** 是否正在加载账号数据（响应式状态） */
  loading = $state(false);

  /** 错误信息（响应式状态），null 表示没有错误 */
  error = $state<string | null>(null);

  /**
   * 当前活跃的账号对象（派生状态）
   *
   * 根据 activeAccountId 自动从 accounts 列表中查找对应的账号。
   * 如果没有找到匹配的账号，返回 null。
   *
   * @returns 当前活跃的 AccountDto 对象，或 null
   */
  activeAccount = $derived(
    this.accounts.find((a) => a.id === this.selectedAccountId) ?? null,
  );

  /** 是否当前处于所有账号范围 */
  isAllAccounts = $derived(this.accountScope.kind === "all");

  /** 当前选中的具体账号 ID；所有账号范围下返回 null */
  selectedAccountId = $derived(
    this.accountScope.kind === "account" ? this.accountScope.accountId : null,
  );

  get activeAccountId() {
    return this.selectedAccountId;
  }

  set activeAccountId(id: number | null) {
    if (id == null) {
      this.setAllAccounts();
      return;
    }

    this.setActive(id);
  }

  persistAccountScope(scope: AccountScope) {
    safeSetLocalStorageItem(
      ACCOUNT_SCOPE_STORAGE_KEY,
      JSON.stringify(scope),
    );
  }

  applyAccountScope(scope: AccountScope) {
    this.accountScope = scope;
    if (scope.kind === "account") {
      this.lastConcreteAccountId = scope.accountId;
    }
    this.persistAccountScope(scope);
  }

  normalizeAccountScope() {
    if (this.accountScope.kind === "all") {
      this.persistAccountScope(this.accountScope);
      return;
    }

    const accountId = this.accountScope.accountId;
    const exists = this.accounts.some((account) => account.id === accountId);
    if (exists) {
      this.lastConcreteAccountId = accountId;
      this.persistAccountScope(this.accountScope);
      return;
    }

    this.setAllAccounts();
  }

  /**
   * 加载所有账号列表
   *
   * 从后端获取用户的所有邮件账号，并更新状态。
   * 如果当前持久化的是不存在的具体账号，会回退到所有账号范围。
   *
   * ==================== 工作流程 ====================
   * 1. 设置 loading 状态为 true
   * 2. 清除之前的错误信息
   * 3. 调用后端 listAccounts 命令获取账号列表
   * 4. 成功时更新 accounts 列表
   * 5. 规范化当前账号范围，必要时回退到所有账号
   * 6. 失败时设置错误信息
   * 7. 最终将 loading 设置为 false
   *
   * ==================== 错误处理 ====================
   * - 后端返回错误：显示错误消息
   * - 网络异常：格式化错误并显示
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const accountState = getAccountState();
   * await accountState.loadAccounts();
   * console.log(`加载了 ${accountState.accounts.length} 个账号`);
   * ```
   *
   * @returns Promise<void>
   */
  async loadAccounts() {
    // 设置加载状态
    this.loading = true;
    // 清除之前的错误
    this.error = null;

    try {
      // 调用后端命令获取账号列表
      const result = await commands.listAccounts();

      if (result.status === "ok") {
        // 成功时更新账号列表
        this.accounts = result.data;
        this.normalizeAccountScope();
      } else {
        // 后端返回错误时，设置错误信息
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      // 捕获异常并格式化错误消息
      this.error = formatError(e);
    } finally {
      // 无论成功或失败，都重置加载状态
      this.loading = false;
    }
  }

  /**
   * 设置当前活跃的账号
   *
   * 切换用户当前操作的邮件账号。切换后，相关的邮件列表、
   * 文件夹等数据会根据新账号重新加载。
   *
   * ==================== 参数说明 ====================
   * @param id - 要设为活跃的账号 ID
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * // 切换到 ID 为 1 的账号
   * accountState.setActive(1);
   * ```
   *
   * ==================== 副作用 ====================
   * - activeAccountId 状态更新
   * - activeAccount 派生状态自动更新
   * - UI 中显示的账号相关信息会刷新
   */
  setActive(id: number) {
    this.applyAccountScope({ kind: "account", accountId: id });
  }

  setAllAccounts() {
    this.applyAccountScope({ kind: "all" });
  }

  /**
   * 删除指定账号
   *
   * 永久删除指定的邮件账号及其所有相关数据。
   * 如果删除的是当前具体账号，会自动回退到所有账号范围。
   *
   * ⚠️ **警告：此操作不可逆！** 删除账号会同时删除：
   * - 账号配置信息
   * - 所有同步的邮件数据
   * - 邮件文件夹结构
   * - 标签和分类
   * - 同步历史记录
   *
   * ==================== 工作流程 ====================
   * 1. 调用后端 deleteAccount 命令删除账号
   * 2. 从本地账号列表中移除该账号
   * 3. 如果删除的是当前具体账号，回退到所有账号范围
   * 4. 同步更新最近一次具体账号记录
   *
   * ==================== 错误处理 ====================
   * - 删除失败时设置错误信息，但不回滚本地状态
   * - 建议在调用前显示确认对话框
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * // 删除前确认
   * if (confirm('确定要删除此账号吗？此操作不可撤销！')) {
   *   await accountState.deleteAccount(accountId);
   * }
   * ```
   *
   * @param id - 要删除的账号 ID
   * @returns Promise<void>
   */
  async deleteAccount(id: number) {
    this.error = null;

    try {
      // 调用后端命令删除账号
      const result = await commands.deleteAccount(id);

      if (result.status === "error") {
        this.error = String(result.error.message);
        return;
      }

      // 从本地列表中移除已删除的账号
      this.accounts = this.accounts.filter((a) => a.id !== id);
      if (this.lastConcreteAccountId === id) {
        this.lastConcreteAccountId = this.accounts[0]?.id ?? null;
      }

      // 如果删除的是当前具体账号，需要回退到所有账号范围
      if (this.selectedAccountId === id) {
        this.setAllAccounts();
      }
    } catch (e: unknown) {
      // 捕获异常并格式化错误消息
      this.error = formatError(e);
    }
  }
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const ACCOUNT_KEY = Symbol("account");

/**
 * 创建账号状态管理器
 *
 * 创建一个新的 AccountState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getAccountState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createAccountState } from '$lib/stores/account.svelte';
 *
 *   // 创建并设置 Context
 *   const accountState = createAccountState();
 *
 *   // 加载初始数据
 *   onMount(async () => {
 *     await accountState.loadAccounts();
 *   });
 * </script>
 * ```
 *
 * @returns AccountState 实例
 */
export function createAccountState() {
  const state = new AccountState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(ACCOUNT_KEY, state);
  return state;
}

/**
 * 获取账号状态管理器
 *
 * 从 Svelte Context 中获取由 createAccountState() 创建的 AccountState 实例。
 * 此函数应在子组件中调用，以访问全局的账号状态。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createAccountState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createAccountState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- AccountSelector.svelte -->
 * <script lang="ts">
 *   import { getAccountState } from '$lib/stores/account.svelte';
 *
 *   // 获取全局账号状态
 *   const accountState = getAccountState();
 * </script>
 *
 * <select bind:value={accountState.activeAccountId}>
 *   {#each accountState.accounts as account}
 *     <option value={account.id}>{account.email}</option>
 *   {/each}
 * </select>
 * ```
 *
 * @returns AccountState 实例
 * @throws 如果在 createAccountState() 之前调用，会抛出 Context 错误
 */
export function getAccountState() {
  return getContext<AccountState>(ACCOUNT_KEY);
}
