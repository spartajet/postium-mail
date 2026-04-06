<script lang="ts">
  import '../app.css';
  import { onMount, setContext } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import ComposeModal from '$lib/components/email/ComposeModal.svelte';
  import AddAccountModal from '$lib/components/settings/AddAccountModal.svelte';
  import { createThemeState } from '$lib/stores/theme.svelte';
  import { createI18nState } from '$lib/stores/i18n.svelte';
  import { createAccountState } from '$lib/stores/account.svelte';
  import { createEmailState } from '$lib/stores/email.svelte';
  import { createSyncState } from '$lib/stores/sync.svelte';
  import { createToastState } from '$lib/stores/toast.svelte';
  import { setupGlobalErrorHandler } from '$lib/utils/error.js';
  import ToastContainer from '$lib/components/common/Toast.svelte';

  let { children } = $props();

  const theme = createThemeState();
  const i18n = createI18nState();
  const account = createAccountState();
  const email = createEmailState();
  const sync = createSyncState();
  const toast = createToastState();

  let composeModal = $state<ComposeModal>();
  let addAccountModal = $state<AddAccountModal>();

  // 通过 context 共享 modal ref，避免子页面创建重复实例
  const COMPOSE_MODAL_KEY = Symbol.for('compose-modal');
  const ADD_ACCOUNT_MODAL_KEY = Symbol.for('add-account-modal');
  setContext(COMPOSE_MODAL_KEY, () => composeModal);
  setContext(ADD_ACCOUNT_MODAL_KEY, () => addAccountModal);

  onMount(() => {
    setupGlobalErrorHandler();

    const unlisten = listen<string>('tray-action', (event) => {
      if (event.payload === 'compose') {
        composeModal?.show();
      } else if (event.payload === 'sync') {
        if (account.activeAccountId) {
          sync.syncAccount(account.activeAccountId);
        }
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  });
</script>

<div class="relative flex h-screen flex-col overflow-hidden bg-background text-foreground">
  <!-- Background orbs -->
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
  <ToastContainer />
</div>
