<script lang="ts">
    import { getAccountState } from "$lib/stores/account.svelte";
    import { getI18nState } from "$lib/stores/i18n.svelte";
    import { commands } from "$lib/bindings";
    import { X } from "lucide-svelte";
    import RichTextEditor from "./RichTextEditor.svelte";

    const accountStore = getAccountState();
    const i18n = getI18nState();
    const t = $derived(i18n.t);

    let open = $state(false);
    let to = $state("");
    let cc = $state("");
    let subject = $state("");
    let sending = $state(false);
    let error = $state("");
    let richEditor = $state<RichTextEditor>();

    async function handleSend() {
        if (!accountStore.activeAccountId) return;
        sending = true;
        error = "";
        try {
            const result = await commands.sendEmail({
                account_id: accountStore.activeAccountId,
                to: to
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean),
                cc: cc
                    ? cc
                          .split(",")
                          .map((s) => s.trim())
                          .filter(Boolean)
                    : [],
                bcc: [],
                subject,
                body_html:
                    richEditor?.getHtml() ||
                    `<pre style="white-space:pre-wrap">${richEditor?.getText() || ""}</pre>`,
                body_text: richEditor?.getText() || "",
            });
            if (result.status === "error") {
                error = result.error.message as string;
            } else {
                close();
            }
        } catch (e: any) {
            error = e.toString();
        } finally {
            sending = false;
        }
    }

    function close() {
        open = false;
        to = "";
        cc = "";
        subject = "";
        error = "";
        richEditor?.clear();
    }

    export function show() {
        open = true;
    }

    export function showReply(
        replyTo: string,
        replySubject: string,
        replyBody: string,
    ) {
        open = true;
        to = replyTo;
        subject = `Re: ${replySubject.replace(/^(Re|Fwd):\s*/i, "")}`;
        richEditor?.setContent(`<p>${replyBody}</p>`);
    }

    export function showForward(fwdSubject: string, fwdBody: string) {
        open = true;
        subject = `Fwd: ${fwdSubject.replace(/^(Re|Fwd):\s*/i, "")}`;
        richEditor?.setContent(`<p>${fwdBody}</p>`);
    }
</script>

{#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
        class="fixed inset-0 z-50 flex items-end justify-end p-6"
        onclick={(e) => e.target === e.currentTarget && close()}
    >
        <div
            class="flex h-[70vh] w-140 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl backdrop-blur-md"
        >
            <!-- Header -->
            <div
                class="flex items-center justify-between border-b border-border px-4 py-3"
            >
                <h3 class="text-sm font-semibold text-foreground">
                    {t.sidebar.compose}
                </h3>
                <button
                    class="rounded-md p-1 text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                    aria-label="Close"
                >
                    <X size={16} />
                </button>
            </div>

            <!-- Fields -->
            <div class="border-b border-border">
                <div class="flex items-center border-b border-border px-4">
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.to}</span
                    >
                    <input
                        type="text"
                        bind:value={to}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                        placeholder="email@example.com"
                    />
                </div>
                <div class="flex items-center border-b border-border px-4">
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.cc}</span
                    >
                    <input
                        type="text"
                        bind:value={cc}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                    />
                </div>
                <div class="flex items-center px-4">
                    <span class="w-14 shrink-0 text-sm text-muted-foreground"
                        >{t.email.subject}</span
                    >
                    <input
                        type="text"
                        bind:value={subject}
                        class="flex-1 bg-transparent py-2 text-sm text-foreground outline-none"
                    />
                </div>
            </div>

            <!-- Body -->
            <div class="flex-1 overflow-hidden">
                <RichTextEditor bind:this={richEditor} />
            </div>

            <!-- Error -->
            {#if error}
                <div class="px-4 py-2 text-sm text-destructive">{error}</div>
            {/if}

            <!-- Footer -->
            <div
                class="flex items-center justify-between border-t border-border px-4 py-3"
            >
                <button
                    class="compose-btn rounded-lg px-5 py-2 text-sm font-medium text-white transition-all disabled:opacity-50"
                    onclick={handleSend}
                    disabled={sending || !to || !subject}
                >
                    {sending ? t.email.loading : t.email.send}
                </button>
                <button
                    class="rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                    onclick={close}
                >
                    {t.common.cancel}
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .compose-btn {
        background: linear-gradient(
            135deg,
            var(--color-primary) 0%,
            var(--color-secondary) 100%
        );
    }
</style>
