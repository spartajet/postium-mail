<script lang="ts">
    import "../app.css";

    import { onMount } from "svelte";

    import ToastContainer from "$lib/components/common/Toast.svelte";
    import { createAccountState } from "$lib/stores/account.svelte";
    import { createEmailState } from "$lib/stores/email.svelte";
    import { createI18nState } from "$lib/stores/i18n.svelte";
    import { createSyncState } from "$lib/stores/sync.svelte";
    import { createThemeState } from "$lib/stores/theme.svelte";
    import { createToastState } from "$lib/stores/toast.svelte";
    import { setupGlobalErrorHandler } from "$lib/utils/error.js";

    let { children } = $props();

    createThemeState();
    createI18nState();
    createAccountState();
    createEmailState();
    createSyncState();
    createToastState();

    onMount(() => {
        setupGlobalErrorHandler();
    });
</script>

{@render children()}

<ToastContainer />
