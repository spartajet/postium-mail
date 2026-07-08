/**
 * Postium Mail - Tauri API 统一 Mock
 * tauri.ts
 *
 * 本文件为测试环境提供 Tauri API 的 Mock 实现。
 * 这些 Mock 用于替代真实的 Tauri API，使测试可以在非 Tauri 环境中运行。
 *
 * ==================== Mock 内容 ====================
 * 1. Tauri Core API: invoke、convertFileSrc
 * 2. Tauri Event API: listen、once、emit
 * 3. 命令结果类型和辅助函数
 *
 * ==================== 使用说明 ====================
 * 这些 Mock 会在测试文件中被导入，并覆盖真实的 Tauri API 实现。
 */

// src/lib/__tests__/mocks/tauri.ts
//
// Unified mock for Tauri APIs used by generated bindings and stores.

import { vi } from "vitest";

/**
 * Mock 命令结果类型
 *
 * 定义 Tauri 命令调用的返回值类型。
 *
 * - { status: "ok"; data: T }: 成功结果，包含返回数据
 * - { status: "error"; error: ... }: 错误结果，包含错误信息
 */
export type MockCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: string | { type: string; message: string } };

/**
 * 创建成功结果
 *
 * 创建一个状态为 "ok" 的 Mock 结果对象。
 *
 * @param data - 返回的数据
 * @returns Mock 结果对象
 */
export function createMockResult<T>(data: T) {
  return { status: "ok" as const, data };
}

/**
 * 创建错误结果
 *
 * 创建一个状态为 "error" 的 Mock 结果对象。
 *
 * @param error - 错误信息
 * @returns Mock 错误对象
 */
export function createMockError(error: string | { type: string; message: string }) {
  return { status: "error" as const, error };
}

/**
 * 断言结果为成功状态
 *
 * 如果结果不是成功状态，抛出错误。
 *
 * @param result - 要检查的结果
 * @throws 如果结果不是成功状态
 */
export function expectOk<T>(
  result: MockCommandResult<T>,
): asserts result is { status: "ok"; data: T } {
  if (result.status !== "ok") {
    throw new Error(`Expected ok result, got ${JSON.stringify(result.error)}`);
  }
}

/**
 * Mock 的 Tauri invoke 函数
 *
 * 用于替代真实的 Tauri invoke 命令调用。
 * 默认返回成功结果，可以在测试中覆盖实现。
 */
export const mockInvoke = vi.fn().mockResolvedValue({ status: "ok", data: null });

/**
 * Mock 的 Tauri listen 函数
 *
 * 用于替代真实的事件监听函数。
 */
export const mockListen = vi.fn();

/**
 * Mock 的 Tauri once 函数
 *
 * 用于替代真实的一次性事件监听函数。
 */
export const mockOnce = vi.fn();

/**
 * Mock 的 Tauri emit 函数
 *
 * 用于替代真实的事件发送函数。
 */
export const mockEmit = vi.fn();

// Mock Tauri Core API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
}));

// Mock Tauri Event API
vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
  once: mockOnce,
  emit: mockEmit,
}));
