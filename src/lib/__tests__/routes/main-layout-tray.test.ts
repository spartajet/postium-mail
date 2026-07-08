/**
 * Postium Mail - 主布局托盘同步测试
 * main-layout-tray.test.ts
 *
 * 本文件测试主布局的托盘同步功能，验证在不同账号视图下
 * 托盘同步事件触发时调用正确的同步方法。
 *
 * ==================== 测试范围 ====================
 * 1. 所有账号视图下托盘同步调用 syncAllAccounts
 * 2. 具体账号视图下托盘同步调用 syncAccount
 */

import { render, waitFor } from "@testing-library/svelte";
import type { Snippet } from "svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MainLayout from "../../../routes/(main)/+layout.svelte";

const mocks = vi.hoisted(() => ({
    listen: vi.fn(),
    syncAccount: vi.fn(),
    syncAllAccounts: vi.fn(),
}));

let isAllAccounts = false;
let activeAccountId: number | null = 1;
const emptyChildren = (() => null) as unknown as Snippet;

vi.mock("@tauri-apps/api/event", () => ({
    listen: mocks.listen,
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        get isAllAccounts() {
            return isAllAccounts;
        },
        get activeAccountId() {
            return activeAccountId;
        },
    }),
}));

vi.mock("$lib/stores/sync.svelte", () => ({
    getSyncState: () => ({
        syncAccount: mocks.syncAccount,
        syncAllAccounts: mocks.syncAllAccounts,
    }),
}));

vi.mock("$lib/components/email/ComposeModal.svelte", () => ({
    default: vi.fn(() => null),
}));

vi.mock("$lib/components/settings/AddAccountModal.svelte", () => ({
    default: vi.fn(() => null),
}));

vi.mock("$lib/components/layout/Sidebar.svelte", () => ({
    default: vi.fn(() => null),
}));

vi.mock("$lib/components/layout/StatusBar.svelte", () => ({
    default: vi.fn(() => null),
}));

vi.mock("$lib/components/layout/TitleBar.svelte", () => ({
    default: vi.fn(() => null),
}));

describe("Main layout tray sync", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        isAllAccounts = false;
        activeAccountId = 1;
        mocks.listen.mockResolvedValue(() => {});
    });

    it("所有账号范围下托盘同步调用 syncAllAccounts", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        render(MainLayout, { props: { children: emptyChildren } });

        await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
        const handler = mocks.listen.mock.calls[0]![1];
        handler({ payload: "sync" });

        expect(mocks.syncAllAccounts).toHaveBeenCalledOnce();
        expect(mocks.syncAccount).not.toHaveBeenCalled();
    });

    it("具体账号范围下托盘同步调用 syncAccount", async () => {
        isAllAccounts = false;
        activeAccountId = 2;
        render(MainLayout, { props: { children: emptyChildren } });

        await waitFor(() => expect(mocks.listen).toHaveBeenCalledOnce());
        const handler = mocks.listen.mock.calls[0]![1];
        handler({ payload: "sync" });

        expect(mocks.syncAccount).toHaveBeenCalledWith(2);
        expect(mocks.syncAllAccounts).not.toHaveBeenCalled();
    });
});
