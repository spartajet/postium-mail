import { describe, it, expect, vi } from "vitest";
import { continueAfterAccountAdded } from "$lib/components/settings/account-add-flow";

describe("continueAfterAccountAdded", () => {
  it("先切换到主界面，再后台启动首次同步", async () => {
    const calls: string[] = [];
    let releaseSync: (() => void) | undefined;

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
      syncAccount: vi.fn(
        () =>
          new Promise<void>((resolve) => {
            calls.push("syncAccount:start");
            releaseSync = resolve;
          }),
      ),
    });

    expect(calls).toEqual([
      "loadAccounts",
      "setActive:7",
      "close",
      "goHome",
      "syncAccount:start",
    ]);
    expect(result.syncStarted).toBe(true);

    releaseSync?.();
  });
});
