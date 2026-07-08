/**
 * Postium Mail - 账号添加流程测试
 * account-add-flow.test.ts
 *
 * 本文件测试账号添加流程的工具函数，包括添加后的状态更新、
 * 初始同步启动、OAuth2 完成后的账号解析等。
 *
 * ==================== 测试范围 ====================
 * 1. continueAfterAccountAdded - 账号添加后的继续操作
 * 2. startInitialSyncAfterAccountAdded - 启动初始同步
 * 3. resolveOAuth2CompletedAccount - OAuth2 完成后的账号解析
 */

import { describe, it, expect, vi } from "vitest";
import {
  continueAfterAccountAdded,
  resolveOAuth2CompletedAccount,
  startInitialSyncAfterAccountAdded,
} from "$lib/components/settings/account-add-flow";

describe("account add flow", () => {
  it("添加账号后只进入主界面，不立即同步", async () => {
    const calls: string[] = [];

    const result = await continueAfterAccountAdded({
      accountId: 7,
      loadAccounts: async () => {
        calls.push("loadAccounts");
      },
      setActive: (accountId) => {
        calls.push(`setActive:${accountId}`);
      },
      close: () => {
        calls.push("close");
      },
      goHome: async () => {
        calls.push("goHome");
      },
    });

    expect(calls).toEqual(["loadAccounts", "setActive:7", "close", "goHome"]);
    expect(result.readyForInitialSync).toBe(true);
  });

  it("用户选择范围后启动带范围首次同步", async () => {
    const syncAccountWithRange = vi.fn(async () => undefined);

    await startInitialSyncAfterAccountAdded({
      accountId: 7,
      range: "three_months",
      syncAccountWithRange,
    });

    expect(syncAccountWithRange).toHaveBeenCalledWith(7, "three_months");
  });

  it("OAuth2 完成后用后端返回邮箱定位账号", async () => {
    const calls: string[] = [];
    let loaded = false;

    const accountId = await resolveOAuth2CompletedAccount({
      completedEmail: "oauth-created@example.com",
      loadAccounts: async () => {
        calls.push("loadAccounts");
        loaded = true;
      },
      getAccounts: () => {
        calls.push(`getAccounts:${loaded}`);
        return [
          { id: 1, email: "typed-before-oauth@example.com" },
          { id: 7, email: "oauth-created@example.com" },
        ];
      },
    });

    expect(accountId).toBe(7);
    expect(calls).toEqual(["loadAccounts", "getAccounts:true"]);
  });

  it("OAuth2 完成后找不到后端返回邮箱时不伪造账号", async () => {
    const accountId = await resolveOAuth2CompletedAccount({
      completedEmail: "missing@example.com",
      loadAccounts: async () => undefined,
      getAccounts: () => [{ id: 1, email: "other@example.com" }],
    });

    expect(accountId).toBeNull();
  });
});
