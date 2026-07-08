/**
 * Postium Mail - 同步历史测试
 * sync-history.test.ts
 *
 * 本文件测试同步历史功能，包括加载历史状态、
 * 同步更早邮件、范围同步等。
 *
 * ==================== 测试范围 ====================
 * 1. getSyncHistoryState - 获取同步历史状态
 * 2. syncOlderEmails - 同步更早的邮件
 * 3. syncAccountWithRange - 按范围同步账号
 * 4. loadHistoryState - 加载并缓存历史状态
 * 5. isOlderSyncing - 检查同步状态
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { commands } from "$lib/bindings";
import { SyncState } from "$lib/stores/sync.svelte";
import { mockInvoke } from "../mocks/tauri";

describe("同步历史命令调用", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("getSyncHistoryState 使用账号和分类参数", async () => {
    mockInvoke.mockResolvedValue({
      account_id: 1,
      category: "inbox",
      history_synced_since: 1_700_000_000,
      history_before_uid: 500,
      history_exhausted: false,
      folders: ["INBOX"],
    });

    const result = await commands.getSyncHistoryState(1, "inbox");

    expect(result).toMatchObject({ status: "ok" });
    expect(mockInvoke).toHaveBeenCalledWith("get_sync_history_state", {
      accountId: 1,
      category: "inbox",
    });
  });

  it("syncOlderEmails 使用账号和分类参数", async () => {
    mockInvoke.mockResolvedValue({
      new_emails: 3,
      updated_emails: 0,
      window_start: 1_600_000_000,
      window_end: 1_700_000_000,
      history_exhausted: false,
      folders: ["INBOX"],
    });

    const result = await commands.syncOlderEmails(1, "inbox");

    expect(result).toMatchObject({ status: "ok" });
    expect(mockInvoke).toHaveBeenCalledWith("sync_older_emails", {
      accountId: 1,
      category: "inbox",
    });
  });

  it("syncAccountWithRange 使用账号和范围参数", async () => {
    mockInvoke.mockResolvedValue(null);

    const result = await commands.syncAccountWithRange(1, "month");

    expect(result).toMatchObject({ status: "ok" });
    expect(mockInvoke).toHaveBeenCalledWith("sync_account_with_range", {
      accountId: 1,
      range: "month",
    });
  });
});

describe("SyncState 历史同步状态", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loadHistoryState 成功后按账号和分类缓存历史状态", async () => {
    const historyState = {
      account_id: 1,
      category: "inbox",
      history_synced_since: 1_700_000_000,
      history_before_uid: 500,
      history_exhausted: false,
      folders: ["INBOX"],
    };
    mockInvoke.mockResolvedValue(historyState);
    const state = new SyncState();

    const result = await state.loadHistoryState(1, "inbox");

    expect(result).toEqual(historyState);
    expect(state.getHistoryState(1, "inbox")).toEqual(historyState);
    expect(mockInvoke).toHaveBeenCalledWith("get_sync_history_state", {
      accountId: 1,
      category: "inbox",
    });
  });

  it("syncOlderEmails 成功后刷新历史状态并清除加载标记", async () => {
    const olderResult = {
      new_emails: 3,
      updated_emails: 0,
      window_start: 1_600_000_000,
      window_end: 1_700_000_000,
      history_exhausted: false,
      folders: ["INBOX"],
    };
    const historyState = {
      account_id: 1,
      category: "inbox",
      history_synced_since: 1_600_000_000,
      history_before_uid: 450,
      history_exhausted: false,
      folders: ["INBOX"],
    };
    mockInvoke
      .mockResolvedValueOnce(olderResult)
      .mockResolvedValueOnce(historyState);
    const state = new SyncState();

    const promise = state.syncOlderEmails(1, "inbox");

    expect(state.isOlderSyncing(1, "inbox")).toBe(true);
    await expect(promise).resolves.toEqual(olderResult);
    expect(state.isOlderSyncing(1, "inbox")).toBe(false);
    expect(state.getHistoryState(1, "inbox")).toEqual(historyState);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, "sync_older_emails", {
      accountId: 1,
      category: "inbox",
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, "get_sync_history_state", {
      accountId: 1,
      category: "inbox",
    });
  });
});
