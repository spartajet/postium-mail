/**
 * Postium Mail - 邮件同步状态管理模块
 * sync.svelte.ts
 *
 * 本模块提供邮件同步的状态管理功能，使用 Svelte 5 的 Runes API 实现响应式状态。
 * 负责管理与邮件服务器的数据同步，包括同步进度跟踪和文件夹统计信息。
 *
 * ==================== 功能概述 ====================
 * 1. 管理邮件账号的同步状态（同步中 / 完成 / 出错）
 * 2. 实时跟踪同步进度（连接、同步文件夹、同步邮件等阶段）
 * 3. 加载并缓存文件夹统计信息（各文件夹的邮件数量）
 * 4. 监听 Tauri 后端发出的同步进度事件
 * 5. 使用 Svelte Context API 实现跨组件状态共享
 *
 * ==================== 设计原则 ====================
 * 1. 事件驱动：通过 Tauri Event 系统接收后端同步进度更新
 * 2. 响应式：使用 $state Runes 实现自动 UI 更新
 * 3. 错误处理：统一处理同步过程中的错误，提供用户友好的提示
 * 4. 单例模式：通过 Svelte Context 确保全局只有一个 SyncState 实例
 * 5. 容错设计：Toast 提示可能未初始化，使用 try-catch 保证稳定性
 *
 * ==================== 同步流程 ====================
 * 1. 用户触发同步（点击同步按钮或自动同步）
 * 2. 调用 syncAccount() 向 Tauri 后端发起同步请求
 * 3. 后端开始异步执行同步任务
 * 4. 后端通过 Tauri Event 持续发送同步进度事件
 * 5. 前端监听事件并更新 UI 状态
 * 6. 同步完成或出错时，显示相应的 Toast 提示
 *
 * ==================== 使用示例 ====================
 * ```typescript
 * // 在 +layout.svelte 中初始化
 * import { createSyncState } from '$lib/stores/sync.svelte';
 * const syncState = createSyncState();
 *
 * // 在子组件中获取状态
 * import { getSyncState } from '$lib/stores/sync.svelte';
 * const syncState = getSyncState();
 *
 * // 触发同步
 * await syncState.syncAccount(accountId);
 *
 * // 读取同步进度
 * if (syncState.syncing) {
 *   console.log('当前阶段:', syncState.progress?.stage);
 * }
 * ```
 */

// 导入 Svelte Context API，用于跨组件状态共享
import { getContext, setContext } from "svelte";

// 导入 Tauri 命令绑定和事件系统，用于调用后端 API 和监听事件
import { commands, events } from "$lib/bindings";

// 导入同步进度和文件夹统计的数据传输对象类型
import type {
  EmailCategory,
  FolderStat,
  HistorySyncState,
  InitialSyncRange,
  OlderSyncResult,
  SyncAllAccountsResult,
  SyncProgress,
} from "$lib/bindings";

// 导入错误格式化工具
import { formatError } from "$lib/utils/error.js";

// 导入 Toast 状态管理，用于在同步完成/失败时显示提示消息
import { getToastState } from "$lib/stores/toast.svelte";

/**
 * 同步状态管理类
 *
 * 管理邮件账号与服务器之间的数据同步状态，包括同步进度、文件夹统计和错误信息。
 * 使用 Svelte 5 的 Runes API ($state) 实现响应式状态管理。
 *
 * ==================== 状态字段 ====================
 * - `syncing`: 是否正在同步中（响应式）
 * - `progress`: 当前的同步进度详情（响应式）
 * - `folderStats`: 各文件夹的邮件统计数据（响应式）
 * - `error`: 错误信息（响应式）
 *
 * ==================== 事件监听 ====================
 * 构造函数中会监听后端发出的 syncProgressEvent 事件，实时更新同步进度。
 * 事件包含以下阶段：
 * - Connecting：正在连接服务器
 * - SyncingFolders：正在同步文件夹列表
 * - SyncingEmails：正在同步邮件内容
 * - Completed：同步完成
 * - Error：同步出错
 *
 * ==================== 生命周期 ====================
 * 1. 应用启动时，由 +layout.svelte 创建 SyncState 实例
 * 2. 构造函数中注册同步进度事件监听器
 * 3. 通过 setContext 将实例存入 Svelte Context
 * 4. 子组件通过 getContext 获取同一个实例
 * 5. 所有状态变更自动触发 UI 更新
 *
 * ==================== 注意事项 ====================
 * - 此类不应直接实例化，应通过 createSyncState() 创建
 * - Toast 提示可能在某些情况下未初始化（如 Context 未设置），
 *   因此使用 try-catch 包裹 Toast 调用
 */
