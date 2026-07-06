/**
 * Postium Mail - 侧边栏组件测试
 * Sidebar.test.ts
 *
 * 本文件测试侧边栏组件的功能，包括文件夹导航、
 * 未读数量显示、账号切换、同步功能等。
 *
 * ==================== 测试范围 ====================
 * 1. 文件夹未读数量角标显示
 * 2. 设置窗口打开（独立窗口 vs 路由回退）
 * 3. 账号下拉菜单和所有账号视图切换
 * 4. 同步按钮功能
 * 5. 文件夹点击加载邮件
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "$lib/components/layout/Sidebar.svelte";
import type { AccountDto, FolderStat } from "$lib/bindings";

const loadAccounts = vi.fn();
const setActive = vi.fn();
const setAllAccounts = vi.fn();
const loadEmailsByCategory = vi.fn();
const loadEmailsByCategoryForAllAccounts = vi.fn();
const deselectEmail = vi.fn();
const syncAccount = vi.fn();
const syncAllAccounts = vi.fn();
const loadFolderStats = vi.fn();
const loadFolderStatsForAllAccounts = vi.fn();

const mocks = vi.hoisted(() => ({
    goto: vi.fn(),
    openSettingsWindow: vi.fn(),
}));

const account: AccountDto = {
    id: 1,
    name: "Primary",
    email: "primary@example.com",
    display_name: null,
    provider: "imap",
    color: null,
    sync_enabled: true,
    auth_type: "password",
    account_type: "personal",
    last_sync_at: null,
    created_at: 1,
};

let activeAccountId: number | null = 1;
let isAllAccounts = false;
let folderStats: FolderStat[] = [];

vi.mock("$app/navigation", () => ({
    goto: mocks.goto,
}));

vi.mock("$lib/bindings", async (importOriginal) => {
    const original = await importOriginal<typeof import("$lib/bindings")>();
    return {
        ...original,
        commands: {
            ...original.commands,
            openSettingsWindow: mocks.openSettingsWindow,
        },
    };
});

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            sidebar: {
                sync: "同步",
                allAccounts: "所有账号",
                compose: "写邮件",
                inbox: "收件箱",
                starred: "星标",
                sent: "已发送",
                drafts: "草稿",
                spam: "垃圾邮件",
                trash: "回收站",
                labelUrgent: "紧急",
                labelWork: "工作",
                labelPersonal: "个人",
                labelFinance: "财务",
                calendar: "日历",
                workflow: "工作流",
                settings: "设置",
            },
            account: {
                add: "添加账号",
            },
        },
    }),
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts: [account],
        get isAllAccounts() {
            return isAllAccounts;
        },
        get activeAccountId() {
            return activeAccountId;
        },
        get activeAccount() {
            return activeAccountId === account.id ? account : null;
        },
        loadAccounts,
        setActive,
        setAllAccounts,
    }),
}));

vi.mock("$lib/stores/email.svelte", () => ({
    getEmailState: () => ({
        currentFolder: "inbox",
        loadEmailsByCategory,
        loadEmailsByCategoryForAllAccounts,
        deselectEmail,
    }),
}));

vi.mock("$lib/stores/sync.svelte", () => ({
    getSyncState: () => ({
        syncing: false,
        get folderStats() {
            return folderStats;
        },
        syncAccount,
        syncAllAccounts,
        loadFolderStats,
        loadFolderStatsForAllAccounts,
    }),
}));

describe("Sidebar unread badges", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        activeAccountId = 1;
        isAllAccounts = false;
        folderStats = [
            { folder: "inbox", total: 10, unread: 3 },
            { folder: "starred", total: 4, unread: 2 },
            { folder: "sent", total: 6, unread: 0 },
        ];
    });

    it("在文件夹名称右侧显示分类未读数", async () => {
        render(Sidebar);

        await waitFor(() => {
            expect(loadFolderStats).toHaveBeenCalledWith(1);
        });
        expect(screen.getByTestId("folder-inbox").textContent).toContain("3");
        expect(screen.getByTestId("folder-starred").textContent).toContain("2");
        expect(screen.getByTestId("folder-sent").textContent).not.toContain("0");
    });

    it("点击设置入口时打开独立设置窗口", async () => {
        mocks.openSettingsWindow.mockResolvedValue({
            status: "ok",
            data: null,
        });

        render(Sidebar);

        await fireEvent.click(screen.getByTestId("settings-nav"));

        expect(mocks.openSettingsWindow).toHaveBeenCalledOnce();
        expect(mocks.goto).not.toHaveBeenCalledWith("/settings");
    });

    it("设置窗口打开失败时回退到设置路由", async () => {
        mocks.openSettingsWindow.mockResolvedValue({
            status: "error",
            error: { type: "InvalidParam", message: "failed" },
        });

        render(Sidebar);

        await fireEvent.click(screen.getByTestId("settings-nav"));

        expect(mocks.openSettingsWindow).toHaveBeenCalledOnce();
        expect(mocks.goto).toHaveBeenCalledWith("/settings");
    });

    it("账号下拉显示所有账号选项并在点击后切换到聚合视图", async () => {
        render(Sidebar);

        await fireEvent.click(screen.getByTestId("account-switcher"));
        await fireEvent.click(screen.getByTestId("account-option-all"));

        expect(setAllAccounts).toHaveBeenCalledOnce();
        expect(deselectEmail).toHaveBeenCalledOnce();
        expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith(
            "inbox",
        );
        expect(loadFolderStatsForAllAccounts).toHaveBeenCalledOnce();
    });

    it("所有账号视图下点击文件夹时加载所有账号邮件", async () => {
        isAllAccounts = true;
        activeAccountId = null;

        render(Sidebar);

        expect(screen.getByTestId("active-account-label").textContent).toBe(
            "所有账号",
        );

        await fireEvent.click(screen.getByTestId("folder-starred"));

        expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith(
            "starred",
        );
        expect(loadEmailsByCategory).not.toHaveBeenCalled();
    });

    it("所有账号视图下同步后刷新当前分类和聚合统计", async () => {
        isAllAccounts = true;
        activeAccountId = null;

        render(Sidebar);

        await fireEvent.click(screen.getByTitle("同步"));

        expect(syncAllAccounts).toHaveBeenCalledOnce();
        expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith(
            "inbox",
        );
        expect(loadFolderStatsForAllAccounts).toHaveBeenCalled();
        expect(syncAccount).not.toHaveBeenCalled();
    });
});
