# Phase 3: 前端基建 + 布局

> 前置：Phase 0 + Phase 1 + Phase 2 完成
> 完成标志：三栏布局渲染完成，TitleBar 可拖拽，Sidebar 可导航，Store 可响应，主题/i18n 切换正常

---

### Task 3.1: 配置 SvelteKit SPA 模式 + 全局 Layout

**Files:**
- Modify: `src/routes/+layout.ts`
- Modify: `src/routes/+layout.svelte`
- Modify: `src/app.html`

**Step 1: 配置 SPA 模式**

`src/routes/+layout.ts`:
```typescript
export const ssr = false;
export const prerender = false;
```

**Step 2: 更新 +layout.svelte**

```svelte
<script lang="ts">
  import '../app.css';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import { ThemeProvider } from '$lib/stores/theme.svelte';

  let { children } = $props();
</script>

<ThemeProvider>
  <div class="flex h-screen flex-col overflow-hidden bg-background text-foreground">
    <TitleBar />
    <main class="flex flex-1 overflow-hidden">
      {@render children()}
    </main>
    <StatusBar />
  </div>
</ThemeProvider>
```

**Step 3: 更新 app.html 确保无边距**

在 `<body>` 标签中移除默认样式，确保：
```html
<body class="overflow-hidden">
  <div style="display: contents">%sveltekit.body%</div>
</body>
```

**Step 4: 验证**

Run: `bun run dev`
Expected: 空白页面无滚动条，占满全屏

**Step 5: Commit**

```bash
git add src/routes/
git commit -m "feat: setup SvelteKit SPA mode and global layout"
```

---

### Task 3.2: 实现 TitleBar 组件

**Files:**
- Create: `src/lib/components/layout/TitleBar.svelte`

**Step 1: 写 TitleBar**

```svelte
<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  const appWindow = getCurrentWindow();

  async function minimize() {
    await appWindow.minimize();
  }

  async function toggleMaximize() {
    await appWindow.toggleMaximize();
  }

  async function close() {
    await appWindow.close();
  }
</script>

<div class="flex h-10 select-none items-center justify-between border-b border-border bg-card/80 backdrop-blur-md">
  <!-- 拖拽区域 -->
  <div class="flex flex-1 items-center gap-2 pl-4" data-tauri-drag-region>
    <div class="flex h-5 w-5 items-center justify-center rounded bg-primary text-[10px] font-bold text-primary-foreground">
      P
    </div>
    <span class="text-xs font-medium text-muted-foreground" data-tauri-drag-region>Postium Mail</span>
  </div>

  <!-- 窗口控制按钮 -->
  <div class="flex h-full">
    <button
      class="inline-flex h-full w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-muted"
      onclick={minimize}
      aria-label="最小化"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
        <line x1="1" y1="6" x2="11" y2="6" />
      </svg>
    </button>
    <button
      class="inline-flex h-full w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-muted"
      onclick={toggleMaximize}
      aria-label="最大化"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="1.5" y="1.5" width="9" height="9" />
      </svg>
    </button>
    <button
      class="inline-flex h-full w-10 items-center justify-center text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
      onclick={close}
      aria-label="关闭"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
        <line x1="1" y1="1" x2="11" y2="11" />
        <line x1="11" y1="1" x2="1" y2="11" />
      </svg>
    </button>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run tauri dev`
Expected: 顶部 40px 标题栏，可拖拽移动窗口，三个按钮可操作

**Step 3: Commit**

```bash
git add src/lib/components/layout/TitleBar.svelte
git commit -m "feat: implement TitleBar with drag region and window controls"
```

---

### Task 3.3: 实现主题切换 Store

**Files:**
- Create: `src/lib/stores/theme.svelte.ts`

**Step 1: 写 Theme Store**

