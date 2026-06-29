// src/lib/__tests__/mocks/tauri.ts
//
// Unified mock for Tauri APIs used by generated bindings and stores.

import { vi } from "vitest";

export type MockCommandResult<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: string | { type: string; message: string } };

export function createMockResult<T>(data: T) {
  return { status: "ok" as const, data };
}

export function createMockError(error: string | { type: string; message: string }) {
  return { status: "error" as const, error };
}

export function expectOk<T>(
  result: MockCommandResult<T>,
): asserts result is { status: "ok"; data: T } {
  if (result.status !== "ok") {
    throw new Error(`Expected ok result, got ${JSON.stringify(result.error)}`);
  }
}

export const mockInvoke = vi.fn().mockResolvedValue({ status: "ok", data: null });
export const mockListen = vi.fn();
export const mockOnce = vi.fn();
export const mockEmit = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
  convertFileSrc: (path: string) => `asset://localhost/${path}`,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mockListen,
  once: mockOnce,
  emit: mockEmit,
}));
