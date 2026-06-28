import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import EmailContextMenu from "$lib/components/email/EmailContextMenu.svelte";

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        t: {
            email: {
                reply: "回复",
                replyAll: "全部回复",
                forward: "转发",
                star: "星标",
                unstar: "取消星标",
                markRead: "标记已读",
                markUnread: "标记未读",
                reload: "重新加载",
                delete: "删除",
            },
            common: {
                operations: "更多操作",
            },
        },
    }),
}));

describe("EmailContextMenu", () => {
    it("点击重新加载时调用 onReload", async () => {
        const onReload = vi.fn();
        const onClose = vi.fn();

        render(EmailContextMenu, {
            props: {
                x: 0,
                y: 0,
                emailId: 42,
                isRead: false,
                isStarred: false,
                onToggleStar: vi.fn(),
                onToggleRead: vi.fn(),
                onDelete: vi.fn(),
                onForward: vi.fn(),
                onReload,
                onClose,
            },
        });

        await fireEvent.click(screen.getByRole("button", { name: "重新加载" }));

        expect(onReload).toHaveBeenCalledWith(42);
        expect(onClose).toHaveBeenCalledOnce();
    });
});