```typescript
import { getContext, setContext } from 'svelte';

type Theme = 'light' | 'dark' | 'system';

class ThemeState {
  theme = $state<Theme>('system');
  resolved = $state<'light' | 'dark'>('light');

  constructor() {
    const saved = localStorage.getItem('postium-theme') as Theme | null;
    if (saved) {
      this.theme = saved;
    }

    // 监听系统主题
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    this.resolved = this.getResolved();

    mediaQuery.addEventListener('change', () => {
      if (this.theme === 'system') {
        this.resolved = this.getResolved();
        this.applyClass();
      }
    });

    this.applyClass();
  }

  private getResolved(): 'light' | 'dark' {
    if (this.theme === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return this.theme;
  }

  private applyClass() {
    document.documentElement.classList.toggle('dark', this.resolved === 'dark');
  }

  setTheme(theme: Theme) {
    this.theme = theme;
    localStorage.setItem('postium-theme', theme);
    this.resolved = this.getResolved();
    this.applyClass();
  }
}

const THEME_KEY = Symbol('theme');

export function createThemeState() {
  const state = new ThemeState();
  setContext(THEME_KEY, state);
  return state;
}

export function getThemeState() {
  return getContext<ThemeState>(THEME_KEY);
}

export function ThemeProvider({ children }: { children: any }) {
  // Svelte 5 snippet component — see usage in layout
}
```

**Step 2: 在 +layout.svelte 中使用 ThemeProvider**

更新 `+layout.svelte`，包裹 ThemeProvider（已在 Task 3.1 写好）：

```svelte
<script lang="ts">
  import '../app.css';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import { createThemeState } from '$lib/stores/theme.svelte';

  let { children } = $props();
  const theme = createThemeState();
</script>

<div class="flex h-screen flex-col overflow-hidden bg-background text-foreground">
  <TitleBar />
  <main class="flex flex-1 overflow-hidden">
    {@render children()}
  </main>
  <StatusBar />
</div>
```

**Step 3: 验证**

Run: `bun run dev`
Expected: 页面默认跟随系统主题，`documentElement` 有/无 `dark` class

**Step 4: Commit**

```bash
git add src/lib/stores/theme.svelte.ts
git commit -m "feat: implement theme store with system/light/dark switching"
```

---

### Task 3.4: 实现 i18n Store

**Files:**
- Create: `src/lib/stores/i18n.svelte.ts`
- Create: `src/lib/i18n/zh-CN.ts`
- Create: `src/lib/i18n/en-US.ts`

**Step 1: 写中文翻译**

`src/lib/i18n/zh-CN.ts`:
```typescript
export default {
  app: { name: 'Postium Mail' },
  sidebar: {
    inbox: '收件箱',
    starred: '星标邮件',
    sent: '已发送',
    drafts: '草稿',
    spam: '垃圾邮件',
    trash: '回收站',
    archive: '归档',
    compose: '写邮件',
    calendar: '日历',
    workflow: '工作流',
    labels: '标签',
    storage: '存储',
    settings: '设置',
    sync: '同步',
    allAccounts: '全部账号',
  },
  email: {
    search: '搜索邮件...',
    noEmailSelected: '选择一封邮件开始阅读',
    noEmails: '没有邮件',
    markRead: '标记已读',
    markUnread: '标记未读',
    star: '星标',
    unstar: '取消星标',
    delete: '删除',
    archive: '归档',
    move: '移动到',
    reply: '回复',
    replyAll: '全部回复',
    forward: '转发',
    send: '发送',
    to: '收件人',
    cc: '抄送',
    bcc: '密送',
    subject: '主题',
    from: '发件人',
    date: '日期',
    attachments: '附件',
    loading: '加载中...',
  },
  account: {
    add: '添加账号',
    edit: '编辑',
    delete: '删除',
    name: '账号名称',
    email: '邮箱地址',
    password: '密码',
    provider: '邮件服务商',
    authType: '认证方式',
    detecting: '检测服务商...',
    detected: '检测到',
    notDetected: '未识别，请手动配置',
  },
  sync: {
    syncing: '同步中...',
    completed: '同步完成',
    failed: '同步失败',
    lastSync: '上次同步',
    never: '从未同步',
    connecting: '连接中...',
    syncingFolders: '同步文件夹...',
    syncingEmails: '同步邮件...',
  },
  settings: {
    title: '设置',
    general: '通用',
    accounts: '账号管理',
    appearance: '外观',
    language: '语言',
    theme: '主题',
    light: '浅色',
    dark: '深色',
    system: '跟随系统',
  },
  common: {
    confirm: '确认',
    cancel: '取消',
    save: '保存',
    close: '关闭',
    loading: '加载中...',
    error: '出错了',
    retry: '重试',
  },
} as const;
```

**Step 2: 写英文翻译**