export class SyncState {
  /**
   * 是否正在同步中（响应式状态）
   *
   * 当 syncAccount() 被调用时设为 true，
   * 同步完成（成功或失败）后设为 false。
   * UI 可以根据此状态显示加载动画或禁用同步按钮。
   */
  syncing = $state(false);

  /**
   * 当前同步进度详情（响应式状态）
   *
   * 包含同步的详细阶段信息，如当前阶段、已同步数量、总数量等。
   * null 表示没有正在进行的同步任务。
   *
   * 此状态由后端的 syncProgressEvent 事件驱动更新。
   */
  progress = $state<SyncProgress | null>(null);

  /**
   * 各文件夹的邮件统计数据（响应式状态）
   *
   * 包含每个文件夹中的邮件总数和未读数量。
   * 通过 loadFolderStats() 方法加载和更新。
   *
   * 用于在侧边栏中显示各文件夹的未读邮件数量角标。
   */
  folderStats = $state<FolderStat[]>([]);

  historyStates = $state<Record<string, HistorySyncState>>({});

  olderSyncingKeys = $state<Set<string>>(new Set());

  /**
   * 错误信息（响应式状态）
   *
   * 同步过程中发生错误时的错误消息。
   * null 表示没有错误。
   *
   * 错误来源包括：
   * - 后端返回的错误（如网络连接失败、认证失败）
   * - 前端捕获的异常
   */
  error = $state<string | null>(null);

  /**
   * 构造函数
   *
   * 初始化同步状态并注册 Tauri 事件监听器。
   * 监听后端发出的 syncProgressEvent 事件，实时更新同步进度。
   *
   * ==================== 事件处理逻辑 ====================
   * 1. 收到进度事件后，更新 progress 状态
   * 2. 如果阶段为 'Completed'：
   *    - 将 syncing 设为 false
   *    - 显示同步成功的 Toast 提示
   * 3. 如果阶段为 'Error'：
   *    - 将 syncing 设为 false
   *    - 设置 error 状态
   *    - 显示同步失败的 Toast 提示
   *
   * ==================== 容错设计 ====================
   * Toast 提示使用 try-catch 包裹，因为在某些情况下
   * Toast 的 Context 可能尚未初始化（如应用启动阶段）。
   */
  constructor() {
    // 监听后端发送的同步进度事件
    events.syncProgressEvent.listen((event) => {
      // 更新当前的同步进度状态
      this.progress = event.payload.progress;

      // 同步完成的处理
      if (event.payload.progress.stage === "Completed") {
        this.syncing = false;
        try {
          // 尝试显示成功提示（Toast 可能未初始化）
          const toast = getToastState();
          toast.success("同步完成");
        } catch {
          /* toast 可能未初始化 */
        }
      } else if (event.payload.progress.stage === "Error") {
        // 同步出错的处理
        this.syncing = false;
        this.error = event.payload.progress.message;
        try {
          // 尝试显示错误提示（Toast 可能未初始化）
          const toast = getToastState();
          toast.error(
            "同步失败: " + (event.payload.progress.message ?? "未知错误"),
          );
        } catch {
          /* toast 可能未初始化 */
        }
      }
    });
  }

  private historyKey(accountId: number, category: EmailCategory) {
    return `${accountId}:${category}`;
  }

