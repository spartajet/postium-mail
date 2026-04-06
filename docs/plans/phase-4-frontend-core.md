# Phase 4: 前端核心功能

> 前置：Phase 3 完成
> 完成标志：邮件列表可加载、邮件详情可查看、可写邮件发送、账号管理可用、同步功能可用

---

### Task 4.1: 初始化 Stores 到全局 Layout

**Files:**
- Modify: `src/routes/+layout.svelte`

**Step 1: 在 layout 中初始化所有 Store**

```svelte
<script lang="ts">
  import '../app.css';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import { createThemeState } from '$lib/stores/theme.svelte';
  import { createI18nState } from '$lib/stores/i18n.svelte';
  import { createAccountState } from '$lib/stores/account.svelte';
  import { createEmailState } from '$lib/stores/email.svelte';
  import { createSyncState } from '$lib/stores/sync.svelte';

  let { children } = $props();
  const theme = createThemeState();
  const i18n = createI18nState();
  const account = createAccountState();
  const email = createEmailState();
  const sync = createSyncState();
</script>

<div class="bg-orbs flex h-screen flex-col overflow-hidden bg-background text-foreground">
  <TitleBar />
  <main class="flex flex-1 overflow-hidden">
    <Sidebar />
    {@render children()}
  </main>
  <StatusBar />
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/routes/+layout.svelte
git commit -m "feat: initialize all stores in global layout"
```

---

### Task 4.2: 实现 AddAccountModal

**Files:**
- Create: `src/lib/components/settings/AddAccountModal.svelte`

**Step 1: 写 AddAccountModal**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getAccountState, type AccountDto } from '$lib/stores/account.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const accountStore = getAccountState();

  let open = $state(false);
  let step = $state<'form' | 'detecting' | 'done'>('form');
  let name = $state('');
  let email = $state('');
  let displayName = $state('');
  let password = $state('');
  let provider = $state('');
  let authType = $state('password');
  let providerDetected = $state(false);
  let providerName = $state('');
  let error = $state('');

  interface ProviderDetectionResult {
    detected: boolean;
    provider_id: string | null;
    provider_name: string | null;
    auth_types: string[];
  }

  async function detectProvider() {
    if (!email.includes('@')) return;
    step = 'detecting';
    try {
      const result = await invoke<ProviderDetectionResult>('detect_provider', { email });
      providerDetected = result.detected;
      if (result.detected && result.provider_id) {
        provider = result.provider_id;
        providerName = result.provider_name || result.provider_id;
        authType = result.auth_types[0] === 'OAuth2' ? 'oauth2' : 'password';
      }
    } catch (e: any) {
      providerDetected = false;
    }
    step = 'form';
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    error = '';
    try {
      await invoke<AccountDto>('create_account', {
        request: {
          name: name || email.split('@')[0],
          email,
          display_name: displayName || null,
          provider: provider || 'custom',
          auth_type: authType,
          password,
          imap_host: null,
          imap_port: null,
          smtp_host: null,
          smtp_port: null,
          color: null,
          account_type: 'personal',
        },
      });
      step = 'done';
      await accountStore.loadAccounts();
    } catch (e: any) {
      error = e.toString();
    }
  }

  function close() {
    open = false;
    step = 'form';
    name = '';
    email = '';
    displayName = '';
    password = '';
    provider = '';
    authType = 'password';
    providerDetected = false;
    error = '';
  }

  export function show() {
    open = true;
  }
</script>

