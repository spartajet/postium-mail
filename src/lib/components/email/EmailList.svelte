<script lang="ts">
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import { getEmailState } from "$lib/stores/email.svelte";
    import { getAccountState } from "$lib/stores/account.svelte";
    import { commands } from "$lib/bindings";
    import type { SearchResult } from "$lib/bindings";
    import {
        Search,
        RefreshCw,
        X,
        List,
        LayoutGrid,
        Mail,
        Star,
    } from "lucide-svelte";

    const i18n = getI18nState();
    const t = $derived(i18n.t);
    const emailState = getEmailState();
    const accountStore = getAccountState();

    let searchQuery = $state("");
    let searchResults = $state<SearchResult[] | null>(null);
    let searching = $state(false);

    // 自动加载邮件：当活跃账号或文件夹变化时触发
    $effect(() => {
        const accountId = accountStore.activeAccountId;
        const folder = emailState.currentFolder;
        if (accountId) {
            emailState.loadEmails(accountId, folder);
        }
    });

    // 搜索防抖 - H-02: 正确清理 timeout
    $effect(() => {
        const q = searchQuery;

        if (!q.trim()) {
            searchResults = null;
            searching = false;
            return;
        }

        searching = true;
        const timeout = setTimeout(async () => {
            try {
                const accountId = accountStore.activeAccountId;
                const result = await commands.searchEmails(q, accountId, 50);
                if (result.status === "ok") {
                    searchResults = result.data;
                }
            } catch (e) {
                console.error("Search failed:", e);
            } finally {
                searching = false;
            }
        }, 300);

        return () => clearTimeout(timeout);
    });

    function clearSearch() {
        searchQuery = "";
        searchResults = null;
    }

    function formatDate(timestamp: number): string {
        const date = new Date(timestamp * 1000);
        const now = new Date();
        const diffMs = now.getTime() - date.getTime();
        const diffMins = Math.floor(diffMs / 60000);
        const diffHours = Math.floor(diffMs / 3600000);
        const diffDays = Math.floor(diffMs / 86400000);

        if (diffMins < 1) return "now";
        if (diffMins < 60) return `${diffMins}m`;
        if (diffHours < 24) return `${diffHours}h`;
        if (diffDays < 7) return `${diffDays}d`;
        return date.toLocaleDateString(undefined, {
            month: "short",
            day: "numeric",
        });
    }
</script>

<div
    class="list-panel flex h-full w-95 shrink-0 flex-col border-r border-border bg-elevated/80"
