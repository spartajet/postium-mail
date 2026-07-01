import { render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Sidebar from "$lib/components/layout/Sidebar.svelte";
import type { AccountDto, FolderStat } from "$lib/bindings";

const loadAccounts = vi.fn();
const setActive = vi.fn();
const loadEmailsByCategory = vi.fn();
const deselectEmail = vi.fn();
const syncAccount = vi.fn();
const loadFolderStats = vi.fn();

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
let folderStats: FolderStat[] = [];

vi.mock("$app/navigation", () => ({
    goto: vi.fn(),
}));

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
        get activeAccountId() {
            return activeAccountId;
        },
        get activeAccount() {
            return activeAccountId === account.id ? account : null;
        },
        loadAccounts,
        setActive,
    }),
}));

vi.mock("$lib/stores/email.svelte", () => ({
    getEmailState: () => ({
        currentFolder: "inbox",
        loadEmailsByCategory,
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
        loadFolderStats,
    }),
}));

describe("Sidebar unread badges", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        activeAccountId = 1;
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
});
