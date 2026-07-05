<script lang="ts">
    import { onMount, setContext } from "svelte";
    import { listen } from "@tauri-apps/api/event";

    import ComposeModal from "$lib/components/email/ComposeModal.svelte";
    import AddAccountModal from "$lib/components/settings/AddAccountModal.svelte";
    import Sidebar from "$lib/components/layout/Sidebar.svelte";
    import StatusBar from "$lib/components/layout/StatusBar.svelte";
    import TitleBar from "$lib/components/layout/TitleBar.svelte";
    import { getAccountState } from "$lib/stores/account.svelte";
    import { getSyncState } from "$lib/stores/sync.svelte";

    let { children } = $props();

    const account = getAccountState();
    const sync = getSyncState();

    let composeModal = $state<ComposeModal>();
    let addAccountModal = $state<AddAccountModal>();

    const COMPOSE_MODAL_KEY = Symbol.for("compose-modal");
    const ADD_ACCOUNT_MODAL_KEY = Symbol.for("add-account-modal");

    setContext(COMPOSE_MODAL_KEY, () => composeModal);
    setContext(ADD_ACCOUNT_MODAL_KEY, () => addAccountModal);

    onMount(() => {
        const unlisten = listen<string>("tray-action", (event) => {
            if (event.payload === "compose") {
                composeModal?.show();
            } else if (event.payload === "sync" && account.activeAccountId) {
                sync.syncAccount(account.activeAccountId);
            }
        });

        return () => {
            unlisten.then((fn) => fn());
        };
    });
</script>

<div
    class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground"
>
    <div class="bg-orbs">
        <div class="orb orb-1"></div>
        <div class="orb orb-2"></div>
        <div class="orb orb-3"></div>
    </div>

    <TitleBar />

    <main class="relative flex flex-1 overflow-hidden">
        <Sidebar />

        <div class="flex-1 overflow-hidden">
            {@render children()}
        </div>
    </main>

    <StatusBar />

    <ComposeModal bind:this={composeModal} />
    <AddAccountModal bind:this={addAccountModal} />
</div>
