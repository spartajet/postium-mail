import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ComposeModal from "$lib/components/email/ComposeModal.svelte";
import { mockInvoke } from "../mocks/tauri";

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
    return (element as HTMLSelectElement).value;
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
                subject: "主题",
                send: "发送",
                loading: "发送中",
            },
            common: { cancel: "取消" },
        },
    }),
}));

describe("ComposeModal 发件账号选择", () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue({ status: "ok", data: "message-id" });
        accounts = [...defaultAccounts];
        isAllAccounts = true;
        activeAccountId = null;
        lastConcreteAccountId = 2;
    });

    it("所有账号视图下显示发件账号选择器并默认上次具体账号", async () => {
        const { component } = render(ComposeModal);

        component.show();

        const select = await screen.findByTestId("compose-account-select");
        expect(selectValue(select)).toBe("2");
    });

    it("所有账号视图下发送使用选择的发件账号", async () => {
        const { component } = render(ComposeModal);

        component.show();
        await fireEvent.change(await screen.findByTestId("compose-account-select"), {
            target: { value: "1" },
        });
        await fireEvent.input(screen.getByTestId("compose-to-input"), {
            target: { value: "to@example.com" },
        });
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
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

        expect(await screen.findByTestId("compose-to-input")).toBeTruthy();
        expect(screen.queryByTestId("compose-account-select")).toBeNull();
        await fireEvent.input(screen.getByTestId("compose-to-input"), {
            target: { value: "to@example.com" },
        });
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });
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
        await fireEvent.input(await screen.findByTestId("compose-to-input"), {
            target: { value: "to@example.com" },
        });
        await fireEvent.input(screen.getByTestId("compose-subject-input"), {
            target: { value: "Hello" },
        });

        const sendButton = screen.getByTestId("compose-send-button");
        expect((sendButton as HTMLButtonElement).disabled).toBe(true);
        await fireEvent.click(sendButton);
        expect(mockInvoke).not.toHaveBeenCalled();
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
});
