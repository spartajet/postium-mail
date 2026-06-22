import { beforeEach, describe, expect, it, vi } from "vitest";
import { AccountState } from "$lib/stores/account.svelte";
import { mockInvoke } from "../mocks/tauri";

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
  });

  it("loadAccounts 成功后写入账号并自动选中第一个账号", async () => {
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();

    await state.loadAccounts();

    expect(state.loading).toBe(false);
    expect(state.error).toBeNull();
    expect(state.accounts).toHaveLength(2);
    expect(state.activeAccountId).toBe(1);
    expect(state.activeAccount?.email).toBe("work@example.com");
    expect(mockInvoke).toHaveBeenCalledWith("list_accounts");
  });

  it("loadAccounts 不覆盖已有活跃账号", async () => {
    mockInvoke.mockResolvedValue(accounts);
    const state = new AccountState();
    state.activeAccountId = 2;

    await state.loadAccounts();

    expect(state.activeAccountId).toBe(2);
    expect(state.activeAccount?.email).toBe("personal@example.com");
  });

  it("loadAccounts 记录后端错误并结束 loading", async () => {
    mockInvoke.mockRejectedValue({ type: "DatabaseError", message: "数据库不可用" });
    const state = new AccountState();

    await state.loadAccounts();

    expect(state.loading).toBe(false);
    expect(state.accounts).toEqual([]);
    expect(state.error).toBe("数据库不可用");
  });

  it("deleteAccount 删除当前账号后切换到剩余账号", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new AccountState();
    state.accounts = [...accounts];
    state.activeAccountId = 1;

    await state.deleteAccount(1);

    expect(state.accounts.map((account) => account.id)).toEqual([2]);
    expect(state.activeAccountId).toBe(2);
    expect(state.activeAccount?.email).toBe("personal@example.com");
    expect(mockInvoke).toHaveBeenCalledWith("delete_account", { id: 1 });
  });

  it("deleteAccount 删除最后一个账号后清空活跃账号", async () => {
    mockInvoke.mockResolvedValue(null);
    const state = new AccountState();
    state.accounts = [accounts[0]!];
    state.activeAccountId = 1;

    await state.deleteAccount(1);

    expect(state.accounts).toEqual([]);
    expect(state.activeAccountId).toBeNull();
    expect(state.activeAccount).toBeNull();
  });
});