`src/lib/i18n/en-US.ts`:
```typescript
export default {
  app: { name: 'Postium Mail' },
  sidebar: {
    inbox: 'Inbox',
    starred: 'Starred',
    sent: 'Sent',
    drafts: 'Drafts',
    spam: 'Spam',
    trash: 'Trash',
    archive: 'Archive',
    compose: 'Compose',
    calendar: 'Calendar',
    workflow: 'Workflow',
    labels: 'Labels',
    storage: 'Storage',
    settings: 'Settings',
    sync: 'Sync',
    allAccounts: 'All Accounts',
  },
  email: {
    search: 'Search emails...',
    noEmailSelected: 'Select an email to read',
    noEmails: 'No emails',
    markRead: 'Mark as read',
    markUnread: 'Mark as unread',
    star: 'Star',
    unstar: 'Unstar',
    delete: 'Delete',
    archive: 'Archive',
    move: 'Move to',
    reply: 'Reply',
    replyAll: 'Reply all',
    forward: 'Forward',
    send: 'Send',
    to: 'To',
    cc: 'CC',
    bcc: 'BCC',
    subject: 'Subject',
    from: 'From',
    date: 'Date',
    attachments: 'Attachments',
    loading: 'Loading...',
  },
  account: {
    add: 'Add Account',
    edit: 'Edit',
    delete: 'Delete',
    name: 'Account Name',
    email: 'Email Address',
    password: 'Password',
    provider: 'Email Provider',
    authType: 'Auth Type',
    detecting: 'Detecting provider...',
    detected: 'Detected',
    notDetected: 'Not recognized, configure manually',
  },
  sync: {
    syncing: 'Syncing...',
    completed: 'Sync completed',
    failed: 'Sync failed',
    lastSync: 'Last sync',
    never: 'Never',
    connecting: 'Connecting...',
    syncingFolders: 'Syncing folders...',
    syncingEmails: 'Syncing emails...',
  },
  settings: {
    title: 'Settings',
    general: 'General',
    accounts: 'Accounts',
    appearance: 'Appearance',
    language: 'Language',
    theme: 'Theme',
    light: 'Light',
    dark: 'Dark',
    system: 'System',
  },
  common: {
    confirm: 'Confirm',
    cancel: 'Cancel',
    save: 'Save',
    close: 'Close',
    loading: 'Loading...',
    error: 'Error',
    retry: 'Retry',
  },
} as const;
```

**Step 3: 写 i18n Store**

`src/lib/stores/i18n.svelte.ts`:
```typescript
import zhCN from '$lib/i18n/zh-CN';
import enUS from '$lib/i18n/en-US';
import { getContext, setContext } from 'svelte';

export type Locale = 'zh-CN' | 'en-US';
type Translation = typeof zhCN;

const translations: Record<Locale, Translation> = {
  'zh-CN': zhCN,
  'en-US': enUS,
};

class I18nState {
  locale = $state<Locale>('zh-CN');
  t = $derived(translations[this.locale]);

  constructor() {
    const saved = localStorage.getItem('postium-locale') as Locale | null;
    if (saved && translations[saved]) {
      this.locale = saved;
    }
  }

  setLocale(locale: Locale) {
    this.locale = locale;
    localStorage.setItem('postium-locale', locale);
  }
}

const I18N_KEY = Symbol('i18n');

export function createI18nState() {
  const state = new I18nState();
  setContext(I18N_KEY, state);
  return state;
}

export function getI18nState() {
  return getContext<I18nState>(I18N_KEY);
}
```

**Step 4: 在 layout 中初始化**

更新 `+layout.svelte`：
```svelte
<script lang="ts">
  import '../app.css';
  import TitleBar from '$lib/components/layout/TitleBar.svelte';
  import StatusBar from '$lib/components/layout/StatusBar.svelte';
  import { createThemeState } from '$lib/stores/theme.svelte';
  import { createI18nState } from '$lib/stores/i18n.svelte';

  let { children } = $props();
  const theme = createThemeState();
  const i18n = createI18nState();
</script>

<div class="flex h-screen flex-col overflow-hidden bg-background text-foreground">
  <TitleBar />
  <main class="flex flex-1 overflow-hidden">
    {@render children()}
  </main>
  <StatusBar />
</div>
```

**Step 5: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 6: Commit**

```bash
git add src/lib/stores/i18n.svelte.ts src/lib/i18n/
git commit -m "feat: implement i18n store with zh-CN and en-US translations"
```

---

