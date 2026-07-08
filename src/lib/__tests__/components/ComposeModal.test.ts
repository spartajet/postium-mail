/**
 * Postium Mail - 写邮件弹窗组件测试
 * ComposeModal.test.ts
 *
 * 本文件测试写邮件弹窗的功能，包括发件账号选择、
 * 邮件发送、回复和转发等。
 *
 * ==================== 测试范围 ====================
 * 1. 发件账号选择器显示和隐藏
 * 2. 所有账号视图下的发件账号选择
 * 3. 单账号视图下的默认发件账号
 * 4. show() 方法传参时的账号优先级
 * 5. 无账号时的禁用状态
 * 6. 回复和转发时的账号传递
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ComposeModal from "$lib/components/email/ComposeModal.svelte";
import { mockInvoke } from "../mocks/tauri";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(),
}));

const defaultAccounts = [
    {
        id: 1,
        name: "Work",
        email: "work@example.com",
        display_name: "Work",
    },
    {
        id: 2,
        name: "Personal",
        email: "personal@example.com",
        display_name: "Personal",
    },
];

let accounts = [...defaultAccounts];
let isAllAccounts = true;
let activeAccountId: number | null = null;
let lastConcreteAccountId: number | null = 2;

function selectValue(element: HTMLElement) {
    return element.dataset.value;
}

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts,
        get isAllAccounts() {
            return isAllAccounts;
        },
        get activeAccountId() {
            return activeAccountId;
        },
        get lastConcreteAccountId() {
            return lastConcreteAccountId;
        },
    }),
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            sidebar: { compose: "写邮件" },
            email: {
                to: "收件人",
                cc: "抄送",
                bcc: "密送",
                showCc: "抄送",
                showBcc: "密送",
                subject: "主题",
                attach: "添加附件",
                removeAttachment: "移除附件",
                draftSaving: "草稿保存中",
                draftSaved: "草稿已保存",
                draftSaveFailed: "草稿保存失败",
                attachmentUnavailable: "附件不可用，请重新选择",
                sendRequiresRecipient: "请填写至少一个收件人",
                sendRequiresSubject: "请填写邮件主题",
                sendRequiresBody: "请填写邮件正文",
                invalidRecipients: "请修正或删除无效收件人",
                send: "发送",
                loading: "发送中",
            },
            common: { cancel: "取消" },
        },
    }),
}));

function defaultInvoke(cmd: string, args?: { input?: unknown }) {
    if (cmd === "parse_email_addresses") {
        const input = String(args?.input ?? "");
        if (input === "bad") {
            return Promise.resolve({
                addresses: [],
                invalid: [{ raw: "bad", reason: "无法解析邮件地址" }],
                duplicates: [],
            });
        }
        return Promise.resolve({
            addresses: [{ name: null, email: input, raw: input }],
            invalid: [],
            duplicates: [],
        });
    }

    return Promise.resolve({
        message_id: "<message-id@example.com>",
        local_email_id: 1,
        remote_archived: true,
        remote_archive_error: null,
    });
}

async function addRecipient(testIdPrefix: string, value: string) {
    const input = await screen.findByTestId(`${testIdPrefix}-recipient-input`);
    await fireEvent.input(input, { target: { value } });
    await fireEvent.keyDown(input, { key: "Enter" });
}

async function fillBody(value: string) {
    await waitFor(() => {
        expect(document.querySelector(".ProseMirror")).toBeTruthy();
    });
    const editor = document.querySelector(".ProseMirror") as HTMLElement;
    editor.textContent = value;
    await fireEvent.input(editor);
}

describe("ComposeModal 发件账号选择", () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        mockInvoke.mockImplementation(defaultInvoke);
        accounts = [...defaultAccounts];
        isAllAccounts = true;
        activeAccountId = null;
        lastConcreteAccountId = 2;
        vi.mocked(openDialog).mockReset();
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it("所有账号视图下显示发件账号选择器并默认上次具体账号", async () => {
        const { component } = render(ComposeModal);

        component.show();

        const select = await screen.findByTestId("compose-account-select");
        expect(selectValue(select)).toBe("2");
        expect(select.tagName).toBe("BUTTON");
        expect(select.classList.contains("bg-transparent")).toBe(true);
    });

    it("所有账号视图下发送使用选择的发件账号", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-account-select"));
        await fireEvent.click(await screen.findByTestId("compose-account-option-1"));
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ account_id: 1 }),
        });
    });

    it("非所有账号视图下不显示发件账号选择器且发送使用当前活跃账号", async () => {
        isAllAccounts = false;
        activeAccountId = 1;
        lastConcreteAccountId = 2;
        const { component } = render(ComposeModal);

        component.show();

        expect(await screen.findByTestId("to-recipient-input")).toBeTruthy();
        expect(screen.queryByTestId("compose-account-select")).toBeNull();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ account_id: 1 }),
        });
    });

    it("show 传入账号时优先使用显式账号", async () => {
        const { component } = render(ComposeModal);

        component.show({ accountId: 1 });

        const select = await screen.findByTestId("compose-account-select");
        expect(selectValue(select)).toBe("1");
    });

    it("没有上次具体账号时回退到当前活跃账号", async () => {
        activeAccountId = 1;
        lastConcreteAccountId = null;
        const { component } = render(ComposeModal);

        component.show();

        const select = await screen.findByTestId("compose-account-select");
        expect(selectValue(select)).toBe("1");
    });

    it("没有上次具体账号和当前活跃账号时回退到第一个账号", async () => {
        activeAccountId = null;
        lastConcreteAccountId = null;
        const { component } = render(ComposeModal);

        component.show();

        const select = await screen.findByTestId("compose-account-select");
        expect(selectValue(select)).toBe("1");
    });

    it("没有可用账号时禁用发送且不发送", async () => {
        accounts = [];
        activeAccountId = null;
        lastConcreteAccountId = null;
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });

        const sendButton = screen.getByTestId("compose-send-button");
        expect((sendButton as HTMLButtonElement).disabled).toBe(true);
        await fireEvent.click(sendButton);
        expect(mockInvoke).not.toHaveBeenCalledWith(
            "send_email",
            expect.anything(),
        );
    });

    it("主题为空时不发送并显示校验错误", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fillBody("Body");

        const sendButton = screen.getByTestId("compose-send-button");
        expect((sendButton as HTMLButtonElement).disabled).toBe(false);
        await fireEvent.click(sendButton);
        expect(await screen.findByText("请填写邮件主题")).toBeTruthy();
        expect(mockInvoke).not.toHaveBeenCalledWith(
            "send_email",
            expect.anything(),
        );
    });

    it("正文为空时不发送并显示校验错误", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });

        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(await screen.findByText("请填写邮件正文")).toBeTruthy();
        expect(mockInvoke).not.toHaveBeenCalledWith(
            "send_email",
            expect.anything(),
        );
    });

    it("展开 Bcc 后发送请求包含密送地址", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.click(screen.getByTestId("compose-show-bcc-button"));
        await addRecipient("bcc", "hidden@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                to: ["to@example.com"],
                bcc: ["hidden@example.com"],
            }),
        });
    });

    it("存在无效收件人 chip 时阻止发送", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "bad");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(await screen.findByText("请修正或删除无效收件人")).toBeTruthy();
        expect(mockInvoke).not.toHaveBeenCalledWith(
            "send_email",
            expect.anything(),
        );
    });

    it("草稿保存包含有效 Bcc 地址", async () => {
        vi.useFakeTimers();
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-show-bcc-button"));
        await addRecipient("bcc", "hidden@example.com");

        await vi.runOnlyPendingTimersAsync();

        expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
            request: expect.objectContaining({
                bcc: ["hidden@example.com"],
            }),
        });
    });

    it("showForward 不会泄漏旧 Bcc 和 draft_id", async () => {
        vi.useFakeTimers();
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return Promise.resolve({
                    draft_id: 42,
                    message_id: "<draft@example.com>",
                    folder: "Drafts",
                    saved_at: 123,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-show-bcc-button"));
        await addRecipient("bcc", "hidden@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Old subject" },
        });
        await fillBody("Old body");
        await vi.advanceTimersByTimeAsync(900);
        await waitFor(() => expect(screen.getByText("草稿已保存")).toBeTruthy());

        component.showForward("Forwarded", "Forward body", 1);
        await addRecipient("to", "forward@example.com");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(screen.queryByText("hidden@example.com")).toBeNull();
        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                to: ["forward@example.com"],
                bcc: [],
                draft_id: null,
            }),
        });
    });

    it("show 新建邮件不会泄漏旧 Bcc 和 draft_id", async () => {
        vi.useFakeTimers();
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return Promise.resolve({
                    draft_id: 99,
                    message_id: "<draft@example.com>",
                    folder: "Drafts",
                    saved_at: 123,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-show-bcc-button"));
        await addRecipient("bcc", "hidden@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Old subject" },
        });
        await fillBody("Old body");
        await vi.advanceTimersByTimeAsync(900);
        await waitFor(() => expect(screen.getByText("草稿已保存")).toBeTruthy());

        component.show();
        await addRecipient("to", "new@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "New subject" },
        });
        await fillBody("New body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(screen.queryByText("hidden@example.com")).toBeNull();
        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                to: ["new@example.com"],
                bcc: [],
                draft_id: null,
            }),
        });
    });

    it("只有无效收件人 chip 时不会自动保存草稿", async () => {
        vi.useFakeTimers();
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "bad");
        await vi.advanceTimersByTimeAsync(900);

        expect(screen.getByText("bad")).toBeTruthy();
        expect(mockInvoke).not.toHaveBeenCalledWith(
            "save_draft",
            expect.anything(),
        );
    });

    it("关闭后重开会重新按账号优先级解析", async () => {
        const { component } = render(ComposeModal);

        component.show({ accountId: 1 });
        expect(
            selectValue(await screen.findByTestId("compose-account-select")),
        ).toBe("1");
        await fireEvent.click(screen.getByTestId("compose-close-button"));

        component.show();

        expect(
            selectValue(await screen.findByTestId("compose-account-select")),
        ).toBe("2");
    });

    it("回复和转发传入账号时用于默认发件账号", async () => {
        const { component } = render(ComposeModal);

        component.showReply("from@example.com", "Subject", "Body", 1);
        expect(
            selectValue(await screen.findByTestId("compose-account-select")),
        ).toBe("1");
        await fireEvent.click(screen.getByTestId("compose-close-button"));

        component.showForward("Subject", "Body", 1);
        expect(
            selectValue(await screen.findByTestId("compose-account-select")),
        ).toBe("1");
    });

    it("选择附件后展示附件并发送时包含附件", async () => {
        vi.mocked(openDialog).mockResolvedValue("/tmp/hello.txt");
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "describe_local_attachments") {
                return Promise.resolve([
                    {
                        path: "/tmp/hello.txt",
                        filename: "hello.txt",
                        content_type: "text/plain",
                        size: 5,
                    },
                ]);
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-attach-button"));

        expect(await screen.findByText("hello.txt")).toBeTruthy();

        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                attachments: [
                    {
                        path: "/tmp/hello.txt",
                        filename: "hello.txt",
                        content_type: "text/plain",
                        size: 5,
                    },
                ],
            }),
        });
    });

    it("重复选择同一路径附件只展示和发送一次", async () => {
        vi.mocked(openDialog).mockResolvedValue([
            "/tmp/hello.txt",
            "/tmp/hello.txt",
        ]);
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "describe_local_attachments") {
                return Promise.resolve([
                    {
                        path: "/tmp/hello.txt",
                        filename: "hello.txt",
                        content_type: "text/plain",
                        size: 5,
                    },
                    {
                        path: "/tmp/hello.txt",
                        filename: "hello.txt",
                        content_type: "text/plain",
                        size: 5,
                    },
                ]);
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.click(await screen.findByTestId("compose-attach-button"));

        expect(await screen.findAllByTestId("compose-attachment-row")).toHaveLength(
            1,
        );
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                attachments: [
                    expect.objectContaining({
                        path: "/tmp/hello.txt",
                    }),
                ],
            }),
        });
    });

    it("发送已保存草稿时携带 draft_id", async () => {
        vi.useFakeTimers();
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return Promise.resolve({
                    draft_id: 42,
                    message_id: "<draft@example.com>",
                    folder: "Drafts",
                    saved_at: 123,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");

        await vi.advanceTimersByTimeAsync(900);
        await waitFor(() => expect(screen.getByText("草稿已保存")).toBeTruthy());

        await fireEvent.click(screen.getByTestId("compose-send-button"));
        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ draft_id: 42 }),
        });
    });

    it("内容变更后立即发送会取消待触发草稿保存", async () => {
        vi.useFakeTimers();
        mockInvoke.mockImplementation(defaultInvoke);
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));
        await vi.advanceTimersByTimeAsync(900);

        expect(mockInvoke).not.toHaveBeenCalledWith(
            "save_draft",
            expect.anything(),
        );
        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ draft_id: null }),
        });
    });

    it("发送时等待在途草稿保存并携带最新 draft_id", async () => {
        vi.useFakeTimers();
        let resolveDraft!: (value: {
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }) => void;
        const draftPromise = new Promise<{
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }>((resolve) => {
            resolveDraft = resolve;
        });
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return draftPromise;
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await vi.advanceTimersByTimeAsync(900);

        const sendClick = fireEvent.click(screen.getByTestId("compose-send-button"));
        resolveDraft({
            draft_id: 77,
            message_id: "<draft@example.com>",
            folder: "Drafts",
            saved_at: 123,
            remote_saved: true,
            cleanup_error: null,
        });
        await sendClick;
        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith("send_email", {
                request: expect.objectContaining({ draft_id: 77 }),
            });
        });
    });

    it("在途草稿保存后编辑并立即发送会先保存最新内容", async () => {
        vi.useFakeTimers();
        let saveDraftCalls = 0;
        let resolveFirstDraft!: (value: {
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }) => void;
        const firstDraftPromise = new Promise<{
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }>((resolve) => {
            resolveFirstDraft = resolve;
        });
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                saveDraftCalls += 1;
                if (saveDraftCalls === 1) {
                    return firstDraftPromise;
                }
                return Promise.resolve({
                    draft_id: 88,
                    message_id: "<draft-latest@example.com>",
                    folder: "Drafts",
                    saved_at: 124,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Old subject" },
        });
        await fillBody("Body");
        await vi.advanceTimersByTimeAsync(900);

        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Latest subject" },
        });
        const sendClick = fireEvent.click(screen.getByTestId("compose-send-button"));
        resolveFirstDraft({
            draft_id: 77,
            message_id: "<draft-old@example.com>",
            folder: "Drafts",
            saved_at: 123,
            remote_saved: true,
            cleanup_error: null,
        });
        await sendClick;

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
                request: expect.objectContaining({
                    draft_id: 77,
                    subject: "Latest subject",
                }),
            });
            expect(mockInvoke).toHaveBeenCalledWith("send_email", {
                request: expect.objectContaining({ draft_id: 88 }),
            });
        });
    });

    it("等待在途草稿保存时重复点击发送不会重复发信", async () => {
        vi.useFakeTimers();
        let resolveDraft!: (value: {
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }) => void;
        const draftPromise = new Promise<{
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }>((resolve) => {
            resolveDraft = resolve;
        });
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return draftPromise;
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await vi.advanceTimersByTimeAsync(900);

        const sendButton = screen.getByTestId("compose-send-button");
        const firstClick = fireEvent.click(sendButton);
        await fireEvent.click(sendButton);
        resolveDraft({
            draft_id: 77,
            message_id: "<draft@example.com>",
            folder: "Drafts",
            saved_at: 123,
            remote_saved: true,
            cleanup_error: null,
        });
        await firstClick;

        await waitFor(() => {
            const sendCalls = mockInvoke.mock.calls.filter(
                ([cmd]) => cmd === "send_email",
            );
            expect(sendCalls).toHaveLength(1);
        });
    });

    it("在途草稿保存期间继续编辑会追加一次最新内容保存", async () => {
        vi.useFakeTimers();
        let saveDraftCalls = 0;
        let resolveFirstDraft!: (value: {
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }) => void;
        const firstDraftPromise = new Promise<{
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }>((resolve) => {
            resolveFirstDraft = resolve;
        });
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                saveDraftCalls += 1;
                if (saveDraftCalls === 1) {
                    return firstDraftPromise;
                }
                return Promise.resolve({
                    draft_id: 42,
                    message_id: "<draft@example.com>",
                    folder: "Drafts",
                    saved_at: 124,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await vi.advanceTimersByTimeAsync(900);

        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Latest subject" },
        });
        await vi.advanceTimersByTimeAsync(900);
        resolveFirstDraft({
            draft_id: 42,
            message_id: "<draft@example.com>",
            folder: "Drafts",
            saved_at: 123,
            remote_saved: true,
            cleanup_error: null,
        });

        await waitFor(() => {
            expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
                request: expect.objectContaining({
                    draft_id: 42,
                    subject: "Latest subject",
                }),
            });
        });
    });

    it("切换发件账号后不会携带旧账号保存的 draft_id", async () => {
        vi.useFakeTimers();
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return Promise.resolve({
                    draft_id: 42,
                    message_id: "<draft@example.com>",
                    folder: "Drafts",
                    saved_at: 123,
                    remote_saved: true,
                    cleanup_error: null,
                });
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "to@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await vi.advanceTimersByTimeAsync(900);
        await waitFor(() => expect(screen.getByText("草稿已保存")).toBeTruthy());

        await fireEvent.click(screen.getByTestId("compose-account-select"));
        await fireEvent.click(screen.getByTestId("compose-account-option-1"));
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({
                account_id: 1,
                draft_id: null,
            }),
        });
    });

    it("关闭后重开不会被旧会话在途草稿保存污染", async () => {
        vi.useFakeTimers();
        let resolveDraft!: (value: {
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }) => void;
        const draftPromise = new Promise<{
            draft_id: number;
            message_id: string;
            folder: string;
            saved_at: number;
            remote_saved: boolean;
            cleanup_error: null;
        }>((resolve) => {
            resolveDraft = resolve;
        });
        mockInvoke.mockImplementation((cmd, args) => {
            if (cmd === "save_draft") {
                return draftPromise;
            }
            return defaultInvoke(cmd, args);
        });
        const { component } = render(ComposeModal);

        component.show();
        await addRecipient("to", "old@example.com");
        await vi.advanceTimersByTimeAsync(900);
        await fireEvent.click(screen.getByTestId("compose-close-button"));

        component.show();
        resolveDraft({
            draft_id: 42,
            message_id: "<old-draft@example.com>",
            folder: "Drafts",
            saved_at: 123,
            remote_saved: true,
            cleanup_error: null,
        });
        await addRecipient("to", "new@example.com");
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
        await fillBody("Body");
        await fireEvent.click(screen.getByTestId("compose-send-button"));

        expect(mockInvoke).toHaveBeenCalledWith("send_email", {
            request: expect.objectContaining({ draft_id: null }),
        });
    });
});
