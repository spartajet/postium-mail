import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import EmailList from "$lib/components/email/EmailList.svelte";
import type { EmailDto } from "$lib/bindings";

const loadNextPage = vi.fn();
const loadNextPageForAllAccounts = vi.fn();
const loadEmailsByCategory = vi.fn();
const loadEmailsByCategoryForAllAccounts = vi.fn();
const refreshLoadedEmailsByCategory = vi.fn();
const refreshLoadedEmailsByCategoryForAllAccounts = vi.fn();
const refreshCurrentCategory = vi.fn();
const selectEmail = vi.fn();
const loadHistoryState = vi.fn();
const syncOlderEmails = vi.fn();
const isOlderSyncing = vi.fn();
const loadFolderStats = vi.fn();
const loadFolderStatsForAllAccounts = vi.fn();
const syncAccount = vi.fn();
const syncAllAccounts = vi.fn();
const showForward = vi.fn();

const mockCommands = vi.hoisted(() => ({
    searchEmails: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
    getEmail: vi.fn(),
}));

const baseEmail: EmailDto = {
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
    emails: EmailDto[];
    selectedEmailId: number | null;
    total: number;
    page: number;
    loading: boolean;
    loadingNextPage: boolean;
    currentFolder: "inbox" | "starred";
    unreadOnly: boolean;
    operatingIds: Set<number>;
    setUnreadOnly: ReturnType<typeof vi.fn>;
    setUnreadOnlyForAllAccounts: ReturnType<typeof vi.fn>;
    loadEmailsByCategory: typeof loadEmailsByCategory;
    loadEmailsByCategoryForAllAccounts: typeof loadEmailsByCategoryForAllAccounts;
    refreshLoadedEmailsByCategory: typeof refreshLoadedEmailsByCategory;
    refreshLoadedEmailsByCategoryForAllAccounts: typeof refreshLoadedEmailsByCategoryForAllAccounts;
    refreshCurrentCategory: typeof refreshCurrentCategory;
    selectEmail: typeof selectEmail;
    loadNextPage: typeof loadNextPage;
    loadNextPageForAllAccounts: typeof loadNextPageForAllAccounts;
    toggleStar: ReturnType<typeof vi.fn>;
    markAsRead: ReturnType<typeof vi.fn>;
    deleteEmails: ReturnType<typeof vi.fn>;
    reloadEmail: ReturnType<typeof vi.fn>;
};

let activeAccountId: number | null = 1;
let isAllAccounts = false;
let historyExhausted = false;
let historyStateLoaded = true;
let syncingOlder = false;

vi.mock("$lib/stores/email.svelte", () => ({
    getEmailState: () => emailState,
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
        getHistoryState: vi.fn(() =>
            historyStateLoaded
                ? {
                      account_id: 1,
                      category: emailState.currentFolder,
                      history_synced_since: 1_700_000_000,
                      history_before_uid: 500,
                      history_exhausted: historyExhausted,
                      folders: ["INBOX"],
                  }
                : null,
        ),
        loadHistoryState,
        syncOlderEmails,
        isOlderSyncing,
        syncAccount,
        syncAllAccounts,
        loadFolderStats,
        loadFolderStatsForAllAccounts,
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
                syncOlder: "加载更早邮件",
                syncingOlder: "正在加载更早邮件...",
                forward: "转发",
            },
            common: {
                operations: "更多操作",
            },
        },
    }),
}));

vi.mock("$lib/bindings", () => ({
    commands: mockCommands,
}));

const composeContextKey = Symbol.for("compose-modal");

