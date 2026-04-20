// src/lib/__tests__/mocks/tauri.ts
//
// 统一 mock @tauri-apps/api 的 invoke 函数

import { vi } from "vitest";

// Mock commands 返回结构
// 返回值格式: { status: "ok", data: ... } 或 { status: "error", error: ... }
export function createMockResult<T>(data: T) {
  return { status: "ok" as const, data };
}

export function createMockError(error: string) {
  return { status: "error" as const, error };
}

// 创建 Tauri invoke mock
export const mockInvoke = vi.fn().mockResolvedValue({ status: "ok", data: null });

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));