{#if open}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" onkeydown={(e) => e.key === 'Escape' && close()}>
    <!-- Modal -->
    <div class="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-2xl">
      <div class="mb-4 flex items-center justify-between">
        <h2 class="text-lg font-semibold">{t.account.add}</h2>
        <button class="rounded-md p-1 text-muted-foreground hover:bg-muted" onclick={close}>
          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      {#if step === 'done'}
        <div class="py-8 text-center">
          <svg class="mx-auto h-12 w-12 text-green-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 11.08V12a10 10 0 11-5.93-9.14" />
            <polyline points="22 4 12 14.01 9 11.01" />
          </svg>
          <p class="mt-4 text-sm text-muted-foreground">账号添加成功！</p>
          <button class="mt-4 rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground" onclick={close}>
            {t.common.close}
          </button>
        </div>
      {:else}
        <form onsubmit={handleSubmit} class="space-y-4">
          <!-- Email -->
          <div>
            <label class="mb-1 block text-sm font-medium">{t.account.email}</label>
            <input
              type="email"
              bind:value={email}
              onchange={detectProvider}
              required
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              placeholder="you@example.com"
            />
            {#if step === 'detecting'}
              <p class="mt-1 text-xs text-muted-foreground">{t.account.detecting}</p>
            {:else if providerDetected}
              <p class="mt-1 text-xs text-green-600">{t.account.detected}: {providerName}</p>
            {:else if email.includes('@')}
              <p class="mt-1 text-xs text-amber-600">{t.account.notDetected}</p>
            {/if}
          </div>

          <!-- Name -->
          <div>
            <label class="mb-1 block text-sm font-medium">{t.account.name}</label>
            <input
              type="text"
              bind:value={name}
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              placeholder={email.split('@')[0] || 'Account name'}
            />
          </div>

          <!-- Display Name -->
          <div>
            <label class="mb-1 block text-sm font-medium">显示名称</label>
            <input
              type="text"
              bind:value={displayName}
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              placeholder="发件人名称（可选）"
            />
          </div>

          <!-- Password -->
          <div>
            <label class="mb-1 block text-sm font-medium">{t.account.password}</label>
            <input
              type="password"
              bind:value={password}
              required
              class="w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
              placeholder="授权码或密码"
            />
          </div>

          <!-- Error -->
          {#if error}
            <p class="text-sm text-destructive">{error}</p>
          {/if}

          <!-- Actions -->
          <div class="flex justify-end gap-2 pt-2">
            <button type="button" class="rounded-md border border-input px-4 py-2 text-sm" onclick={close}>
              {t.common.cancel}
            </button>
            <button type="submit" class="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground">
              {t.common.confirm}
            </button>
          </div>
        </form>
      {/if}
    </div>
  </div>
{/if}
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/lib/components/settings/AddAccountModal.svelte
git commit -m "feat: implement AddAccountModal with provider detection"
```

---

### Task 4.3: 完善 EmailList 组件

**Files:**
- Modify: `src/lib/components/email/EmailList.svelte`

**Step 1: 重写 EmailList**

```svelte
<script lang="ts">
  import { getEmailState, type EmailDto } from '$lib/stores/email.svelte';
  import { getAccountState } from '$lib/stores/account.svelte';
  import { getI18nState } from '$lib/stores/i18n.svelte';

  const emailStore = getEmailState();
  const accountStore = getAccountState();
  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let searchQuery = $state('');

  // 加载邮件
  $effect(() => {
    if (accountStore.activeAccountId) {
      emailStore.loadEmails(accountStore.activeAccountId, emailStore.currentFolder);
    }
  });

  function formatTime(timestamp: number): string {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const isToday = date.toDateString() === now.toDateString();
    if (isToday) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  function getInitials(name: string | null, email: string): string {
    if (name) return name.charAt(0).toUpperCase();
    return email.charAt(0).toUpperCase();
  }

  const colors = ['#EA4335', '#34A853', '#4285F4', '#FBBC05', '#FF6D01', '#46BDC6', '#7B61FF'];
  function getColor(str: string): string {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      hash = str.charCodeAt(i) + ((hash << 5) - hash);
    }
    return colors[Math.abs(hash) % colors.length];
  }
</script>

<div class="flex h-full flex-col">
  <!-- Search bar -->
  <div class="border-b border-border p-3">
    <div class="flex items-center gap-2 rounded-md border border-input bg-background px-3 py-1.5 transition-colors focus-within:border-ring">
      <svg class="h-4 w-4 shrink-0 text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
      <input
        type="text"
        bind:value={searchQuery}
        placeholder={t.email.search}
        class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
      />
    </div>
  </div>

  <!-- Toolbar -->
  <div class="flex items-center gap-1 border-b border-border px-3 py-1.5">
    <button class="rounded p-1.5 text-muted-foreground hover:bg-muted" title="刷新">
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0118.8-4.3M22 12.5a10 10 0 01-18.8 4.2" />
      </svg>
    </button>
    <div class="flex-1"></div>
    <span class="text-xs text-muted-foreground">{emailStore.emails.length} / {emailStore.total}</span>
  </div>

  <!-- Email items -->
  <div class="flex-1 overflow-y-auto">
    {#if emailStore.loading}
      <div class="flex items-center justify-center py-12">
        <svg class="h-5 w-5 animate-spin text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21.5 2v6h-6M2.5 22v-6h6" />
        </svg>
      </div>
    {:else if emailStore.emails.length === 0}
      <div class="flex items-center justify-center py-12">
        <p class="text-sm text-muted-foreground">{t.email.noEmails}</p>
      </div>
    {:else}
      {#each emailStore.emails as email (email.id)}
        <button
          class="flex w-full gap-3 border-b border-border px-4 py-3 text-left transition-colors {emailStore.selectedEmailId === email.id ? 'bg-muted' : 'hover:bg-muted/30'}"
          onclick={() => emailStore.selectEmail(email.id)}
        >
          <!-- Avatar -->
          <div
            class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-xs font-medium text-white"
            style="background-color: {getColor(email.sender_email)}"
          >
            {getInitials(email.sender_name, email.sender_email)}
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex items-center justify-between gap-2">
              <span class="truncate text-sm {email.is_read ? 'text-muted-foreground' : 'font-semibold text-foreground'}">
                {email.sender_name || email.sender_email}
              </span>
              <span class="shrink-0 text-xs text-muted-foreground">{formatTime(email.sent_at)}</span>
            </div>
            <p class="truncate text-sm {email.is_read ? 'text-muted-foreground' : 'font-medium text-foreground'}">
              {email.subject || '(无主题)'}
            </p>
            {#if email.preview}
              <p class="mt-0.5 truncate text-xs text-muted-foreground">{email.preview}</p>
            {/if}
          </div>

          <!-- Indicators -->
          <div class="flex shrink-0 flex-col items-center gap-1 pt-1">
            {#if email.is_starred}
              <svg class="h-3.5 w-3.5 text-amber-400" viewBox="0 0 24 24" fill="currentColor">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
              </svg>
            {/if}
          </div>
        </button>
      {/each}
    {/if}
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 邮件列表样式完整，带头像、主题、预览、时间

**Step 3: Commit**

```bash
git add src/lib/components/email/EmailList.svelte
git commit -m "feat: implement EmailList with avatar, formatting, and selection"
```

---

### Task 4.4: 完善 EmailDetail 组件

**Files:**
- Modify: `src/lib/components/email/EmailDetail.svelte`

**Step 1: 重写 EmailDetail**

```svelte
<script lang="ts">
  import { getEmailState } from '$lib/stores/email.svelte';
  import { getI18nState } from '$lib/stores/i18n.svelte';

  const emailStore = getEmailState();
  const i18n = getI18nState();
  const t = $derived(i18n.t);

  function formatDate(ts: number): string {
    return new Date(ts * 1000).toLocaleString();
  }
</script>

<div class="flex h-full flex-col overflow-hidden">
  {#if emailStore.selectedEmail}
    {@const email = emailStore.selectedEmail}

    <!-- Header -->
    <div class="border-b border-border p-6">
      <h1 class="text-xl font-semibold">{email.email.subject || '(无主题)'}</h1>

      <div class="mt-4 flex items-start gap-3">
        <!-- Avatar -->
        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-primary text-sm font-medium text-primary-foreground">
          {(email.email.sender_name || email.email.sender_email).charAt(0).toUpperCase()}
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span class="font-medium">{email.email.sender_name || email.email.sender_email}</span>
            <span class="text-sm text-muted-foreground">&lt;{email.email.sender_email}&gt;</span>
          </div>
          <div class="mt-0.5 text-xs text-muted-foreground">
            {t.email.to}: {email.recipient_emails}
            {#if email.cc_emails}
              &nbsp;| {t.email.cc}: {email.cc_emails}
            {/if}
          </div>
          <div class="mt-0.5 text-xs text-muted-foreground">{formatDate(email.email.sent_at)}</div>
        </div>
      </div>

      <!-- Actions -->
      <div class="mt-4 flex items-center gap-2">
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-input px-3 py-1.5 text-xs transition-colors hover:bg-muted"
          title={t.email.reply}
        >
          <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 17 4 12 9 7" /><path d="M20 18v-2a4 4 0 00-4-4H4" /></svg>
          {t.email.reply}
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-input px-3 py-1.5 text-xs transition-colors hover:bg-muted"
          title={t.email.forward}
        >
          <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 17 20 12 15 7" /><path d="M4 18v-2a4 4 0 014-4h12" /></svg>
          {t.email.forward}
        </button>
        <div class="flex-1"></div>
        <button
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs transition-colors hover:bg-muted"
          class:text-amber-500={email.email.is_starred}
          onclick={() => emailStore.toggleStar(email.email.id)}
        >
          <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill={email.email.is_starred ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2">
            <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
          </svg>
        </button>
        <button
          class="inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs text-destructive transition-colors hover:bg-destructive/10"
          onclick={() => emailStore.deleteEmails([email.email.id])}
        >
          <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto p-6">
      {#if email.body_html}
        <div class="prose prose-sm max-w-none dark:prose-invert">
          {@html email.body_html}
        </div>
      {:else if email.body_text}
        <pre class="whitespace-pre-wrap text-sm">{email.body_text}</pre>
      {:else}
        <p class="text-sm text-muted-foreground">{t.email.loading}</p>
      {/if}
    </div>
  {:else}
    <!-- Empty state -->
    <div class="flex h-full items-center justify-center">
      <div class="text-center">
        <svg class="mx-auto h-12 w-12 text-muted-foreground/30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
          <polyline points="22,6 12,13 2,6" />
        </svg>
        <p class="mt-3 text-sm text-muted-foreground">{t.email.noEmailSelected}</p>
      </div>
    </div>
  {/if}
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 邮件详情完整布局 — 主题、发件人、操作栏、HTML正文

**Step 3: Commit**

```bash
git add src/lib/components/email/EmailDetail.svelte
git commit -m "feat: implement EmailDetail with header, actions, and HTML body"
```

---

### Task 4.5: 实现 ComposeModal (写邮件)

**Files:**
- Create: `src/lib/components/email/ComposeModal.svelte`

**Step 1: 写 ComposeModal**

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { getAccountState } from '$lib/stores/account.svelte';
  import { getI18nState } from '$lib/stores/i18n.svelte';

  const accountStore = getAccountState();
  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let open = $state(false);
  let to = $state('');
  let cc = $state('');
  let subject = $state('');
  let bodyHtml = $state('');
  let bodyText = $state('');
  let sending = $state(false);
  let error = $state('');

  async function handleSend() {
    if (!accountStore.activeAccountId) return;
    sending = true;
    error = '';
    try {
      await invoke('send_email', {
        request: {
          account_id: accountStore.activeAccountId,
          to: to.split(',').map(s => s.trim()).filter(Boolean),
          cc: cc ? cc.split(',').map(s => s.trim()).filter(Boolean) : [],
          bcc: [],
          subject,
          body_html: bodyHtml || `<pre>${bodyText}</pre>`,
          body_text: bodyText,
        },
      });
      close();
    } catch (e: any) {
      error = e.toString();
    } finally {
      sending = false;
    }
  }

  function close() {
    open = false;
    to = '';
    cc = '';
    subject = '';
    bodyHtml = '';
    bodyText = '';
    error = '';
  }

  export function show() {
    open = true;
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 flex items-end justify-end p-6">
    <div class="flex h-[70vh] w-[560px] flex-col rounded-xl border border-border bg-card shadow-2xl">
      <!-- Header -->
      <div class="flex items-center justify-between border-b border-border px-4 py-3">
        <h3 class="font-semibold">新邮件</h3>
        <button class="rounded p-1 text-muted-foreground hover:bg-muted" onclick={close}>
          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <!-- Fields -->
      <div class="space-y-0 border-b border-border">
        <div class="flex items-center border-b border-border px-4">
          <span class="w-12 text-sm text-muted-foreground">{t.email.to}</span>
          <input type="text" bind:value={to} class="flex-1 bg-transparent py-2 text-sm outline-none" placeholder="email@example.com" />
        </div>
        <div class="flex items-center border-b border-border px-4">
          <span class="w-12 text-sm text-muted-foreground">{t.email.cc}</span>
          <input type="text" bind:value={cc} class="flex-1 bg-transparent py-2 text-sm outline-none" placeholder="cc@example.com (可选)" />
        </div>
        <div class="flex items-center px-4">
          <span class="w-12 text-sm text-muted-foreground">{t.email.subject}</span>
          <input type="text" bind:value={subject} class="flex-1 bg-transparent py-2 text-sm outline-none" />
        </div>
      </div>

      <!-- Body (简化 textarea，TipTap 在 Phase 5 集成) -->
      <div class="flex-1 p-4">
        <textarea
          bind:value={bodyText}
          class="h-full w-full resize-none bg-transparent text-sm outline-none"
          placeholder="在此输入邮件内容..."
        ></textarea>
      </div>

      <!-- Error -->
      {#if error}
        <div class="px-4 py-2 text-sm text-destructive">{error}</div>
      {/if}

      <!-- Footer -->
      <div class="flex items-center justify-between border-t border-border px-4 py-3">
        <button
          class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
          onclick={handleSend}
          disabled={sending || !to || !subject}
        >
          {sending ? '发送中...' : t.email.send}
        </button>
        <button class="rounded-md px-3 py-2 text-sm text-muted-foreground hover:bg-muted" onclick={close}>
          存草稿
        </button>
      </div>
    </div>
  </div>
{/if}
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/lib/components/email/ComposeModal.svelte
git commit -m "feat: implement ComposeModal for sending emails"
```

---

### Task 4.6: 连接 Sidebar 与 Store

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte`

**Step 1: 更新 Sidebar 连接 store**

在 Sidebar.svelte 的 script 部分添加 store 连接：

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getAccountState } from '$lib/stores/account.svelte';
  import { getEmailState } from '$lib/stores/email.svelte';
  import { getSyncState } from '$lib/stores/sync.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const accountStore = getAccountState();
  const emailStore = getEmailState();
  const syncStore = getSyncState();

  // 切换文件夹
  function selectFolder(folderId: string) {
    emailStore.currentFolder = folderId;
    if (accountStore.activeAccountId) {
      emailStore.loadEmails(accountStore.activeAccountId, folderId);
    }
  }

  // 触发同步
  async function handleSync() {
    if (accountStore.activeAccountId) {
      await syncStore.syncAccount(accountStore.activeAccountId);
    }
  }
</script>
```

并更新文件夹按钮 `onclick` 为 `onclick={() => selectFolder(folder.id)}`，同步按钮 `onclick={handleSync}`。

**Step 2: 验证**

Run: `bun run dev`
Expected: 点击文件夹切换高亮，同步按钮可点击

**Step 3: Commit**

```bash
git add src/lib/components/layout/Sidebar.svelte
git commit -m "feat: connect Sidebar to Email and Sync stores"
```

---

### Task 4.7: 实现同步进度显示

**Files:**
- Modify: `src/lib/components/layout/StatusBar.svelte`

**Step 1: 更新 StatusBar 连接 Sync Store**

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getSyncState } from '$lib/stores/sync.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const syncStore = getSyncState();

  let statusText = $derived(
    syncStore.syncing
      ? syncStore.progress?.message || t.sync.syncing
      : syncStore.error
        ? t.sync.failed
        : t.sync.lastSync + ': ' + t.sync.never
  );
</script>

<div class="flex h-8 items-center justify-between border-t border-border bg-card/60 px-4 text-xs text-muted-foreground backdrop-blur-md">
  <div class="flex items-center gap-2">
    {#if syncStore.syncing}
      <svg class="h-3 w-3 animate-spin text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21.5 2v6h-6M2.5 22v-6h6" />
      </svg>
    {/if}
    <span class={syncStore.error ? 'text-destructive' : ''}>{statusText}</span>
    {#if syncStore.syncing && syncStore.progress}
      <span>({syncStore.progress.current}/{syncStore.progress.total})</span>
    {/if}
  </div>
  <span>v0.1.0</span>
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 状态栏显示实时同步状态

**Step 3: Commit**

```bash
git add src/lib/components/layout/StatusBar.svelte
git commit -m "feat: connect StatusBar to Sync store for live progress"
```

---

### Task 4.8: 实现搜索功能

**Files:**
- Modify: `src/lib/components/email/EmailList.svelte`

**Step 1: 在 EmailList 添加搜索逻辑**

在 script 部分添加：

```typescript
import { invoke } from '@tauri-apps/api/core';

interface SearchResult {
  id: number;
  account_id: number;
  folder: string;
  subject: string | null;
  sender_email: string;
  sent_at: number;
  preview: string | null;
  rank: number;
}

let isSearching = $state(false);

async function handleSearch() {
  if (!searchQuery.trim()) {
    // 恢复正常列表
    if (accountStore.activeAccountId) {
      await emailStore.loadEmails(accountStore.activeAccountId, emailStore.currentFolder);
    }
    return;
  }

  isSearching = true;
  try {
    const results = await invoke<SearchResult[]>('search_emails', {
      query: searchQuery,
      accountId: accountStore.activeAccountId,
      limit: 50,
    });
    // 将搜索结果转换为 EmailDto 格式显示
    emailStore.emails = results.map(r => ({
      id: r.id,
      account_id: r.account_id,
      folder: r.folder,
      uid: null,
      subject: r.subject,
      sender_name: null,
      sender_email: r.sender_email,
      preview: r.preview,
      is_read: true,
      is_starred: false,
      sent_at: r.sent_at,
      has_attachments: false,
    }));
    emailStore.total = results.length;
  } catch (e: any) {
    console.error('Search failed:', e);
  } finally {
    isSearching = false;
  }
}
```

更新搜索输入框添加回车事件：
```svelte
<input
  type="text"
  bind:value={searchQuery}
  placeholder={t.email.search}
  class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
  onkeydown={(e) => e.key === 'Enter' && handleSearch()}
/>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 搜索框输入后按回车触发搜索

**Step 3: Commit**

```bash
git add src/lib/components/email/EmailList.svelte
git commit -m "feat: add FTS5 search to EmailList"
```

---

### Task 4.9: 最终验证 + 修复

**Step 1: 完整编译**

Run: `bun run build`

**Step 2: 启动测试**

Run: `bun run tauri dev`

验证：
- 添加账号 → 弹窗 → 检测服务商 → 填写密码 → 提交
- 邮件列表 → 点击邮件 → 详情展示
- 写邮件 → 填写 → 发送
- 同步按钮 → 进度显示
- 搜索 → 结果展示

**Step 3: 修复编译/运行问题**

根据错误信息修复。

**Step 4: 最终 Commit**

```bash
git add -A
git commit -m "feat: Phase 4 complete - email CRUD, compose, search, sync UI"
```
