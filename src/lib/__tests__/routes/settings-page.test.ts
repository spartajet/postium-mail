/**
 * Postium Mail - 设置页面测试
 * settings-page.test.ts
 *
 * 本文件测试设置页面的功能和交互，包括导航切换、
 * 主题和语言设置、账号管理等。
 *
 * ==================== 测试范围 ====================
 * 1. 设置页面默认显示通用分组
 * 2. 导航切换功能
 * 3. 主题和语言设置
 * 4. 账号管理面板
 * 5. 标题栏和关闭按钮
 */

import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "../../../routes/settings/+page.svelte";

const mocks = vi.hoisted(() => ({
    goto: vi.fn(),
    setTheme: vi.fn(),
    setLocale: vi.fn(),
    loadAccounts: vi.fn(),
    deleteAccount: vi.fn(),
    closeWindow: vi.fn(),
    minimizeWindow: vi.fn(),
    toggleMaximizeWindow: vi.fn(),
    isMaximized: vi.fn(),
    onResized: vi.fn(),
}));

let currentTheme: "light" | "dark" | "system" = "system";
let currentLocale: "zh-CN" | "en-US" = "zh-CN";

vi.mock("$app/navigation", () => ({
    goto: mocks.goto,
}));

vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({
        minimize: mocks.minimizeWindow,
        toggleMaximize: mocks.toggleMaximizeWindow,
        isMaximized: mocks.isMaximized,
        close: mocks.closeWindow,
        onResized: mocks.onResized,
    }),
}));

vi.mock("$lib/stores/i18n.svelte", () => ({
    getI18nState: () => ({
        get locale() {
            return currentLocale;
        },
        setLocale: mocks.setLocale,
        t: {
            settings: {
                title: "设置",
                general: "通用",
                generalTitle: "基础设置",
                generalDescription:
                    "管理应用级偏好设置。后续通用选项会放在这里。",
                accounts: "账号管理",
                appearance: "外观",
                language: "语言",
                theme: "主题",
                light: "浅色",
                dark: "深色",
                system: "跟随系统",
            },
            account: {
                delete: "删除",
            },
        },
    }),
}));

vi.mock("$lib/stores/theme.svelte", () => ({
    getThemeState: () => ({
        get theme() {
            return currentTheme;
        },
        setTheme: mocks.setTheme,
    }),
}));

vi.mock("$lib/stores/account.svelte", () => ({
    getAccountState: () => ({
        accounts: [],
        error: null,
        loadAccounts: mocks.loadAccounts,
        deleteAccount: mocks.deleteAccount,
    }),
}));

describe("Settings page layout", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        currentTheme = "system";
        currentLocale = "zh-CN";
        mocks.isMaximized.mockResolvedValue(false);
        mocks.onResized.mockResolvedValue(() => {});
    });

    it("默认显示通用分组", async () => {
        render(SettingsPage);

        await waitFor(() => {
            expect(mocks.loadAccounts).toHaveBeenCalledOnce();
        });

        const settingsPage = screen.getByTestId("settings-page");
        const generalNav = screen.getByTestId("settings-nav-general");
        const generalPanel = screen.getByTestId("settings-panel-general");

        expect(settingsPage.isConnected).toBe(true);
        expect(generalNav.tagName).toBe("BUTTON");
        expect(generalNav.getAttribute("aria-current")).toBe("page");
        expect(generalPanel.isConnected).toBe(true);
    });

    it("点击外观导航后显示主题和语言设置", async () => {
        render(SettingsPage);

        const appearanceNav = screen.getByTestId("settings-nav-appearance");

        expect(appearanceNav.tagName).toBe("BUTTON");

        await fireEvent.click(appearanceNav);

        expect(screen.getByTestId("theme-light").isConnected).toBe(true);
        expect(screen.getByTestId("theme-dark").isConnected).toBe(true);
        expect(screen.getByTestId("theme-system").isConnected).toBe(true);
    });

    it("点击账号管理导航后显示账号管理面板", async () => {
        render(SettingsPage);

        const accountsNav = screen.getByTestId("settings-nav-accounts");

        expect(accountsNav.tagName).toBe("BUTTON");

        await fireEvent.click(accountsNav);

        expect(screen.getByTestId("settings-accounts-panel").isConnected).toBe(
            true,
        );
    });

    it("显示和主窗口一致的自定义标题栏", () => {
        render(SettingsPage);

        expect(screen.getByTestId("window-title").textContent).toBe(
            "Postium Mail Settings",
        );
        expect(
            screen.getByRole("button", { name: "Minimize" }).isConnected,
        ).toBe(true);
        expect(
            screen.getByRole("button", { name: "Maximize" }).isConnected,
        ).toBe(true);
        expect(screen.getByRole("button", { name: "Close" }).isConnected).toBe(
            true,
        );
        expect(screen.queryByTestId("settings-close-button")).toBeNull();
    });

    it("点击标题栏关闭按钮时关闭当前设置窗口", async () => {
        mocks.closeWindow.mockResolvedValue(undefined);

        render(SettingsPage);

        await fireEvent.click(screen.getByRole("button", { name: "Close" }));

        expect(mocks.closeWindow).toHaveBeenCalledOnce();
        expect(mocks.goto).not.toHaveBeenCalled();
    });

    it("标题栏关闭当前设置窗口失败时回退到主页面", async () => {
        mocks.closeWindow.mockRejectedValue(new Error("not in tauri"));

        render(SettingsPage);

        await fireEvent.click(screen.getByRole("button", { name: "Close" }));

        expect(mocks.closeWindow).toHaveBeenCalledOnce();
        expect(mocks.goto).toHaveBeenCalledWith("/");
    });
});