### Task 3.5: 实现 Sidebar 组件

**Files:**
- Create: `src/lib/components/layout/Sidebar.svelte`

**Step 1: 写 Sidebar**

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let activeFolder = $state('inbox');

  const folders = $derived([
    { id: 'inbox', icon: 'inbox', label: t.sidebar.inbox, count: 0 },
    { id: 'starred', icon: 'star', label: t.sidebar.starred, count: 0 },
    { id: 'sent', icon: 'send', label: t.sidebar.sent, count: 0 },
    { id: 'drafts', icon: 'file', label: t.sidebar.drafts, count: 0 },
    { id: 'spam', icon: 'alert-circle', label: t.sidebar.spam, count: 0 },
    { id: 'trash', icon: 'trash', label: t.sidebar.trash, count: 0 },
  ]);

  function getFolderIcon(icon: string): string {
    const map: Record<string, string> = {
      inbox: 'M20 4H4a2 2 0 00-2 2v12a2 2 0 002 2h16a2 2 0 002-2V6a2 2 0 00-2-2zm0 4l-8 5-8-5',
      star: 'M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z',
      send: 'M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z',
      file: 'M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z M14 2v6h6',
      'alert-circle': 'M12 2a10 10 0 100 20 10 10 0 000-20zM12 8v4M12 16h.01',
      trash: 'M3 6h18M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2',
    };
    return map[icon] || map.inbox;
  }
</script>

