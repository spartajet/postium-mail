/**
 * Postium Mail - 账号命令测试
 * account.test.ts
 *
 * 本文件测试账号相关的 Tauri 命令调用，确保命令参数
 * 和返回值正确传递。
 *
 * ==================== 测试范围 ====================
 * 1. listAccounts - 获取账号列表
 * 2. getAccount - 获取指定账号详情
 * 3. deleteAccount - 删除账号
 * 4. createAccount - 创建账号
 * 5. detectProvider - 检测服务商
 * 6. 错误响应处理
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  createMockResult,
  expectOk,
  type MockCommandResult,
  mockInvoke,
} from "../mocks/tauri";

// Import after mock
const { invoke } = await import("@tauri-apps/api/core");

type Account = {
  id: number;
  name: string;
  email: string;
  display_name: string | null;
  provider: string;
  color: string | null;
  sync_enabled: boolean;
  auth_type: string;
  account_type: string;
  last_sync_at: number | null;
  created_at: number;
};

type ProviderDetection = {
  detected: boolean;
  provider_id: string;
  provider_name: string;
  auth_types: string;
};

function invokeMock<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<MockCommandResult<T>> {
  return invoke<MockCommandResult<T>>(command, args);
}

describe("AccountState invoke 测试", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("listAccounts 获取账号列表", async () => {
    const accounts = [
      {
        id: 1,
        name: "Gmail",
        email: "user@gmail.com",
        display_name: "User",
        provider: "gmail",
        color: "#FF0000",
        sync_enabled: true,
        auth_type: "Password",
        account_type: "personal",
        last_sync_at: null,
        created_at: 1745126400,
      },
    ];
    mockInvoke.mockResolvedValue(createMockResult(accounts));

    const result = await invokeMock<Account[]>("list_accounts");
    expect(result.status).toBe("ok");
    expectOk(result);
    expect(result.data).toHaveLength(1);
    expect(result.data[0]!.email).toBe("user@gmail.com");
  });

  it("getAccount 获取指定账号详情", async () => {
    const account = {
      id: 1,
      name: "Gmail",
      email: "user@gmail.com",
      display_name: "User",
      provider: "gmail",
      color: "#FF0000",
      sync_enabled: true,
      auth_type: "Password",
      account_type: "personal",
      last_sync_at: null,
      created_at: 1745126400,
    };
    mockInvoke.mockResolvedValue(createMockResult(account));

    const result = await invokeMock<Account>("get_account", { id: 1 });
    expect(result.status).toBe("ok");
    expectOk(result);
    expect(result.data.id).toBe(1);
    expect(result.data.email).toBe("user@gmail.com");
    expect(mockInvoke).toHaveBeenCalledWith("get_account", { id: 1 });
  });

  it("deleteAccount 调用删除命令", async () => {
    mockInvoke.mockResolvedValue(createMockResult(null));
    await invokeMock<null>("delete_account", { id: 1 });
    expect(mockInvoke).toHaveBeenCalledWith("delete_account", { id: 1 });
  });

  it("createAccount 调用创建命令（参数使用 request 包装）", async () => {
    const newAccount = {
      id: 2,
      name: "QQ",
      email: "user@qq.com",
      display_name: null,
      provider: "qq",
      color: null,
      sync_enabled: true,
      auth_type: "Password",
      account_type: "personal",
      last_sync_at: null,
      created_at: 1745126400,
    };
    mockInvoke.mockResolvedValue(createMockResult(newAccount));

    const request = {
      name: "QQ",
      email: "user@qq.com",
      display_name: null,
      provider: "qq",
      auth_type: "Password",
      password: "test",
      imap_host: null,
      imap_port: null,
      imap_ssl_mode: null,
      smtp_host: null,
      smtp_port: null,
      smtp_ssl_mode: null,
      color: null,
      account_type: null,
    };

    const result = await invokeMock<Account>("create_account", { request });
    expect(result.status).toBe("ok");
    expectOk(result);
    expect(result.data.email).toBe("user@qq.com");
    expect(mockInvoke).toHaveBeenCalledWith("create_account", { request });
  });

  it("detectProvider 返回检测结果", async () => {
    mockInvoke.mockResolvedValue(
      createMockResult({
        detected: true,
        provider_id: "gmail",
        provider_name: "Gmail",
        auth_types: "Password",
      }),
    );
    const result = await invokeMock<ProviderDetection>("detect_provider", {
      email: "user@gmail.com",
    });
    expectOk(result);
    expect(result.data.detected).toBe(true);
    expect(result.data.provider_id).toBe("gmail");
    expect(mockInvoke).toHaveBeenCalledWith("detect_provider", {
      email: "user@gmail.com",
    });
  });

  it("处理错误响应", async () => {
    mockInvoke.mockResolvedValue({
      status: "error",
      error: { type: "NotFound", message: "账号不存在: 999" },
    });
    const result = await invokeMock<Account>("get_account", { id: 999 });
    expect(result.status).toBe("error");
  });
});
