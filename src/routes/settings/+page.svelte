<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getThemeState } from '$lib/stores/theme.svelte';
  import { getAccountState } from '$lib/stores/account.svelte';
  import { Settings, Palette, Globe, Monitor, Moon, Sun, User, Plus, Trash2, ChevronLeft } from 'lucide-svelte';
  import { goto } from '$app/navigation';

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const themeStore = getThemeState();
  const accountStore = getAccountState();

  $effect(() => {
    accountStore.loadAccounts();
  });
</script>

<div class="flex h-full flex-col overflow-y-auto bg-background">
  <!-- Header -->
  <div class="flex items-center gap-3 border-b border-border px-6 py-4">
    <button
      class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
      onclick={() => goto('/')}
    >
      <ChevronLeft size={20} />
    </button>
    <Settings size={20} class="text-primary" />
    <h1 class="text-lg font-semibold text-foreground">{t.settings.title}</h1>
  </div>

  <div class="max-w-2xl space-y-6 p-6">
    <!-- Appearance -->
    <section class="rounded-xl border border-border bg-card p-5">
      <div class="mb-4 flex items-center gap-2">
        <Palette size={18} class="text-primary" />
        <h2 class="text-sm font-semibold text-foreground">{t.settings.appearance}</h2>
      </div>

      <div class="space-y-4">
        <!-- Theme -->
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Monitor size={16} class="text-muted-foreground" />
            <span class="text-sm text-foreground">{t.settings.theme}</span>
          </div>
          <div class="flex rounded-lg border border-border bg-glass p-0.5">
            <button
              class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme === 'light' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => themeStore.setTheme('light')}
            >
              <Sun size={12} /> {t.settings.light}
            </button>
            <button
              class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme === 'dark' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => themeStore.setTheme('dark')}
            >
              <Moon size={12} /> {t.settings.dark}
            </button>
            <button
              class="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors {themeStore.theme === 'system' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => themeStore.setTheme('system')}
            >
              <Monitor size={12} /> {t.settings.system}
            </button>
          </div>
        </div>

        <!-- Language -->
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2">
            <Globe size={16} class="text-muted-foreground" />
            <span class="text-sm text-foreground">{t.settings.language}</span>
          </div>
          <div class="flex rounded-lg border border-border bg-glass p-0.5">
            <button
              class="rounded-md px-3 py-1.5 text-xs transition-colors {i18n.locale === 'zh-CN' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => i18n.setLocale('zh-CN')}
            >
              中文
            </button>
            <button
              class="rounded-md px-3 py-1.5 text-xs transition-colors {i18n.locale === 'en-US' ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
              onclick={() => i18n.setLocale('en-US')}
            >
              English
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- Accounts -->
    <section class="rounded-xl border border-border bg-card p-5">
      <div class="mb-4 flex items-center justify-between">
        <div class="flex items-center gap-2">
          <User size={18} class="text-primary" />
          <h2 class="text-sm font-semibold text-foreground">{t.settings.accounts}</h2>
        </div>
      </div>

      <div class="space-y-2">
        {#each accountStore.accounts as account (account.id)}
          <div class="flex items-center gap-3 rounded-lg border border-border bg-glass px-4 py-3">
            <div class="flex h-8 w-8 items-center justify-center rounded-full bg-primary/15 text-xs font-bold text-primary">
              {account.email.charAt(0).toUpperCase()}
            </div>
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-medium text-foreground">{account.email}</div>
              <div class="text-xs text-muted-foreground">{account.provider}</div>
            </div>
            <button
              class="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
              onclick={() => accountStore.deleteAccount(account.id)}
              title={t.account.delete}
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}

        {#if accountStore.accounts.length === 0}
          <div class="py-6 text-center text-sm text-muted-foreground">
            {t.settings.accounts}
          </div>
        {/if}
      </div>
    </section>
  </div>
</div>
