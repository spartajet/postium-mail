<script lang="ts">
    import { getToastState } from "$lib/stores/toast.svelte";
    import type { ToastType } from "$lib/stores/toast.svelte";

    let toastState = getToastState();
    let seen = new Set<number>();

    $effect(() => {
        for (const t of toastState.toasts) {
            if (!seen.has(t.id)) {
                seen.add(t.id);
                const id = t.id;
                setTimeout(() => {
                    seen.delete(id);
                    toastState.dismiss(id);
                }, t.duration);
            }
        }
    });

    function getColor(type: ToastType): string {
        if (type === "success")
            return "bg-success/15 border-success/30 text-success";
        if (type === "error")
            return "bg-destructive/15 border-destructive/30 text-destructive";
        if (type === "info") return "bg-info/15 border-info/30 text-info";
        return "bg-warning/15 border-warning/30 text-warning";
    }

    function getIcon(type: ToastType): string {
        if (type === "success") return "\u2713";
        if (type === "error") return "\u2717";
        if (type === "info") return "\u2139";
        return "\u26A0";
    }

    function dismiss(id: number) {
        seen.delete(id);
        toastState.dismiss(id);
    }
</script>

{#if toastState.toasts.length > 0}
    <div class="fixed bottom-4 right-4 z-9999 flex flex-col gap-2">
        {#each toastState.toasts as t (t.id)}
            <div
                class="flex items-center gap-2 rounded-lg border px-4 py-3 shadow-lg backdrop-blur-sm animate-slide-in {getColor(
                    t.type,
                )}"
                role="alert"
            >
                <span class="text-lg">{getIcon(t.type)}</span>
                <span class="text-sm flex-1">{t.message}</span>
                <button
                    class="ml-2 rounded p-1 opacity-60 hover:opacity-100 transition-opacity"
                    onclick={() => dismiss(t.id)}
                    aria-label="close"
                >
                    &times;
                </button>
            </div>
        {/each}
    </div>
{/if}

<style>
    .animate-slide-in {
        animation: slideIn 0.2s ease-out;
    }

    @keyframes slideIn {
        from {
            transform: translateX(100%);
            opacity: 0;
        }
        to {
            transform: translateX(0);
            opacity: 1;
        }
    }
</style>
