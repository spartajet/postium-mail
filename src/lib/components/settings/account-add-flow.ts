/**
 * Postium Mail - 账号添加流程工具函数
 * account-add-flow.ts
 *
 * 本文件提供账号添加流程中的工具函数，用于处理账号添加后的状态更新和初始同步。
 *
 * ==================== 功能概述 ====================
 * 1. continueAfterAccountAdded: 账号添加完成后的状态更新流程
 * 2. startInitialSyncAfterAccountAdded: 启动账号的初始同步
 * 3. resolveOAuth2CompletedAccount: 解析 OAuth2 完成后的账号信息
 *
 * ==================== 使用场景 ====================
 * - 用户通过密码添加账号后的处理流程
 * - 用户通过 OAuth2 添加账号后的处理流程
 * - 账号添加完成后启动初始同步
 * - 账号添加完成后切换到新账号
 */

import type { InitialSyncRange } from "$lib/bindings";

/**
 * 账号添加完成后的继续操作选项
 *
 * 定义继续操作所需的回调函数和参数。
 */
type ContinueAfterAccountAddedOptions = {
  /** 新添加的账号 ID */
  accountId: number;
  /** 重新加载账号列表的回调函数 */
  loadAccounts: () => Promise<void>;
  /** 设置当前活跃账号的回调函数 */
  setActive: (accountId: number) => void;
  /** 关闭设置窗口的回调函数 */
  close: () => void;
  /** 返回主页面的回调函数 */
  goHome: () => Promise<void>;
};

/**
 * 账号添加完成后的继续操作
 *
 * 此函数处理账号添加完成后的后续操作，包括：
 * 1. 重新加载账号列表
 * 2. 切换到新添加的账号
 * 3. 关闭设置窗口
 * 4. 返回主页面
 *
 * ==================== 参数说明 ====================
 * @param options - 继续操作所需的选项
 *
 * ==================== 返回值说明 ====================
 * @returns 包含 readyForInitialSync 标志的对象，表示准备好进行初始同步
 */
export async function continueAfterAccountAdded({
  accountId,
  loadAccounts,
  setActive,
  close,
  goHome,
}: ContinueAfterAccountAddedOptions) {
  // 重新加载账号列表，以包含新添加的账号
  await loadAccounts();
  // 切换到新添加的账号
  setActive(accountId);
  // 关闭设置窗口
  close();
  // 返回主页面
  await goHome();

  return { readyForInitialSync: true };
}

/**
 * 启动初始同步的选项
 *
 * 定义启动初始同步所需的参数和回调函数。
 */
type StartInitialSyncOptions = {
  /** 要同步的账号 ID */
  accountId: number;
  /** 初始同步范围 */
  range: InitialSyncRange;
  /** 启动同步的回调函数 */
  syncAccountWithRange: (
    accountId: number,
    range: InitialSyncRange,
  ) => Promise<void>;
};

/**
 * 启动账号的初始同步
 *
 * 此函数在账号添加完成后启动初始同步。
 *
 * ==================== 参数说明 ====================
 * @param options - 启动初始同步所需的选项
 *
 * ==================== 返回值说明 ====================
 * @returns 包含 syncStarted 标志的对象，表示同步已启动
 */
export async function startInitialSyncAfterAccountAdded({
  accountId,
  range,
  syncAccountWithRange,
}: StartInitialSyncOptions) {
  // 调用同步函数，启动指定范围的同步
  await syncAccountWithRange(accountId, range);

  return { syncStarted: true };
}

/**
 * 账号邮箱标识
 *
 * 定义账号的基本标识信息。
 */
type AccountEmailIdentity = {
  /** 账号 ID */
  id: number;
  /** 账号邮箱地址 */
  email: string;
};

/**
 * 解析 OAuth2 完成后的账号选项
 *
 * 定义解析 OAuth2 完成账号所需的参数和回调函数。
 */
type ResolveOAuth2CompletedAccountOptions = {
  /** OAuth2 完成后的邮箱地址 */
  completedEmail: string;
  /** 重新加载账号列表的回调函数 */
  loadAccounts: () => Promise<void>;
  /** 获取账号列表的回调函数 */
  getAccounts: () => AccountEmailIdentity[];
};

/**
 * 解析 OAuth2 完成后的账号信息
 *
 * 此函数在 OAuth2 授权完成后，查找对应的账号 ID。
 *
 * ==================== 参数说明 ====================
 * @param options - 解析账号所需的选项
 *
 * ==================== 返回值说明 ====================
 * @returns 找到的账号 ID，如果未找到则返回 null
 */
export async function resolveOAuth2CompletedAccount({
  completedEmail,
  loadAccounts,
  getAccounts,
}: ResolveOAuth2CompletedAccountOptions) {
  // 重新加载账号列表，确保包含新添加的账号
  await loadAccounts();

  // 查找与 OAuth2 完成邮箱匹配的账号
  return (
    getAccounts().find((account) => account.email === completedEmail)?.id ??
    null
  );
}
