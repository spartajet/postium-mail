<script lang="ts">
    import { getContext, onMount } from "svelte";
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import { getAccountState } from "$lib/stores/account.svelte";
    import { getEmailState } from "$lib/stores/email.svelte";
    import type { EmailCategory } from "$lib/bindings";
    import { getSyncState } from "$lib/stores/sync.svelte";
    import type ComposeModal from "$lib/components/email/ComposeModal.svelte";
    import type AddAccountModal from "$lib/components/settings/AddAccountModal.svelte";
    import {
        Inbox,
        Star,
        Send,
        FileText,
        AlertCircle,
        Trash2,
        Plus,
        RefreshCw,
        Settings,
        ChevronDown,
        Calendar,
        Workflow,
    } from "lucide-svelte";
    import { goto } from "$app/navigation";

    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal");
    const ADD_ACCOUNT_MODAL_KEY = Symbol.for("add-account-modal");
    const getComposeModal =
        getContext<() => ComposeModal | undefined>(COMPOSE_MODAL_KEY);
    const getAddAccountModal = getContext<() => AddAccountModal | undefined>(
        ADD_ACCOUNT_MODAL_KEY,
    );

    const i18n = getI18nState();
    const t = $derived(i18n.t);
    const accountStore = getAccountState();
    const emailStore = getEmailState();
    const syncStore = getSyncState();

    let activeFolder = $state("inbox");
    let showAccountDropdown = $state(false);

    onMount(() => {
        accountStore.loadAccounts();
    });

    function selectFolder(folderId: EmailCategory) {
        activeFolder = folderId;
        emailStore.currentFolder = folderId;
        if (accountStore.activeAccountId) {
            emailStore.loadEmailsByCategory(
                accountStore.activeAccountId,
                folderId,
            );
        }
        goto("/");
    }

    async function handleSync() {
        if (accountStore.activeAccountId) {
            await syncStore.syncAccount(accountStore.activeAccountId);
            await emailStore.loadEmailsByCategory(
                accountStore.activeAccountId,
                emailStore.currentFolder,
            );
        }
    }

    const folders = $derived([
        { id: "inbox" as EmailCategory, label: t.sidebar.inbox, icon: Inbox },
        { id: "starred" as EmailCategory, label: t.sidebar.starred, icon: Star },
        { id: "sent" as EmailCategory, label: t.sidebar.sent, icon: Send },
        { id: "drafts" as EmailCategory, label: t.sidebar.drafts, icon: FileText },
        { id: "spam" as EmailCategory, label: t.sidebar.spam, icon: AlertCircle },
        { id: "trash" as EmailCategory, label: t.sidebar.trash, icon: Trash2 },
    ]);

    const labels = $derived([
        { id: "urgent", name: t.sidebar.labelUrgent, color: "#EF4444" },
        { id: "work", name: t.sidebar.labelWork, color: "#3B82F6" },
        { id: "personal", name: t.sidebar.labelPersonal, color: "#10B981" },
        { id: "finance", name: t.sidebar.labelFinance, color: "#F59E0B" },
    ]);
</script>

<aside
    class="flex h-full w-65 shrink-0 flex-col border-r border-border bg-glass backdrop-blur-md"
