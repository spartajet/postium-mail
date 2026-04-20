import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri invoke
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));

// Mock @tauri-apps/api/event (used by bindings)
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
  once: vi.fn(),
  emit: vi.fn(),
}));

// Import after mock
const { invoke } = await import("@tauri-apps/api/core");

function createMockResult<T>(data: T) {
  return { status: "ok" as const, data };
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

    const result = await invoke("list_accounts");
    expect(result.status).toBe("ok");
    expect(result.data).toHaveLength(1);
    expect(result.data[0].email).toBe("user@gmail.com");
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

    const result = await invoke("get_account", { id: 1 });
    expect(result.status).toBe("ok");
    expect(result.data.id).toBe(1);
    expect(result.data.email).toBe("user@gmail.com");
    expect(mockInvoke).toHaveBeenCalledWith("get_account", { id: 1 });
  });

  it("deleteAccount 调用删除命令", async () => {
    mockInvoke.mockResolvedValue(createMockResult(null));
    await invoke("delete_account", { id: 1 });
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

    const result = await invoke("create_account", { request });
    expect(result.status).toBe("ok");
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
    const result = await invoke("detect_provider", { email: "user@gmail.com" });
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
    const result = await invoke("get_account", { id: 999 });
    expect(result.status).toBe("error");
  });
});