function resetState() {
    emailState = {
        emails: [baseEmail],
        selectedEmailId: null,
        total: 1,
        page: 1,
        loading: false,
        loadingNextPage: false,
        currentFolder: "inbox",
        unreadOnly: false,
        operatingIds: new Set(),
        setUnreadOnly: vi.fn(),
        setUnreadOnlyForAllAccounts: vi.fn(),
        loadEmailsByCategory,
        loadEmailsByCategoryForAllAccounts,
        refreshLoadedEmailsByCategory,
        refreshLoadedEmailsByCategoryForAllAccounts,
        refreshCurrentCategory,
        selectEmail,
        loadNextPage,
        loadNextPageForAllAccounts,
        toggleStar: vi.fn(),
        markAsRead: vi.fn(),
        deleteEmails: vi.fn(),
        reloadEmail: vi.fn(),
    };
    activeAccountId = 1;
    isAllAccounts = false;
    historyExhausted = false;
    historyStateLoaded = true;
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
        expect(screen.queryByRole("button", { name: "加载更早邮件" })).toBeNull();
    });

    it("本地加载完且历史未耗尽时显示加载更早邮件并刷新当前已加载范围", async () => {
        render(EmailList);
        const loadEmailsBeforeSyncOlder = loadEmailsByCategory.mock.calls.length;

        await fireEvent.click(
            screen.getByRole("button", { name: "加载更早邮件" }),
        );

        expect(syncOlderEmails).toHaveBeenCalledWith(1, "inbox");
        await waitFor(() => {
            expect(refreshLoadedEmailsByCategory).toHaveBeenCalledWith(1, 2);
        });
        expect(loadEmailsByCategory).toHaveBeenCalledTimes(
            loadEmailsBeforeSyncOlder,
        );
    });

    it("历史已耗尽时不显示加载更早邮件", () => {
        historyExhausted = true;

        render(EmailList);

        expect(screen.queryByRole("button", { name: "加载更早邮件" })).toBeNull();
    });

    it("历史状态尚未加载完成时仍显示加载更早邮件入口", () => {
        historyStateLoaded = false;

        render(EmailList);

        expect(
            screen.getByRole("button", { name: "加载更早邮件" }),
        ).toBeTruthy();
    });

    it("搜索输入防抖期间仍可显示加载更早邮件，搜索结果模式隐藏", async () => {
        const { unmount } = render(EmailList);

        await fireEvent.input(screen.getByTestId("email-search-input"), {
            target: { value: "hello" },
        });

        expect(
            screen.getByRole("button", { name: "加载更早邮件" }),
        ).toBeTruthy();

        await waitFor(() => {
            expect(
                screen.queryByRole("button", { name: "加载更早邮件" }),
            ).toBeNull();
        });
        unmount();
    });

    it("starred 分类不显示加载更早邮件", () => {
        emailState.currentFolder = "starred";
        render(EmailList);

        expect(screen.queryByRole("button", { name: "加载更早邮件" })).toBeNull();
    });

    it("本地不足一页且历史未耗尽时空列表显示加载更早邮件", () => {
        emailState.emails = [];
        emailState.total = 0;

        render(EmailList);

        expect(
            screen.getByRole("button", { name: "加载更早邮件" }),
        ).toBeTruthy();
    });

    it("点击未读切换时按未读模式重新加载并移除无用布局按钮", async () => {
        render(EmailList);

        expect(screen.queryByLabelText("列表视图")).toBeNull();
        expect(screen.queryByLabelText("网格视图")).toBeNull();

        await fireEvent.click(
            screen.getByRole("button", { name: "仅显示未读邮件" }),
        );

        expect(emailState.setUnreadOnly).toHaveBeenCalledWith(1, true);
    });

    it("未读模式下打开未读邮件后刷新当前未读列表", async () => {
        emailState.unreadOnly = true;
        emailState.emails = [{ ...baseEmail, is_read: false }];

        render(EmailList);
        await fireEvent.click(screen.getByTestId("email-item"));

        expect(selectEmail).toHaveBeenCalledWith(1);
        expect(refreshLoadedEmailsByCategory).toHaveBeenCalledWith(1);
    });

    it("所有账号视图初始化时加载聚合邮件并显示账号来源名称", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.emails = [
            {
                ...baseEmail,
                account_display_name: "工作邮箱",
                account_email: "work@example.com",
            },
            {
                ...baseEmail,
                id: 2,
                subject: "Fallback",
                account_display_name: null,
                account_email: "fallback@example.com",
            },
        ];

        render(EmailList);

        await waitFor(() => {
            expect(loadEmailsByCategoryForAllAccounts).toHaveBeenCalledWith(
                "inbox",
            );
        });
        const sources = screen.getAllByTestId("email-account-source");
        expect(sources[0]!.textContent).toBe("工作邮箱");
        expect(sources[1]!.textContent).toBe("fallback@example.com");
        expect(loadEmailsByCategory).not.toHaveBeenCalled();
    });

    it("所有账号视图下账号来源缺失时回退显示账号 ID", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.emails = [
            {
                ...baseEmail,
                account_id: 42,
                account_display_name: null,
                account_email: null,
            },
        ];

        render(EmailList);

        expect(screen.getByTestId("email-account-source").textContent).toBe(
            "账号 #42",
        );
    });

    it("所有账号视图下刷新时调用全量同步、聚合加载和聚合统计", async () => {
        isAllAccounts = true;
        activeAccountId = null;

        render(EmailList);

        expect(
            (screen.getByTestId("email-refresh-button") as HTMLButtonElement)
                .disabled,
        ).toBe(false);
        expect(
            (
                screen.getByRole("button", {
                    name: "仅显示未读邮件",
                }) as HTMLButtonElement
            ).disabled,
        ).toBe(false);

        await fireEvent.click(screen.getByTestId("email-refresh-button"));

        expect(syncAllAccounts).toHaveBeenCalledOnce();
        expect(loadEmailsByCategoryForAllAccounts).toHaveBeenLastCalledWith(
            "inbox",
            1,
        );
        expect(loadFolderStatsForAllAccounts).toHaveBeenCalledOnce();
        expect(syncAccount).not.toHaveBeenCalled();
        expect(loadFolderStats).not.toHaveBeenCalled();
    });

    it("所有账号视图下搜索时传入 accountId=null", async () => {
        isAllAccounts = true;
        activeAccountId = null;

        render(EmailList);

        await fireEvent.input(screen.getByTestId("email-search-input"), {
            target: { value: "hello" },
        });

        await waitFor(() => {
            expect(mockCommands.searchEmails).toHaveBeenCalledWith(
                "hello",
                null,
                50,
            );
        });
    });

    it("所有账号视图下搜索结果显示账号来源", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        mockCommands.searchEmails.mockResolvedValueOnce({
            status: "ok",
            data: [
                {
                    id: 99,
                    account_id: 2,
                    account_email: "search@example.com",
                    account_display_name: "搜索账号",
                    folder: "INBOX",
                    subject: "Search hit",
                    sender_email: "sender@example.com",
                    sent_at: 1745126400,
                    preview: "Matched preview",
                    rank: 1,
                },
            ],
        });

        render(EmailList);

        await fireEvent.input(screen.getByTestId("email-search-input"), {
            target: { value: "hello" },
        });

        await waitFor(() => {
            expect(screen.getByTestId("email-account-source").textContent).toBe(
                "搜索账号",
            );
        });
    });

    it("所有账号视图下搜索结果来源缺失时回退显示账号 ID", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        mockCommands.searchEmails.mockResolvedValueOnce({
            status: "ok",
            data: [
                {
                    id: 100,
                    account_id: 7,
                    account_email: null,
                    account_display_name: null,
                    folder: "INBOX",
                    subject: "Search fallback",
                    sender_email: "sender@example.com",
                    sent_at: 1745126400,
                    preview: "Matched preview",
                    rank: 1,
                },
            ],
        });

        render(EmailList);

        await fireEvent.input(screen.getByTestId("email-search-input"), {
            target: { value: "fallback" },
        });

        await waitFor(() => {
            expect(screen.getByTestId("email-account-source").textContent).toBe(
                "账号 #7",
            );
        });
    });

    it("所有账号视图下右键转发使用原邮件所属账号", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.emails = [{ ...baseEmail, account_id: 9 }];
        mockCommands.getEmail.mockResolvedValueOnce({
            status: "ok",
            data: {
                ...baseEmail,
                account_id: 9,
                recipient_emails: "me@example.com",
                cc_emails: null,
                body_text: "Body",
                body_html: "<p>Body</p>",
                attachments: [],
            },
        });

        render(EmailList, {
            context: new Map([
                [
                    composeContextKey,
                    () => ({
                        showForward,
                    }),
                ],
            ]),
        });

        await fireEvent.contextMenu(screen.getByTestId("email-item"));
        await fireEvent.click(screen.getByRole("button", { name: "转发" }));

        await waitFor(() => {
            expect(showForward).toHaveBeenCalledWith("Hello", "Body", 9);
        });
    });

    it("所有账号视图下未读筛选调用聚合接口", async () => {
        isAllAccounts = true;
        activeAccountId = null;

        render(EmailList);

        await fireEvent.click(
            screen.getByRole("button", { name: "仅显示未读邮件" }),
        );

        expect(emailState.setUnreadOnlyForAllAccounts).toHaveBeenCalledWith(
            true,
        );
        expect(emailState.setUnreadOnly).not.toHaveBeenCalled();
    });

    it("所有账号视图下加载更多调用聚合分页接口", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.total = 2;

        render(EmailList);

        await fireEvent.click(screen.getByRole("button", { name: "加载更多" }));

        expect(loadNextPageForAllAccounts).toHaveBeenCalledOnce();
        expect(loadNextPage).not.toHaveBeenCalled();
    });

    it("所有账号视图下不显示加载更早邮件入口", () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.total = 0;
        emailState.emails = [];

        render(EmailList);

        expect(screen.queryByRole("button", { name: "加载更早邮件" })).toBeNull();
        expect(loadHistoryState).not.toHaveBeenCalled();
    });

    it("所有账号未读视图下打开未读邮件后刷新聚合已加载列表", async () => {
        isAllAccounts = true;
        activeAccountId = null;
        emailState.unreadOnly = true;
        emailState.emails = [
            {
                ...baseEmail,
                is_read: false,
                account_display_name: "工作邮箱",
                account_email: "work@example.com",
            },
        ];

        render(EmailList);

        await fireEvent.click(screen.getByTestId("email-item"));

        expect(selectEmail).toHaveBeenCalledWith(1);
        expect(loadFolderStatsForAllAccounts).toHaveBeenCalled();
        expect(
            refreshLoadedEmailsByCategoryForAllAccounts,
        ).toHaveBeenCalledOnce();
        expect(refreshLoadedEmailsByCategory).not.toHaveBeenCalled();
    });
});
