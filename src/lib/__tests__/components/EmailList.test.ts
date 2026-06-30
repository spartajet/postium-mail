import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import EmailList from "$lib/components/email/EmailList.svelte";

const loadNextPage = vi.fn();
const loadEmailsByCategory = vi.fn();
const refreshLoadedEmailsByCategory = vi.fn();
const refreshCurrentCategory = vi.fn();
const selectEmail = vi.fn();
const loadHistoryState = vi.fn();
const syncOlderEmails = vi.fn();
const isOlderSyncing = vi.fn();

const baseEmail = {
    id: 1,
    account_id: 1,
    folder: "INBOX",
    uid: 100,
    subject: "Hello",
    sender_name: "Alice",
    sender_email: "alice@example.com",
    preview: "Preview",
    is_read: true,
    is_starred: false,
    sent_at: 1745126400,
    has_attachments: false,
};

let emailState: {
    emails: typeof baseEmail[];
    selectedEmailId: number | null;
    total: number;
    loading: boolean;
    loadingNextPage: boolean;
    currentFolder: "inbox" | "starred";
    operatingIds: Set<number>;
    loadEmailsByCategory: typeof loadEmailsByCategory;
    refreshLoadedEmailsByCategory: typeof refreshLoadedEmailsByCategory;
    refreshCurrentCategory: typeof refreshCurrentCategory;
    selectEmail: typeof selectEmail;
    loadNextPage: typeof loadNextPage;
    toggleStar: ReturnType<typeof vi.fn>;
    markAsRead: ReturnType<typeof vi.fn>;
    deleteEmails: ReturnType<typeof vi.fn>;
    reloadEmail: ReturnType<typeof vi.fn>;
};

let activeAccountId: number | null = 1;
let historyExhausted = false;
let syncingOlder = false;

vi.mock("$lib/stores/email.svelte", () => ({
    getEmailState: () => emailState,
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        get activeAccountId() {
            return activeAccountId;
        },
    }),
}));

vi.mock("$lib/stores/sync.svelte", () => ({
    getSyncState: () => ({
        getHistoryState: vi.fn(() => ({
            account_id: 1,
            category: emailState.currentFolder,
            history_synced_since: 1_700_000_000,
            history_exhausted: historyExhausted,
            folders: ["INBOX"],
        })),
        loadHistoryState,
        syncOlderEmails,
        isOlderSyncing,
    }),
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            sidebar: {
                sync: "同步",
            },
            email: {
                search: "搜索邮件...",
                noEmails: "没有邮件",
                loadMore: "加载更多",
                syncOlder: "同步更久邮件",
                syncingOlder: "正在同步更久...",
            },
        },
    }),
}));

vi.mock("$lib/bindings", () => ({
    commands: {
        searchEmails: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
        getEmail: vi.fn(),
    },
}));

function resetState() {
    emailState = {
        emails: [baseEmail],
        selectedEmailId: null,
        total: 1,
        loading: false,
        loadingNextPage: false,
        currentFolder: "inbox",
        operatingIds: new Set(),
        loadEmailsByCategory,
        refreshLoadedEmailsByCategory,
        refreshCurrentCategory,
        selectEmail,
        loadNextPage,
        toggleStar: vi.fn(),
        markAsRead: vi.fn(),
        deleteEmails: vi.fn(),
        reloadEmail: vi.fn(),
    };
    activeAccountId = 1;
    historyExhausted = false;
    syncingOlder = false;
    syncOlderEmails.mockResolvedValue({ new_emails: 1 });
    isOlderSyncing.mockImplementation(() => syncingOlder);
}

describe("EmailList footer", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        resetState();
    });

    it("本地未加载完时显示加载更多并调用下一页加载", async () => {
        emailState.total = 2;

        render(EmailList);

        const button = screen.getByRole("button", { name: "加载更多" });
        await fireEvent.click(button);

        expect(loadNextPage).toHaveBeenCalledWith(1);
        expect(screen.queryByRole("button", { name: "同步更久邮件" })).toBeNull();
    });

    it("本地加载完且历史未耗尽时显示同步更久邮件并刷新当前已加载范围", async () => {
        render(EmailList);
        const loadEmailsBeforeSyncOlder = loadEmailsByCategory.mock.calls.length;

        await fireEvent.click(
            screen.getByRole("button", { name: "同步更久邮件" }),
        );

        expect(syncOlderEmails).toHaveBeenCalledWith(1, "inbox");
        await waitFor(() => {
            expect(refreshLoadedEmailsByCategory).toHaveBeenCalledWith(1, 2);
        });
        expect(loadEmailsByCategory).toHaveBeenCalledTimes(
            loadEmailsBeforeSyncOlder,
        );
    });

    it("搜索输入防抖期间仍可显示同步更久邮件，搜索结果模式隐藏", async () => {
        const { unmount } = render(EmailList);

        await fireEvent.input(screen.getByTestId("email-search-input"), {
            target: { value: "hello" },
        });

        expect(
            screen.getByRole("button", { name: "同步更久邮件" }),
        ).toBeTruthy();

        await waitFor(() => {
            expect(
                screen.queryByRole("button", { name: "同步更久邮件" }),
            ).toBeNull();
        });
        unmount();
    });

    it("starred 分类不显示同步更久邮件", () => {
        emailState.currentFolder = "starred";
        render(EmailList);

        expect(screen.queryByRole("button", { name: "同步更久邮件" })).toBeNull();
    });

    it("本地不足一页且历史未耗尽时空列表显示同步更久邮件", () => {
        emailState.emails = [];
        emailState.total = 0;

        render(EmailList);

        expect(
            screen.getByRole("button", { name: "同步更久邮件" }),
        ).toBeTruthy();
    });
});