>
    <!-- Header: Logo + Sync -->
    <div
        class="flex items-center justify-between border-b border-border px-4 py-3"
    >
        <div class="flex items-center gap-2">
            <div
                class="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-xs font-bold text-primary-foreground"
            >
                P
            </div>
            <span class="text-sm font-bold text-foreground">Postium</span>
        </div>
        <button
            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
            title={t.sidebar.sync}
            onclick={handleSync}
            disabled={syncStore.syncing}
        >
            <RefreshCw
                size={16}
                class={syncStore.syncing ? "animate-spin" : ""}
            />
        </button>
    </div>

    <!-- Account Selector -->
    <div class="relative px-3">
        <button
            class="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors hover:bg-glass-hover"
            onclick={() => (showAccountDropdown = !showAccountDropdown)}
        >
            <div
                class="flex h-6 w-6 items-center justify-center rounded-full bg-primary/15 text-[10px] font-bold text-primary"
            >
                {accountStore.activeAccount?.email?.charAt(0)?.toUpperCase() ||
                    "?"}
            </div>
            <span class="flex-1 truncate text-muted-foreground"
                >{accountStore.activeAccount?.email ||
                    t.sidebar.allAccounts}</span
            >
            <ChevronDown size={14} class="shrink-0 text-muted-foreground" />
        </button>
        {#if showAccountDropdown}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
                class="absolute left-3 right-3 top-full z-50 mt-1 rounded-lg border border-border bg-card shadow-lg"
                onclick={(e) => e.stopPropagation()}
            >
                {#each accountStore.accounts as account}
                    <button
                        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors hover:bg-glass-hover {account.id ===
                        accountStore.activeAccountId
                            ? 'bg-primary/10 text-primary'
                            : 'text-foreground'}"
                        onclick={() => {
                            accountStore.setActive(account.id);
                            showAccountDropdown = false;
                        }}
                    >
                        <div
                            class="flex h-5 w-5 items-center justify-center rounded-full bg-primary/15 text-[9px] font-bold text-primary"
                        >
                            {account.email.charAt(0).toUpperCase()}
                        </div>
                        <span class="truncate">{account.email}</span>
                    </button>
                {/each}
                <div class="border-t border-border">
                    <button
                        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-primary transition-colors hover:bg-glass-hover"
                        onclick={() => {
                            getAddAccountModal?.()?.show();
                            showAccountDropdown = false;
                        }}
                    >
                        <Plus size={14} />
                        {t.account.add}
                    </button>
                </div>
            </div>
        {/if}
    </div>

    <!-- Compose Button -->
    <div class="px-4 py-3">
        <button
            class="compose-btn flex w-full items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium text-white transition-all"
            onclick={() => getComposeModal?.()?.show()}
        >
            <Plus size={18} />
            {t.sidebar.compose}
        </button>
    </div>

    <!-- Folder Nav -->
    <nav class="flex-1 overflow-y-auto px-3 py-1">
        <div class="mb-3">
            {#each folders as folder}
                <button
                    class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm transition-colors {activeFolder ===
                    folder.id
                        ? 'active'
                        : ''}"
                    onclick={() => selectFolder(folder.id)}
                >
                    <folder.icon size={18} class="shrink-0" />
                    <span class="flex-1">{folder.label}</span>
                </button>
            {/each}
        </div>

        <!-- Separator -->
        <div class="mx-2 my-3 h-px bg-border"></div>

        <!-- Labels section -->
        <div>
            <div
                class="px-3 py-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground"
            >
                {t.sidebar.labels}
            </div>
            {#each labels as label}
                <button
                    class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-1.5 text-left text-sm text-muted-foreground transition-colors"
                >
                    <span
                        class="h-2 w-2 shrink-0 rounded-full"
                        style="background: {label.color}"
                    ></span>
                    <span>{label.name}</span>
                </button>
            {/each}
        </div>

        <!-- Separator -->
        <div class="mx-2 my-3 h-px bg-border"></div>

        <!-- Calendar & Workflow -->
        <div class="mb-1">
            <button
                class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
                onclick={() => {
                    activeFolder = "";
                    goto("/calendar");
                }}
            >
                <Calendar size={18} class="shrink-0" />
                <span class="flex-1">{t.sidebar.calendar}</span>
            </button>
            <button
                class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
                onclick={() => {
                    activeFolder = "";
                    goto("/workflow");
                }}
            >
                <Workflow size={18} class="shrink-0" />
                <span class="flex-1">{t.sidebar.workflow}</span>
            </button>
        </div>
    </nav>

    <!-- Settings -->
    <div class="border-t border-border px-3 py-2">
        <button
            class="nav-item flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm text-muted-foreground transition-colors"
            onclick={() => {
                activeFolder = "";
                goto("/settings");
            }}
        >
            <Settings size={18} class="shrink-0" />
            {t.sidebar.settings}
        </button>
    </div>

    <!-- Storage Footer -->
    <div class="border-t border-border p-4">
        <div class="mb-2 flex items-center justify-between">
            <span class="text-xs text-muted-foreground">Storage</span>
            <span class="text-xs text-muted-foreground">2.4 GB / 15 GB</span>
        </div>
        <div class="h-1 overflow-hidden rounded-full bg-glass-active">
            <div class="storage-fill h-full w-[16%] rounded-full"></div>
        </div>
    </div>
</aside>

<style>
    .compose-btn {
        background: linear-gradient(
            135deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
        box-shadow: 0 4px 16px rgba(124, 58, 237, 0.3);
    }

    .compose-btn:hover {
        transform: translateY(-1px);
        box-shadow: 0 6px 20px rgba(124, 58, 237, 0.4);
    }

    .compose-btn:active {
        transform: translateY(0);
    }

    .nav-item:hover {
        background: var(--color-glass-hover);
        color: var(--color-foreground);
    }

    .nav-item.active {
        background: color-mix(in srgb, var(--color-primary) 15%, transparent);
        color: var(--color-primary);
    }

    .storage-fill {
        background: linear-gradient(
            90deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
        transition: width 300ms ease;
    }
</style>