>
    <!-- Search bar + toolbar -->
    <div class="border-b border-border px-4 py-3">
        <!-- Search input -->
        <div
            class="search-container flex items-center gap-2 rounded-lg border border-border bg-glass px-3 py-2 transition-colors focus-within:border-primary focus-within:ring-2 focus-within:ring-primary/20"
        >
            <Search size={18} class="shrink-0 text-muted-foreground" />
            <input
                type="text"
                placeholder={t.email.search}
                bind:value={searchQuery}
                class="flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
            />
            {#if searching}
                <RefreshCw
                    size={16}
                    class="animate-spin text-muted-foreground"
                />
            {:else if searchQuery}
                <button
                    class="text-muted-foreground hover:text-foreground"
                    onclick={clearSearch}
                >
                    <X size={16} />
                </button>
            {/if}
            <kbd
                class="rounded bg-glass-active px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
                >Ctrl+K</kbd
            >
        </div>

        <!-- Toolbar -->
        <div class="mt-3 flex items-center gap-1">
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                title={t.sidebar.sync}
            >
                <RefreshCw size={18} />
            </button>
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            >
                <List size={18} />
            </button>
            <button
                class="icon-btn-sm flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            >
                <LayoutGrid size={18} />
            </button>
            <span class="ml-auto text-xs text-muted-foreground">
                {searchResults !== null
                    ? searchResults.length
                    : emailState.emails.length}
            </span>
        </div>
    </div>

    <!-- Email list -->
    <div class="flex-1 overflow-y-auto">
        {#if searchResults !== null}
            <!-- Search results mode -->
            {#if searchResults.length === 0}
                <div
                    class="flex h-full flex-col items-center justify-center py-12 text-muted-foreground"
                >
                    <Search size={48} class="mb-4 opacity-30" />
                    <p class="text-sm">{t.email.noEmails}</p>
                </div>
            {:else}
                {#each searchResults as result (result.id)}
                    <button
                        class="email-item group relative flex w-full flex-col border-b border-border px-5 py-3 text-left transition-colors {emailState.selectedEmailId ===
                        result.id
                            ? 'active'
                            : ''}"
                        onclick={() => emailState.selectEmail(result.id)}
                    >
                        <div class="flex items-center justify-between gap-2">
                            <span
                                class="truncate text-sm font-medium text-foreground"
                            >
                                {result.sender_email}
                            </span>
                            <span
                                class="shrink-0 text-xs text-muted-foreground"
                            >
                                {formatDate(result.sent_at)}
                            </span>
                        </div>
                        <span
                            class="mt-0.5 truncate text-sm text-muted-foreground"
                        >
                            {result.subject || "(No Subject)"}
                        </span>
                        {#if result.preview}
                            <p
                                class="mt-0.5 truncate text-xs text-muted-foreground"
                            >
                                {result.preview}
                            </p>
                        {/if}
                    </button>
                {/each}
            {/if}
        {:else if emailState.loading}
            <div class="space-y-3 p-4">
                <div class="skeleton skeleton-text" style="width: 80%"></div>
                <div class="skeleton skeleton-text" style="width: 60%"></div>
                <div class="skeleton skeleton-text" style="width: 90%"></div>
                <div class="skeleton skeleton-text" style="width: 70%"></div>
                <div class="skeleton skeleton-text" style="width: 50%"></div>
                <div class="skeleton skeleton-text" style="width: 85%"></div>
            </div>
        {:else if emailState.emails.length === 0}
            <div
                class="flex h-full flex-col items-center justify-center py-12 text-muted-foreground"
            >
                <Mail size={48} class="mb-4 opacity-30" strokeWidth={1} />
                <h3 class="text-lg text-secondary-foreground/70">
                    {t.email.noEmails}
                </h3>
                <p class="text-sm">{t.email.noEmails}</p>
            </div>
        {:else}
            {#each emailState.emails as email (email.id)}
                <button
                    class="email-item group relative flex w-full flex-col border-b border-border px-5 py-3 text-left transition-colors {emailState.selectedEmailId ===
                    email.id
                        ? 'active'
                        : ''}"
                    onclick={() => emailState.selectEmail(email.id)}
                >
                    <!-- Unread dot -->
                    {#if !email.is_read}
                        <div
                            class="unread-dot absolute left-2 top-1/2 h-1.5 w-1.5 -translate-y-1/2 rounded-full bg-primary"
                        ></div>
                    {/if}

                    <div class="flex items-center justify-between gap-2">
                        <span
                            class="truncate text-sm {email.is_read
                                ? 'text-muted-foreground'
                                : 'font-semibold text-foreground'}"
                        >
                            {email.sender_name || email.sender_email}
                        </span>
                        <span class="shrink-0 text-xs text-muted-foreground">
                            {formatDate(email.sent_at)}
                        </span>
                    </div>
                    <div class="mt-0.5 flex items-center gap-2">
                        {#if email.is_starred}
                            <Star
                                size={14}
                                class="shrink-0 fill-yellow-400 text-yellow-400"
                            />
                        {/if}
                        <span
                            class="truncate text-sm {email.is_read
                                ? 'text-muted-foreground'
                                : 'font-medium text-foreground'}"
                        >
                            {email.subject || "(No Subject)"}
                        </span>
                    </div>
                    {#if email.preview}
                        <p
                            class="mt-0.5 truncate text-xs text-muted-foreground"
                        >
                            {email.preview}
                        </p>
                    {/if}
                </button>
            {/each}
        {/if}
    </div>
</div>

<style>
    .email-item {
        cursor: pointer;
        transition: background 150ms ease;
    }

    .email-item:hover {
        background: var(--color-glass-hover);
    }

    .email-item.active {
        background: color-mix(in srgb, var(--color-primary) 15%, transparent);
        border-left: 3px solid var(--color-primary);
        padding-left: calc(1.25rem - 3px);
    }
</style>