  /**
   * 触发账号同步
   *
   * 向 Tauri 后端发送同步请求，开始同步指定账号的邮件数据。
   * 同步过程是异步的，后端会通过事件持续发送进度更新。
   *
   * ==================== 工作流程 ====================
   * 1. 设置 syncing 为 true，表示开始同步
   * 2. 清除之前的错误信息
   * 3. 调用后端 syncAccount 命令发起同步
   * 4. 成功时等待事件监听器处理后续进度
   * 5. 失败时设置错误信息
   * 6. 最终将 syncing 设置为 false
   *
   * ==================== 注意事项 ====================
   * - 同步是异步操作，此函数返回后同步可能仍在进行
   * - 实际的进度更新通过 syncProgressEvent 事件接收
   * - 如果后端返回错误，会设置 error 状态
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const syncState = getSyncState();
   *
   * // 同步指定账号
   * await syncState.syncAccount(1);
   *
   * // 监听同步状态
   * $: if (syncState.syncing) {
   *   console.log('同步中...', syncState.progress?.stage);
   * }
   * ```
   *
   * @param accountId - 要同步的邮件账号 ID
   * @returns Promise<void>
   */
  async syncAccount(accountId: number) {
    // 设置同步中状态
    this.syncing = true;
    // 清除之前的错误信息
    this.error = null;

    try {
      // 调用后端命令发起账号同步
      const result = await commands.syncAccount(accountId);

      if (result.status === "error") {
        // 后端返回错误时，设置错误信息
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      // 捕获异常并格式化错误消息
      this.error = formatError(e);
    } finally {
      // 无论成功或失败，都重置同步状态
      // 注意：如果同步仍在进行（异步），事件监听器会在完成时再次设置 syncing
      this.syncing = false;
    }
  }

  async syncAllAccounts(): Promise<SyncAllAccountsResult | null> {
    this.syncing = true;
    this.error = null;

    try {
      const result = await commands.syncAllAccounts();

      if (result.status === "error") {
        this.error = result.error.message as string;
        return null;
      }

      return result.data;
    } catch (e: unknown) {
      this.error = formatError(e);
      return null;
    } finally {
      this.syncing = false;
    }
  }

  async syncAccountWithRange(accountId: number, range: InitialSyncRange) {
    // 设置同步中状态
    this.syncing = true;
    // 清除之前的错误信息
    this.error = null;

    try {
      // 调用后端命令发起指定范围的账号同步
      const result = await commands.syncAccountWithRange(accountId, range);

      if (result.status === "error") {
        // 后端返回错误时，设置错误信息
        this.error = result.error.message as string;
      }
    } catch (e: unknown) {
      // 捕获异常并格式化错误消息
      this.error = formatError(e);
    } finally {
      // 无论成功或失败，都重置同步状态
      this.syncing = false;
    }
  }

  async loadHistoryState(accountId: number, category: EmailCategory) {
    const result = await commands.getSyncHistoryState(accountId, category);

    if (result.status === "error") {
      this.error = result.error.message as string;
      return null;
    }

    this.historyStates = {
      ...this.historyStates,
      [this.historyKey(accountId, category)]: result.data,
    };
    return result.data;
  }

  async syncOlderEmails(
    accountId: number,
    category: EmailCategory,
  ): Promise<OlderSyncResult | null> {
    const key = this.historyKey(accountId, category);
    this.olderSyncingKeys = new Set([...this.olderSyncingKeys, key]);
    this.error = null;

    try {
      const result = await commands.syncOlderEmails(accountId, category);

      if (result.status === "error") {
        this.error = result.error.message as string;
        return null;
      }

      await this.loadHistoryState(accountId, category);
      return result.data;
    } catch (e: unknown) {
      this.error = formatError(e);
      return null;
    } finally {
      const next = new Set(this.olderSyncingKeys);
      next.delete(key);
      this.olderSyncingKeys = next;
    }
  }

  isOlderSyncing(accountId: number, category: EmailCategory) {
    return this.olderSyncingKeys.has(this.historyKey(accountId, category));
  }

  getHistoryState(accountId: number, category: EmailCategory) {
    return this.historyStates[this.historyKey(accountId, category)] ?? null;
  }

  /**
   * 加载文件夹统计信息
   *
   * 获取指定账号各文件夹的邮件统计数据，包括邮件总数和未读数量。
   * 通常在切换账号或同步完成后调用，用于更新侧边栏的邮件计数显示。
   *
   * ==================== 工作流程 ====================
   * 1. 调用后端 getFolderStats 命令获取统计数据
   * 2. 成功时更新 folderStats 状态
   * 3. 失败时在控制台输出错误（不设置 error 状态，因为是非关键操作）
   *
   * ==================== 使用场景 ====================
   * - 账号切换后重新加载文件夹统计
   * - 同步完成后刷新未读邮件计数
   * - 应用启动时加载初始数据
   *
   * ==================== 使用示例 ====================
   * ```typescript
   * const syncState = getSyncState();
   *
   * // 加载文件夹统计
   * await syncState.loadFolderStats(accountId);
   *
   * // 读取统计信息
   * for (const stat of syncState.folderStats) {
   *   console.log(`${stat.folder}: ${stat.unread} 封未读`);
   * }
   * ```
   *
   * @param accountId - 要加载统计信息的账号 ID
   * @returns Promise<void>
   */
  async loadFolderStats(accountId: number) {
    try {
      // 调用后端命令获取文件夹统计数据
      const result = await commands.getFolderStats(accountId);

      if (result.status === "ok") {
        // 成功时更新文件夹统计状态
        this.folderStats = result.data;
      }
    } catch (e: unknown) {
      // 文件夹统计加载失败不影响核心功能，仅在控制台输出错误
      console.error("Failed to load folder stats:", e);
    }
  }

  async loadFolderStatsForAllAccounts() {
    try {
      const result = await commands.getFolderStatsForAllAccounts();

      if (result.status === "ok") {
        this.folderStats = result.data;
      }
    } catch (e: unknown) {
      console.error("Failed to load folder stats for all accounts:", e);
    }
  }
}

/**
 * Svelte Context 的键名
 *
 * 使用 Symbol 作为键名，确保在 Svelte Context 中的唯一性，
 * 避免与其他模块的 Context 键名冲突。
 */
const SYNC_KEY = Symbol("sync");

/**
 * 创建同步状态管理器
 *
 * 创建一个新的 SyncState 实例并将其存入 Svelte Context。
 * 此函数应在应用的根布局组件（+layout.svelte）中调用，
 * 确保所有子组件都能访问同一个状态实例。
 *
 * ==================== 使用说明 ====================
 * - 只在应用初始化时调用一次
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例可以通过 getSyncState() 在子组件中获取
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- +layout.svelte -->
 * <script lang="ts">
 *   import { createSyncState } from '$lib/stores/sync.svelte';
 *
 *   // 创建并设置 Context
 *   const syncState = createSyncState();
 * </script>
 * ```
 *
 * @returns SyncState 实例
 */
export function createSyncState() {
  const state = new SyncState();
  // 将状态实例存入 Svelte Context，供子组件获取
  setContext(SYNC_KEY, state);
  return state;
}

/**
 * 获取同步状态管理器
 *
 * 从 Svelte Context 中获取由 createSyncState() 创建的 SyncState 实例。
 * 此函数应在子组件中调用，以访问全局的同步状态。
 *
 * ==================== 使用说明 ====================
 * - 调用此函数前，必须确保父组件已调用 createSyncState()
 * - 必须在组件初始化阶段调用（不能在 onMount 中）
 * - 返回的实例与 createSyncState() 创建的是同一个对象
 *
 * ==================== 使用示例 ====================
 * ```svelte
 * <!-- SyncButton.svelte -->
 * <script lang="ts">
 *   import { getSyncState } from '$lib/stores/sync.svelte';
 *   import { getAccountState } from '$lib/stores/account.svelte';
 *
 *   const syncState = getSyncState();
 *   const accountState = getAccountState();
 *
 *   async function handleSync() {
 *     if (accountState.activeAccountId) {
 *       await syncState.syncAccount(accountState.activeAccountId);
 *     }
 *   }
 * </script>
 *
 * <button onclick={handleSync} disabled={syncState.syncing}>
 *   {syncState.syncing ? '同步中...' : '同步'}
 * </button>
 * ```
 *
 * @returns SyncState 实例
 * @throws 如果在 createSyncState() 之前调用，会抛出 Context 错误
 */
export function getSyncState() {
  return getContext<SyncState>(SYNC_KEY);
}
