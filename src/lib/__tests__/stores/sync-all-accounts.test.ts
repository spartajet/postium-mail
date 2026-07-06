import { beforeEach, describe, expect, it, vi } from "vitest";
import { SyncState } from "$lib/stores/sync.svelte";
import { mockInvoke } from "../mocks/tauri";

describe("SyncState 所有账号能力", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loadFolderStatsForAllAccounts 成功后更新 folderStats", async () => {
    const folderStats = [
      { folder: "inbox", total: 20, unread: 5 },
      { folder: "sent", total: 8, unread: 0 },
    ];
    mockInvoke.mockResolvedValue(folderStats);
    const state = new SyncState();

    await state.loadFolderStatsForAllAccounts();

    expect(state.folderStats).toEqual(folderStats);
    expect(mockInvoke).toHaveBeenCalledWith("get_folder_stats_for_all_accounts");
  });

  it("syncAllAccounts 返回聚合同步结果", async () => {
    const syncResult = {
      total: 3,
      succeeded: [1, 2],
      failed: [
        {
          account_id: 3,
          email: "broken@example.com",
          message: "auth failed",
        },
      ],
    };
    mockInvoke.mockResolvedValue(syncResult);
    const state = new SyncState();

    const result = await state.syncAllAccounts();

    expect(state.syncing).toBe(false);
    expect(state.error).toBeNull();
    expect(result).toEqual(syncResult);
    expect(mockInvoke).toHaveBeenCalledWith("sync_all_accounts");
  });
});
