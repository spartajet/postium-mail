import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "../../../routes/settings/+page.svelte";

const mocks = vi.hoisted(() => ({
    goto: vi.fn(),
    setTheme: vi.fn(),
    setLocale: vi.fn(),
    loadAccounts: vi.fn(),
    deleteAccount: vi.fn(),
}));

let currentTheme: "light" | "dark" | "system" = "system";
let currentLocale: "zh-CN" | "en-US" = "zh-CN";

vi.mock("$app/navigation", () => ({
    goto: mocks.goto,
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
});
