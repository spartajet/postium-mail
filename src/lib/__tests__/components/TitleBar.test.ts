/**
 * Postium Mail - 标题栏组件测试
 * TitleBar.test.ts
 *
 * 本文件测试标题栏组件的功能，包括窗口标题显示、
 * 关闭按钮行为（主窗口隐藏 vs 设置窗口关闭）等。
 *
 * ==================== 测试范围 ====================
 * 1. 默认显示主窗口标题并隐藏窗口
 * 2. 支持设置窗口标题和关闭策略
 */

import { fireEvent, render, screen } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TitleBar from "$lib/components/layout/TitleBar.svelte";

const mocks = vi.hoisted(() => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    isMaximized: vi.fn(),
    hide: vi.fn(),
    close: vi.fn(),
    onResized: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({
        minimize: mocks.minimize,
        toggleMaximize: mocks.toggleMaximize,
        isMaximized: mocks.isMaximized,
        hide: mocks.hide,
        close: mocks.close,
        onResized: mocks.onResized,
    }),
}));

describe("TitleBar", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        mocks.isMaximized.mockResolvedValue(false);
        mocks.onResized.mockResolvedValue(() => {});
    });

    it("默认显示主窗口标题并隐藏窗口", async () => {
        render(TitleBar);

        expect(screen.getByTestId("window-title").textContent).toBe(
            "Postium Mail",
        );

        await fireEvent.click(screen.getByRole("button", { name: "Close" }));

        expect(mocks.hide).toHaveBeenCalledOnce();
        expect(mocks.close).not.toHaveBeenCalled();
    });

    it("支持设置窗口标题和关闭策略", async () => {
        render(TitleBar, {
            props: {
                title: "Postium Mail Settings",
                closeBehavior: "close",
            },
        });

        expect(screen.getByTestId("window-title").textContent).toBe(
            "Postium Mail Settings",
        );

        await fireEvent.click(screen.getByRole("button", { name: "Close" }));

        expect(mocks.close).toHaveBeenCalledOnce();
        expect(mocks.hide).not.toHaveBeenCalled();
    });
});
