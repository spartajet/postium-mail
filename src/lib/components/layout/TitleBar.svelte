<script lang="ts">
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { Mail, Minus, Square, Copy, X } from "lucide-svelte";

    const appWindow = getCurrentWindow();
    let isMaximized = $state(false);

    async function minimize() {
        await appWindow.minimize();
    }

    async function toggleMaximize() {
        await appWindow.toggleMaximize();
        await updateState();
    }

    async function close() {
        await appWindow.hide();
    }

    async function updateState() {
        isMaximized = await appWindow.isMaximized();
    }

    function handleDragDblClick() {
        toggleMaximize();
    }

    $effect(() => {
        updateState();
        const unlisten = appWindow.onResized(async () => {
            await updateState();
        });
        return () => {
            unlisten.then((fn) => fn());
        };
    });
</script>

<div
    class="relative flex h-10 select-none items-center justify-between border-b border-border bg-elevated/80 backdrop-blur-md"
>
    <!-- Drag region with app icon and title -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="flex flex-1 items-center gap-2 pl-4"
        data-tauri-drag-region
        ondblclick={handleDragDblClick}
    >
        <!-- Email icon -->
        <Mail size={16} class="text-primary" />
        <span
            class="text-[13px] font-medium text-foreground"
            data-tauri-drag-region>Postium Mail</span
        >
    </div>

    <!-- Window controls -->
    <div class="flex h-full" style="-webkit-app-region: no-drag;">
        <button class="control-btn" onclick={minimize} aria-label="Minimize">
            <Minus size={12} />
        </button>
        <button
            class="control-btn"
            onclick={toggleMaximize}
            aria-label={isMaximized ? "Restore" : "Maximize"}
        >
            {#if isMaximized}
                <Copy size={12} />
            {:else}
                <Square size={12} />
            {/if}
        </button>
        <button
            class="control-btn close-btn"
            onclick={close}
            aria-label="Close"
        >
            <X size={12} />
        </button>
    </div>
</div>

<style>
    .control-btn {
        width: 46px;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        border: none;
        background: transparent;
        cursor: pointer;
        transition: background 150ms ease;
        color: var(--color-muted-foreground);
    }

    .control-btn:hover {
        background: var(--color-glass-hover);
        color: var(--color-foreground);
    }

    .close-btn:hover {
        background: #e81123 !important;
        color: white !important;
    }
</style>
