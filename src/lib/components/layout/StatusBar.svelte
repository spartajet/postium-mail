<script lang="ts">
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import { getThemeState } from "$lib/stores/theme.svelte";
    import { getSyncState } from "$lib/stores/sync.svelte";
    import { getAccountState } from "$lib/stores/account.svelte";
    import {
        RefreshCw,
        CircleX,
        CircleCheck,
        Globe,
        Sun,
        Moon,
    } from "lucide-svelte";

    const i18n = getI18nState();
    const t = $derived(i18n.t);
    const themeState = getThemeState();
    const syncStore = getSyncState();
    const accountStore = getAccountState();

    const syncStatus = $derived<"idle" | "syncing" | "error">(
        syncStore.syncing ? "syncing" : syncStore.error ? "error" : "idle",
    );

    function getAccountEmail(accountId: number): string {
        return (
            accountStore.accounts.find((a) => a.id === accountId)?.email ?? ""
        );
    }

    function getProgressText(): string {
        if (!syncStore.progress) return "";
        const p = syncStore.progress;
        const email = getAccountEmail(p.account_id);
        const prefix = email ? `${email} - ` : "";

        switch (p.stage) {
            case "Connecting":
                return `${prefix}${t.sync.connecting}`;
            case "SyncingFolders":
                return `${prefix}${t.sync.syncingFolders}`;
            case "SyncingEmails": {
                const folder = p.folder ?? "";
                const progress =
                    p.total > 0 ? ` (${p.current}/${p.total})` : "";
                return `${prefix}${t.sync.syncingEmails} ${folder}${progress}`;
            }
            case "Completed":
                return p.message || t.sync.completed;
            case "Error":
                return p.message;
            default:
                return "";
        }
    }

    const currentTime = $derived(
        new Date().toLocaleDateString(undefined, {
            weekday: "short",
            month: "short",
            day: "numeric",
        }),
    );

    function toggleTheme() {
        themeState.setTheme(themeState.resolved === "dark" ? "light" : "dark");
    }

    function toggleLocale() {
        i18n.setLocale(i18n.locale === "zh-CN" ? "en-US" : "zh-CN");
    }
</script>

<div
    class="status-bar relative flex h-8 items-center justify-between border-t border-border bg-elevated/80 px-6 text-xs text-muted-foreground backdrop-blur-md"
>
    <div class="flex items-center gap-6">
        {#if syncStatus === "syncing"}
            <div class="flex items-center gap-1.5">
                <RefreshCw size={14} class="animate-spin text-primary" />
                <span>{getProgressText() || t.sync.syncing}</span>
            </div>
        {:else if syncStatus === "error"}
            <div class="flex items-center gap-1.5">
                <CircleX size={14} class="text-destructive" />
                <span class="text-destructive"
                    >{syncStore.error || t.sync.failed}</span
                >
            </div>
        {:else}
            <div class="flex items-center gap-1.5">
                <CircleCheck size={14} class="text-success" />
                <span>{syncStore.progress?.message || t.sync.completed}</span>
            </div>
        {/if}
    </div>

    <div class="flex items-center gap-5">
        <!-- Date -->
        <span>{currentTime}</span>

        <!-- Language toggle -->
        <button
            class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-glass-hover hover:text-foreground"
            onclick={toggleLocale}
            title="Toggle language"
        >
            <Globe size={14} />
            <span>{i18n.locale === "zh-CN" ? "EN" : "中"}</span>
        </button>

        <!-- Theme toggle -->
        <button
            class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-glass-hover hover:text-foreground"
            onclick={toggleTheme}
            title="Toggle theme"
        >
            {#if themeState.resolved === "dark"}
                <Sun size={14} />
            {:else}
                <Moon size={14} />
            {/if}
        </button>

        <span class="text-[11px]">v0.1.0</span>
    </div>
</div>