<aside class="flex h-full w-60 flex-col border-r border-border bg-card/60 backdrop-blur-md">
  <!-- Logo + Sync -->
  <div class="flex items-center justify-between px-4 py-3">
    <div class="flex items-center gap-2">
      <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-primary text-xs font-bold text-primary-foreground">
        P
      </div>
      <span class="text-sm font-semibold">Postium</span>
    </div>
    <button
      class="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      title={t.sidebar.sync}
    >
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0118.8-4.3M22 12.5a10 10 0 01-18.8 4.2" />
      </svg>
    </button>
  </div>

  <!-- Compose Button -->
  <div class="px-3 pb-2">
    <button class="flex w-full items-center justify-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90">
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 5v14M5 12h14" />
      </svg>
      {t.sidebar.compose}
    </button>
  </div>

  <!-- Folder Nav -->
  <nav class="flex-1 overflow-y-auto px-2 py-1">
    {#each folders as folder}
      <button
        class="flex w-full items-center gap-3 rounded-md px-3 py-1.5 text-sm transition-colors {activeFolder === folder.id ? 'bg-muted font-medium text-foreground' : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
        onclick={() => activeFolder = folder.id}
      >
        <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d={getFolderIcon(folder.icon)} />
        </svg>
        <span class="flex-1 text-left">{folder.label}</span>
        {#if folder.count > 0}
          <span class="rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
            {folder.count}
          </span>
        {/if}
      </button>
    {/each}

    <!-- Separator -->
    <div class="my-2 h-px bg-border"></div>

    <!-- Extra nav -->
    <button class="flex w-full items-center gap-3 rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground">
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="4" width="18" height="18" rx="2" ry="2" />
        <line x1="16" y1="2" x2="16" y2="6" />
        <line x1="8" y1="2" x2="8" y2="6" />
        <line x1="3" y1="10" x2="21" y2="10" />
      </svg>
      {t.sidebar.calendar}
    </button>
    <button class="flex w-full items-center gap-3 rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground">
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="16 18 22 12 16 6" />
        <polyline points="8 6 2 12 8 18" />
      </svg>
      {t.sidebar.workflow}
    </button>
  </nav>

  <!-- Bottom section -->
  <div class="border-t border-border p-3">
    <button class="flex w-full items-center gap-3 rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground">
      <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
      </svg>
      {t.sidebar.settings}
    </button>
  </div>
</aside>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 侧边栏显示文件夹列表、写邮件按钮、日历/工作流入口、设置按钮

**Step 3: Commit**

```bash
git add src/lib/components/layout/Sidebar.svelte
git commit -m "feat: implement Sidebar with folder navigation and compose button"
```

---

### Task 3.6: 实现 StatusBar 组件

**Files:**
- Create: `src/lib/components/layout/StatusBar.svelte`

**Step 1: 写 StatusBar**

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let syncStatus = $state('idle'); // 'idle' | 'syncing' | 'error'
  let lastSyncText = $state(t.sync.never);
</script>

<div class="flex h-8 items-center justify-between border-t border-border bg-card/60 px-4 text-xs text-muted-foreground backdrop-blur-md">
  <div class="flex items-center gap-2">
    {#if syncStatus === 'syncing'}
      <svg class="h-3 w-3 animate-spin text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21.5 2v6h-6M2.5 22v-6h6" />
      </svg>
      <span>{t.sync.syncing}</span>
    {:else if syncStatus === 'error'}
      <span class="text-destructive">{t.sync.failed}</span>
    {:else}
      <span>{t.sync.lastSync}: {lastSyncText}</span>
    {/if}
  </div>
  <div class="flex items-center gap-3">
    <span>v0.1.0</span>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 底部 32px 状态栏显示同步状态

**Step 3: Commit**

```bash
git add src/lib/components/layout/StatusBar.svelte
git commit -m "feat: implement StatusBar with sync status display"
```

---

### Task 3.7: 组装三栏布局 AppShell

**Files:**
- Modify: `src/routes/+page.svelte`

**Step 1: 重写 +page.svelte**

```svelte
<script lang="ts">
  import Sidebar from '$lib/components/layout/Sidebar.svelte';
  import EmailList from '$lib/components/email/EmailList.svelte';
  import EmailDetail from '$lib/components/email/EmailDetail.svelte';
</script>

<div class="flex h-full w-full">
  <Sidebar />

  <!-- Email List Panel -->
  <div class="flex w-80 flex-col border-r border-border">
    <EmailList />
  </div>

  <!-- Email Detail Panel -->
  <div class="flex-1">
    <EmailDetail />
  </div>
</div>
```

**Step 2: 创建 EmailList + EmailDetail 占位组件**

`src/lib/components/email/EmailList.svelte`:
```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  const i18n = getI18nState();
  const t = $derived(i18n.t);
</script>

<div class="flex h-full flex-col">
  <!-- Search bar -->
  <div class="border-b border-border p-3">
    <div class="flex items-center gap-2 rounded-md border border-input bg-background px-3 py-1.5">
      <svg class="h-4 w-4 text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
      <input
        type="text"
        placeholder={t.email.search}
        class="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
      />
    </div>
  </div>

  <!-- Email list placeholder -->
  <div class="flex flex-1 items-center justify-center">
    <p class="text-sm text-muted-foreground">{t.email.noEmails}</p>
  </div>
</div>
```

`src/lib/components/email/EmailDetail.svelte`:
```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  const i18n = getI18nState();
  const t = $derived(i18n.t);
</script>

<div class="flex h-full items-center justify-center">
  <div class="text-center">
    <svg class="mx-auto h-12 w-12 text-muted-foreground/30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round">
      <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z" />
      <polyline points="22,6 12,13 2,6" />
    </svg>
    <p class="mt-3 text-sm text-muted-foreground">{t.email.noEmailSelected}</p>
  </div>
</div>
```

**Step 3: 验证**

Run: `bun run tauri dev`
Expected: 三栏布局正常渲染 — Sidebar 240px | EmailList 320px | EmailDetail 自适应

**Step 4: Commit**

```bash
git add src/routes/+page.svelte src/lib/components/email/
git commit -m "feat: assemble 3-panel layout with placeholder components"
```

---

### Task 3.8: 实现 Account Store

**Files:**
- Create: `src/lib/stores/account.svelte.ts`

**Step 1: 写 Account Store**

```typescript
import { getContext, setContext } from 'svelte';
import { invoke } from '@tauri-apps/api/core';

// 类型将在 Phase 2 完成后从 bindings.ts 导入
// 目前先手动定义
interface AccountDto {
  id: number;
  name: string;
  email: string;
  display_name: string | null;
  provider: string;
  color: string | null;
  sync_enabled: boolean;
  auth_type: string;
  account_type: string;
  last_sync_at: number | null;
  created_at: number;
}

class AccountState {
  accounts = $state<AccountDto[]>([]);
  activeAccountId = $state<number | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  activeAccount = $derived(
    this.accounts.find(a => a.id === this.activeAccountId) ?? null
  );

  async loadAccounts() {
    this.loading = true;
    this.error = null;
    try {
      this.accounts = await invoke<AccountDto[]>('list_accounts');
      if (!this.activeAccountId && this.accounts.length > 0) {
        this.activeAccountId = this.accounts[0].id;
      }
    } catch (e: any) {
      this.error = e.toString();
    } finally {
      this.loading = false;
    }
  }

  setActive(id: number) {
    this.activeAccountId = id;
  }

  async deleteAccount(id: number) {
    try {
      await invoke('delete_account', { id });
      this.accounts = this.accounts.filter(a => a.id !== id);
      if (this.activeAccountId === id) {
        this.activeAccountId = this.accounts.length > 0 ? this.accounts[0].id : null;
      }
    } catch (e: any) {
      this.error = e.toString();
    }
  }
}

const ACCOUNT_KEY = Symbol('account');

export function createAccountState() {
  const state = new AccountState();
  setContext(ACCOUNT_KEY, state);
  return state;
}

export function getAccountState() {
  return getContext<AccountState>(ACCOUNT_KEY);
}

export type { AccountDto };
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/lib/stores/account.svelte.ts
git commit -m "feat: implement Account store with Svelte 5 runes"
```

---

### Task 3.9: 实现 Email Store

**Files:**
- Create: `src/lib/stores/email.svelte.ts`

**Step 1: 写 Email Store**

```typescript
import { getContext, setContext } from 'svelte';
import { invoke } from '@tauri-apps/api/core';

interface EmailDto {
  id: number;
  account_id: number;
  folder: string;
  uid: number | null;
  subject: string | null;
  sender_name: string | null;
  sender_email: string;
  preview: string | null;
  is_read: boolean;
  is_starred: boolean;
  sent_at: number;
  has_attachments: boolean;
}

interface EmailListResponse {
  emails: EmailDto[];
  total: number;
  page: number;
  limit: number;
}

interface EmailDetail {
  email: EmailDto;
  recipient_emails: string;
  cc_emails: string | null;
  body_text: string | null;
  body_html: string | null;
}

class EmailState {
  emails = $state<EmailDto[]>([]);
  selectedEmailId = $state<number | null>(null);
  selectedEmail = $state<EmailDetail | null>(null);
  total = $state(0);
  page = $state(1);
  limit = $state(50);
  loading = $state(false);
  currentFolder = $state('inbox');

  async loadEmails(accountId: number, folder: string, page = 1) {
    this.loading = true;
    this.currentFolder = folder;
    try {
      const resp = await invoke<EmailListResponse>('list_emails', {
        accountId,
        folder,
        page,
        limit: this.limit,
      });
      this.emails = resp.emails;
      this.total = resp.total;
      this.page = resp.page;
    } catch (e: any) {
      console.error('Failed to load emails:', e);
    } finally {
      this.loading = false;
    }
  }

  async selectEmail(id: number) {
    this.selectedEmailId = id;
    try {
      this.selectedEmail = await invoke<EmailDetail>('get_email', { id });
      // 自动标记已读
      if (!this.selectedEmail.email.is_read) {
        await invoke('mark_as_read', { emailId: id, isRead: true });
        const email = this.emails.find(e => e.id === id);
        if (email) email.is_read = true;
      }
    } catch (e: any) {
      console.error('Failed to load email:', e);
    }
  }

  deselectEmail() {
    this.selectedEmailId = null;
    this.selectedEmail = null;
  }

  async toggleStar(emailId: number) {
    try {
      const newState = await invoke<boolean>('toggle_star', { emailId });
      const email = this.emails.find(e => e.id === emailId);
      if (email) email.is_starred = newState;
      if (this.selectedEmail?.email.id === emailId) {
        this.selectedEmail.email.is_starred = newState;
      }
    } catch (e: any) {
      console.error('Failed to toggle star:', e);
    }
  }

  async deleteEmails(ids: number[]) {
    try {
      await invoke('delete_emails', { emailIds: ids });
      this.emails = this.emails.filter(e => !ids.includes(e.id));
      if (this.selectedEmailId && ids.includes(this.selectedEmailId)) {
        this.deselectEmail();
      }
      this.total -= ids.length;
    } catch (e: any) {
      console.error('Failed to delete emails:', e);
    }
  }
}

const EMAIL_KEY = Symbol('email');

export function createEmailState() {
  const state = new EmailState();
  setContext(EMAIL_KEY, state);
  return state;
}

export function getEmailState() {
  return getContext<EmailState>(EMAIL_KEY);
}

export type { EmailDto, EmailDetail };
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/lib/stores/email.svelte.ts
git commit -m "feat: implement Email store with CRUD operations"
```

---

### Task 3.10: 实现 Sync Store

**Files:**
- Create: `src/lib/stores/sync.svelte.ts`

**Step 1: 写 Sync Store**

```typescript
import { getContext, setContext } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface SyncProgress {
  account_id: number;
  stage: string;
  folder: string | null;
  current: number;
  total: number;
  message: string;
}

interface FolderStat {
  folder: string;
  total: number;
  unread: number;
}

class SyncState {
  syncing = $state(false);
  progress = $state<SyncProgress | null>(null);
  folderStats = $state<FolderStat[]>([]);
  error = $state<string | null>(null);

  constructor() {
    // 监听同步进度事件
    listen<SyncProgress>('sync-progress', (event) => {
      this.progress = event.payload;
      if (event.payload.stage === 'Completed') {
        this.syncing = false;
      } else if (event.payload.stage === 'Error') {
        this.syncing = false;
        this.error = event.payload.message;
      }
    });
  }

  async syncAccount(accountId: number) {
    this.syncing = true;
    this.error = null;
    try {
      await invoke('sync_account', { accountId });
    } catch (e: any) {
      this.syncing = false;
      this.error = e.toString();
    }
  }

  async loadFolderStats(accountId: number) {
    try {
      this.folderStats = await invoke<FolderStat[]>('get_folder_stats', { accountId });
    } catch (e: any) {
      console.error('Failed to load folder stats:', e);
    }
  }
}

const SYNC_KEY = Symbol('sync');

export function createSyncState() {
  const state = new SyncState();
  setContext(SYNC_KEY, state);
  return state;
}

export function getSyncState() {
  return getContext<SyncState>(SYNC_KEY);
}
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 编译通过

**Step 3: Commit**

```bash
git add src/lib/stores/sync.svelte.ts
git commit -m "feat: implement Sync store with progress event listener"
```

---

### Task 3.11: 添加 Glassmorphism 背景效果

**Files:**
- Modify: `src/app.css`
- Modify: `src/routes/+layout.svelte`

**Step 1: 在 app.css 添加渐变光球**

在 `@layer base` 之前添加：

```css
/* Glassmorphism background orbs */
.bg-orbs::before,
.bg-orbs::after {
  content: '';
  position: fixed;
  border-radius: 50%;
  filter: blur(80px);
  opacity: 0.3;
  pointer-events: none;
  z-index: -1;
}

.bg-orbs::before {
  width: 400px;
  height: 400px;
  background: hsl(220 70% 60%);
  top: -100px;
  right: -100px;
}

.bg-orbs::after {
  width: 300px;
  height: 300px;
  background: hsl(280 60% 60%);
  bottom: -50px;
  left: -50px;
}

.dark .bg-orbs::before {
  opacity: 0.15;
  background: hsl(220 80% 40%);
}

.dark .bg-orbs::after {
  opacity: 0.15;
  background: hsl(280 70% 40%);
}
```

**Step 2: 在 layout 添加 bg-orbs class**

更新 `+layout.svelte` 外层 div：
```svelte
<div class="bg-orbs flex h-screen flex-col overflow-hidden bg-background text-foreground">
```

**Step 3: 验证**

Run: `bun run tauri dev`
Expected: 背景有模糊渐变光球效果，面板半透明毛玻璃效果

**Step 4: Commit**

```bash
git add src/app.css src/routes/+layout.svelte
git commit -m "feat: add Glassmorphism background orbs effect"
```

---

### Task 3.12: 最终验证 + 修复

**Step 1: 完整启动**

Run: `bun run tauri dev`

**Step 2: 检查布局**

- TitleBar 40px，可拖拽，按钮可用
- Sidebar 240px，文件夹可点击高亮
- EmailList 320px，搜索栏可见
- EmailDetail 自适应，空状态图标
- StatusBar 32px，同步状态

**Step 3: 检查主题切换**

在浏览器控制台测试：
```js
localStorage.setItem('postium-theme', 'dark')
location.reload()
```
Expected: 切换到深色主题

**Step 4: 修复问题**

根据测试结果修复。

**Step 5: 最终 Commit**

```bash
git add -A
git commit -m "feat: Phase 3 complete - frontend infrastructure with 3-panel layout, stores, theme, i18n"
```
