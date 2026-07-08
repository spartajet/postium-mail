/**
 * Postium Mail - 账号状态测试
 * account-state.test.ts
 *
 * 本文件测试账号状态管理器的功能，包括账号列表管理、
 * 当前活跃账号切换、持久化存储等。
 *
 * ==================== 测试范围 ====================
 * 1. 账号列表加载和管理
 * 2. 活跃账号切换和持久化
 * 3. 所有账号视图和单账号视图切换
 * 4. localStorage 持久化和容错处理
 * 5. 账号删除后的状态回退
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccountState } from "$lib/stores/account.svelte";
import { mockInvoke } from "../mocks/tauri";

const ACCOUNT_SCOPE_STORAGE_KEY = "postium-account-scope";

const accounts = [
  {
    id: 1,
    name: "工作邮箱",
    email: "work@example.com",
    display_name: "Work",
    provider: "gmail",
    color: "#2196f3",
    sync_enabled: true,
    auth_type: "Password",
    account_type: "work",
    last_sync_at: null,
    created_at: 1_745_126_400,
  },
  {
    id: 2,
    name: "个人邮箱",
    email: "personal@example.com",
    display_name: "Personal",
    provider: "outlook",
    color: "#ff9800",
    sync_enabled: true,
    auth_type: "Password",
    account_type: "personal",
    last_sync_at: null,
    created_at: 1_745_126_401,
  },
];

describe("AccountState 状态行为", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  it("首次加载默认所有账号", async () => {
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();

    await state.loadAccounts();

    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
    expect(state.accounts).toHaveLength(2);
    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.selectedAccountId).toBeNull();
    expect(state.activeAccount).toBeNull();
    expect(mockInvoke).toHaveBeenCalledWith("list_accounts");
  });

  it("setActive 选择具体账号并持久化", async () => {
    const state = new AccountState();
    state.accounts = [...accounts];

    state.setActive(2);

    expect(state.accountScope).toEqual({ kind: "account", accountId: 2 });
    expect(state.isAllAccounts).toBe(false);
    expect(state.activeAccountId).toBe(2);
    expect(state.selectedAccountId).toBe(2);
    expect(state.lastConcreteAccountId).toBe(2);
    expect(state.activeAccount?.email).toBe("personal@example.com");
    expect(localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY)).toBe(
      JSON.stringify({ kind: "account", accountId: 2 }),
    );
  });

  it("setAllAccounts 持久化所有账号范围", () => {
    const state = new AccountState();
    state.accounts = [...accounts];

    state.setActive(2);
    state.setAllAccounts();

    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.selectedAccountId).toBeNull();
    expect(state.lastConcreteAccountId).toBe(2);
    expect(localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY)).toBe(
      JSON.stringify({ kind: "all" }),
    );
  });

  it("activeAccountId setter 设为具体账号时走兼容层并持久化", () => {
    const state = new AccountState();
    state.accounts = [...accounts];

    state.activeAccountId = 2;

    expect(state.accountScope).toEqual({ kind: "account", accountId: 2 });
    expect(state.isAllAccounts).toBe(false);
    expect(state.activeAccountId).toBe(2);
    expect(state.selectedAccountId).toBe(2);
    expect(state.lastConcreteAccountId).toBe(2);
    expect(localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY)).toBe(
      JSON.stringify({ kind: "account", accountId: 2 }),
    );
  });

  it("activeAccountId setter 设为 null 时走兼容层并持久化所有账号范围", () => {
    const state = new AccountState();
    state.accounts = [...accounts];
    state.setActive(2);

    state.activeAccountId = null;

    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.selectedAccountId).toBeNull();
    expect(state.lastConcreteAccountId).toBe(2);
    expect(localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY)).toBe(
      JSON.stringify({ kind: "all" }),
    );
  });

  it("恢复 localStorage 的具体账号", async () => {
    localStorage.setItem(
      ACCOUNT_SCOPE_STORAGE_KEY,
      JSON.stringify({ kind: "account", accountId: 2 }),
    );
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();

    await state.loadAccounts();

    expect(state.activeAccountId).toBe(2);
    expect(state.activeAccount?.email).toBe("personal@example.com");
    expect(state.accountScope).toEqual({ kind: "account", accountId: 2 });
    expect(state.selectedAccountId).toBe(2);
  });

  it("恢复不存在账号时回退所有账号", async () => {
    localStorage.setItem(
      ACCOUNT_SCOPE_STORAGE_KEY,
      JSON.stringify({ kind: "account", accountId: 999 }),
    );
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();

    await state.loadAccounts();

    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.activeAccount).toBeNull();
    expect(localStorage.getItem(ACCOUNT_SCOPE_STORAGE_KEY)).toBe(
      JSON.stringify({ kind: "all" }),
    );
  });

  it("deleteAccount 删除当前具体账号时回退所有账号", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new AccountState();
    state.accounts = [...accounts];
    state.setActive(1);

    await state.deleteAccount(1);

    expect(state.accounts.map((account) => account.id)).toEqual([2]);
    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.selectedAccountId).toBeNull();
    expect(state.lastConcreteAccountId).toBe(2);
    expect(mockInvoke).toHaveBeenCalledWith("delete_account", { id: 1 });
  });

  it("localStorage.getItem 抛错时构造与加载回退为所有账号", async () => {
    const getItemSpy = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new DOMException("blocked", "SecurityError");
      });
    mockInvoke.mockResolvedValue(accounts);

    const state = new AccountState();
    await state.loadAccounts();

    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.isAllAccounts).toBe(true);
    expect(state.activeAccountId).toBeNull();
    expect(state.accounts).toHaveLength(2);
    expect(state.error).toBeNull();

    getItemSpy.mockRestore();
  });

  it("localStorage.setItem 抛错时 setActive/setAllAccounts/delete/load 仍正常工作", async () => {
    const setItemSpy = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw new DOMException("quota", "QuotaExceededError");
      });
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();

    state.setActive(2);
    expect(state.accountScope).toEqual({ kind: "account", accountId: 2 });
    expect(state.activeAccountId).toBe(2);
    expect(state.lastConcreteAccountId).toBe(2);

    state.setAllAccounts();
    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.activeAccountId).toBeNull();
    expect(state.lastConcreteAccountId).toBe(2);

    state.accountScope = { kind: "account", accountId: 999 };
    await state.loadAccounts();
    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.accounts).toHaveLength(2);
    expect(state.error).toBeNull();

    state.accounts = [...accounts];
    state.setActive(1);
    mockInvoke.mockResolvedValueOnce(null);
    await state.deleteAccount(1);
    expect(state.accountScope).toEqual({ kind: "all" });
    expect(state.activeAccountId).toBeNull();
    expect(state.lastConcreteAccountId).toBe(2);
    expect(state.accounts.map((account) => account.id)).toEqual([2]);

    setItemSpy.mockRestore();
  });
});
