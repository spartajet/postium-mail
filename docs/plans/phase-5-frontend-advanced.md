# Phase 5: 前端高级功能

> 前置：Phase 4 完成
> 完成标志：标签 UI 可用，日历视图渲染，工作流编辑器骨架可用，AI 操作 UI 就绪，TipTap 集成

---

### Task 5.1: 集成 TipTap 富文本编辑器

**Files:**
- Create: `src/lib/components/email/RichTextEditor.svelte`

**Step 1: 写 RichTextEditor**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Placeholder from '@tiptap/extension-placeholder';

  let editorEl: HTMLElement;
  let editor: Editor;

  let content = $state('');
  let htmlContent = $state('');

  onMount(() => {
    editor = new Editor({
      element: editorEl,
      extensions: [
        StarterKit,
        Placeholder.configure({
          placeholder: '在此输入邮件内容...',
        }),
      ],
      content: '',
      onUpdate: ({ editor }) => {
        content = editor.getText();
        htmlContent = editor.getHTML();
      },
    });

    return () => {
      editor.destroy();
    };
  });

  export function getHtml(): string {
    return htmlContent;
  }

  export function getText(): string {
    return content;
  }

  export function setContent(html: string) {
    editor?.commands.setContent(html);
  }
</script>

<div class="flex h-full flex-col">
  <!-- Toolbar -->
  <div class="flex items-center gap-1 border-b border-border px-3 py-1.5">
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleBold().run()}
      class:bg-muted={editor?.isActive('bold')}
      title="粗体"
    >
      <span class="text-xs font-bold">B</span>
    </button>
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleItalic().run()}
      class:bg-muted={editor?.isActive('italic')}
      title="斜体"
    >
      <span class="text-xs italic">I</span>
    </button>
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleStrike().run()}
      class:bg-muted={editor?.isActive('strike')}
      title="删除线"
    >
      <span class="text-xs line-through">S</span>
    </button>
    <div class="mx-1 h-4 w-px bg-border"></div>
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
      class:bg-muted={editor?.isActive('bulletList')}
      title="无序列表"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="8" y1="6" x2="21" y2="6" /><line x1="8" y1="12" x2="21" y2="12" /><line x1="8" y1="18" x2="21" y2="18" />
        <circle cx="4" cy="6" r="1" fill="currentColor" /><circle cx="4" cy="12" r="1" fill="currentColor" /><circle cx="4" cy="18" r="1" fill="currentColor" />
      </svg>
    </button>
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleOrderedList().run()}
      class:bg-muted={editor?.isActive('orderedList')}
      title="有序列表"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="10" y1="6" x2="21" y2="6" /><line x1="10" y1="12" x2="21" y2="12" /><line x1="10" y1="18" x2="21" y2="18" />
        <text x="3" y="8" font-size="8" fill="currentColor" stroke="none">1</text>
        <text x="3" y="14" font-size="8" fill="currentColor" stroke="none">2</text>
        <text x="3" y="20" font-size="8" fill="currentColor" stroke="none">3</text>
      </svg>
    </button>
    <div class="mx-1 h-4 w-px bg-border"></div>
    <button
      class="rounded p-1 text-muted-foreground transition-colors hover:bg-muted"
      onclick={() => editor?.chain().focus().toggleBlockquote().run()}
      class:bg-muted={editor?.isActive('blockquote')}
      title="引用"
    >
      <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M3 21c3 0 7-1 7-8V5c0-1.25-.756-2.017-2-2H4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2 1 0 1 0 1 1v1c0 1-1 2-2 2s-1 .008-1 1.031V21z" />
        <path d="M15 21c3 0 7-1 7-8V5c0-1.25-.757-2.017-2-2h-4c-1.25 0-2 .75-2 1.972V11c0 1.25.75 2 2 2h.75c0 2.25.25 4-2.75 4v3z" />
      </svg>
    </button>
  </div>

  <!-- Editor area -->
  <div class="flex-1 overflow-y-auto">
    <div bind:this={editorEl} class="prose prose-sm max-w-none p-4 dark:prose-invert"></div>
  </div>
</div>

<style>
  :global(.tiptap) {
    outline: none;
    min-height: 100%;
  }

  :global(.tiptap p.is-editor-empty:first-child::before) {
    content: attr(data-placeholder);
    float: left;
    color: hsl(var(--muted-foreground));
    pointer-events: none;
    height: 0;
  }
</style>
```

**Step 2: 在 ComposeModal 中替换 textarea**

更新 ComposeModal 使用 RichTextEditor：

```svelte
<script lang="ts">
  import RichTextEditor from './RichTextEditor.svelte';
  let richEditor: RichTextEditor;
</script>

<!-- 替换 textarea 为 -->
<RichTextEditor bind:this={richEditor} />

<!-- handleSend 中获取内容 -->
let bodyHtml = richEditor?.getHtml() || '';
let bodyText = richEditor?.getText() || '';
```

**Step 3: 验证**

Run: `bun run dev`
Expected: 富文本编辑器可输入，工具栏可操作粗体/斜体/列表

**Step 4: Commit**

```bash
git add src/lib/components/email/RichTextEditor.svelte src/lib/components/email/ComposeModal.svelte
git commit -m "feat: integrate TipTap rich text editor into compose modal"
```

---

### Task 5.2: 实现标签 UI

**Files:**
- Create: `src/lib/components/labels/LabelBadge.svelte`
- Modify: `src/lib/components/layout/Sidebar.svelte` (添加标签列表)

**Step 1: 写 LabelBadge**

`src/lib/components/labels/LabelBadge.svelte`:
```svelte
<script lang="ts">
  let { label, removable = false, onremove }: {
    label: { id: number; name: string; color: string };
    removable?: boolean;
    onremove?: () => void;
  } = $props();
</script>

<span
  class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
  style="background-color: {label.color}20; color: {label.color}"
>
  <span class="h-1.5 w-1.5 rounded-full" style="background-color: {label.color}"></span>
  {label.name}
  {#if removable}
    <button class="ml-0.5 hover:opacity-70" onclick={onremove}>
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
      </svg>
    </button>
  {/if}
</span>
```

**Step 2: 在 Sidebar 添加标签列表**

在 Sidebar.svelte 的文件夹列表下方添加标签区域：

```svelte
<!-- Labels -->
<div class="mt-3 px-3">
  <div class="flex items-center justify-between">
    <span class="text-xs font-medium text-muted-foreground">{t.sidebar.labels}</span>
    <button class="rounded p-0.5 text-muted-foreground hover:bg-muted">
      <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
      </svg>
    </button>
  </div>
  <div class="mt-2 flex flex-wrap gap-1.5">
    {#each mockLabels as label}
      <span
        class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium hover:opacity-80 cursor-pointer"
        style="background-color: {label.color}20; color: {label.color}"
      >
        <span class="h-1.5 w-1.5 rounded-full" style="background-color: {label.color}"></span>
        {label.name}
      </span>
    {/each}
  </div>
</div>
```

**Step 3: 验证**

Run: `bun run dev`
Expected: 侧边栏显示彩色标签列表

**Step 4: Commit**

```bash
git add src/lib/components/labels/ src/lib/components/layout/Sidebar.svelte
git commit -m "feat: add label badges and label list in sidebar"
```

---

### Task 5.3: 实现日历视图页面

**Files:**
- Create: `src/routes/calendar/+page.svelte`

**Step 1: 写日历页面**

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  const i18n = getI18nState();
  const t = $derived(i18n.t);

  let currentDate = $state(new Date());
  let viewMode = $state<'month' | 'week'>('month');

  const daysOfWeek = ['日', '一', '二', '三', '四', '五', '六'];
  const monthNames = ['一月', '二月', '三月', '四月', '五月', '六月', '七月', '八月', '九月', '十月', '十一月', '十二月'];

  // 计算日历网格
  const calendarDays = $derived.by(() => {
    const year = currentDate.getFullYear();
    const month = currentDate.getMonth();
    const firstDay = new Date(year, month, 1).getDay();
    const daysInMonth = new Date(year, month + 1, 0).getDate();
    const daysInPrevMonth = new Date(year, month, 0).getDate();

    const days: { date: number; month: number; year: number; isCurrentMonth: boolean; hasEvent: boolean }[] = [];

    // 上月补齐
    for (let i = firstDay - 1; i >= 0; i--) {
      days.push({
        date: daysInPrevMonth - i,
        month: month - 1,
        year,
        isCurrentMonth: false,
        hasEvent: false,
      });
    }

    // 当月
    for (let i = 1; i <= daysInMonth; i++) {
      days.push({
        date: i,
        month,
        year,
        isCurrentMonth: true,
        hasEvent: Math.random() > 0.8, // Mock: 随机事件
      });
    }

    // 下月补齐
    const remaining = 42 - days.length;
    for (let i = 1; i <= remaining; i++) {
      days.push({
        date: i,
        month: month + 1,
        year,
        isCurrentMonth: false,
        hasEvent: false,
      });
    }

    return days;
  });

  function prevMonth() {
    currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
  }

  function nextMonth() {
    currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
  }

  function goToToday() {
    currentDate = new Date();
  }

  const today = new Date();
</script>

<div class="flex h-full flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between border-b border-border px-6 py-4">
    <div class="flex items-center gap-4">
      <h1 class="text-xl font-semibold">
        {monthNames[currentDate.getMonth()]} {currentDate.getFullYear()}
      </h1>
      <div class="flex items-center gap-1">
        <button class="rounded-md p-1.5 text-muted-foreground hover:bg-muted" onclick={prevMonth}>
          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6" /></svg>
        </button>
        <button class="rounded-md p-1.5 text-muted-foreground hover:bg-muted" onclick={nextMonth}>
          <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 18 15 12 9 6" /></svg>
        </button>
      </div>
    </div>
    <div class="flex items-center gap-2">
      <button
        class="rounded-md px-3 py-1 text-xs {viewMode === 'month' ? 'bg-muted font-medium' : 'text-muted-foreground hover:bg-muted'}"
        onclick={() => viewMode = 'month'}
      >
        月
      </button>
      <button
        class="rounded-md px-3 py-1 text-xs {viewMode === 'week' ? 'bg-muted font-medium' : 'text-muted-foreground hover:bg-muted'}"
        onclick={() => viewMode = 'week'}
      >
        周
      </button>
      <button class="rounded-md px-3 py-1 text-xs text-muted-foreground hover:bg-muted" onclick={goToToday}>
        今天
      </button>
    </div>
  </div>

  <!-- Calendar Grid -->
  <div class="flex-1 p-4">
    <!-- Day headers -->
    <div class="grid grid-cols-7 gap-px">
      {#each daysOfWeek as day}
        <div class="py-2 text-center text-xs font-medium text-muted-foreground">{day}</div>
      {/each}
    </div>

    <!-- Date cells -->
    <div class="grid grid-cols-7 gap-px">
      {#each calendarDays as day}
        <div
          class="flex min-h-[80px] flex-col border border-border/50 p-1.5 {day.isCurrentMonth ? 'bg-background' : 'bg-muted/30'}"
        >
          <span
            class="text-xs {day.isCurrentMonth ? 'text-foreground' : 'text-muted-foreground'} {day.date === today.getDate() && day.month === today.getMonth() && day.year === today.getFullYear() ? 'flex h-5 w-5 items-center justify-center rounded-full bg-primary text-primary-foreground font-medium' : ''}"
          >
            {day.date}
          </span>
          {#if day.hasEvent && day.isCurrentMonth}
            <div class="mt-1 rounded bg-primary/10 px-1 py-0.5 text-[10px] text-primary">
              邮件事件
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev` → 导航到 `/calendar`
Expected: 月视图日历网格，可翻月

**Step 3: Commit**

```bash
git add src/routes/calendar/
git commit -m "feat: implement calendar view with month grid"
```

---

### Task 5.4: 实现工作流编辑器页面

**Files:**
- Create: `src/routes/workflow/+page.svelte`

**Step 1: 写工作流页面**

```svelte
<script lang="ts">
  interface WorkflowNode {
    id: string;
    type: 'trigger' | 'condition' | 'action';
    title: string;
    description: string;
    position: { x: number; y: number };
  }

  interface WorkflowEdge {
    from: string;
    to: string;
    label?: string;
  }

  let nodes = $state<WorkflowNode[]>([
    { id: '1', type: 'trigger', title: '收到新邮件', description: '当收件箱收到新邮件时触发', position: { x: 250, y: 40 } },
    { id: '2', type: 'condition', title: '判断发件人', description: '检查发件人是否在联系人中', position: { x: 250, y: 160 } },
    { id: '3', type: 'action', title: '标记星标', description: '自动标记为星标邮件', position: { x: 120, y: 280 } },
    { id: '4', type: 'action', title: '移动到文件夹', description: '移动到指定文件夹', position: { x: 380, y: 280 } },
  ]);

  let edges = $state<WorkflowEdge[]>([
    { from: '1', to: '2' },
    { from: '2', to: '3', label: '是' },
    { from: '2', to: '4', label: '否' },
  ]);

  let selectedNode = $state<string | null>(null);

  const typeColors: Record<string, string> = {
    trigger: 'border-green-500 bg-green-500/5',
    condition: 'border-amber-500 bg-amber-500/5',
    action: 'border-blue-500 bg-blue-500/5',
  };

  const typeLabels: Record<string, string> = {
    trigger: '触发器',
    condition: '条件',
    action: '操作',
  };
</script>

<div class="flex h-full flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between border-b border-border px-6 py-4">
    <div>
      <h1 class="text-xl font-semibold">工作流编辑器</h1>
      <p class="text-xs text-muted-foreground">自动化邮件处理规则（功能开发中）</p>
    </div>
    <div class="flex items-center gap-2">
      <button class="rounded-md border border-input px-3 py-1.5 text-xs hover:bg-muted">新建规则</button>
    </div>
  </div>

  <!-- Canvas -->
  <div class="flex-1 overflow-auto bg-muted/20 p-8">
    <div class="relative" style="width: 600px; height: 400px;">
      <!-- Edges -->
      <svg class="pointer-events-none absolute inset-0 h-full w-full">
        {#each edges as edge}
          {@const fromNode = nodes.find(n => n.id === edge.from)}
          {@const toNode = nodes.find(n => n.id === edge.to)}
          {#if fromNode && toNode}
            {@const x1 = fromNode.position.x + 80}
            {@const y1 = fromNode.position.y + 40}
            {@const x2 = toNode.position.x + 80}
            {@const y2 = toNode.position.y}
            <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="currentColor" stroke-width="1.5" class="text-border" />
            {#if edge.label}
              <text x={(x1 + x2) / 2} y={(y1 + y2) / 2 - 6} text-anchor="middle" class="fill-muted-foreground text-xs">{edge.label}</text>
            {/if}
          {/if}
        {/each}
      </svg>

      <!-- Nodes -->
      {#each nodes as node (node.id)}
        <div
          class="absolute w-40 rounded-lg border-2 bg-card p-3 shadow-sm transition-shadow hover:shadow-md cursor-pointer {typeColors[node.type]} {selectedNode === node.id ? 'ring-2 ring-ring' : ''}"
          style="left: {node.position.x}px; top: {node.position.y}px;"
          onclick={() => selectedNode = node.id}
        >
          <div class="mb-1 text-[10px] font-medium uppercase text-muted-foreground">{typeLabels[node.type]}</div>
          <div class="text-sm font-medium">{node.title}</div>
          <div class="mt-1 text-xs text-muted-foreground">{node.description}</div>
        </div>
      {/each}
    </div>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev` → 导航到 `/workflow`
Expected: 工作流编辑器渲染，节点可点击选中

**Step 3: Commit**

```bash
git add src/routes/workflow/
git commit -m "feat: implement workflow editor page with visual node graph"
```

---

### Task 5.5: 实现 AI 操作 UI (EmailDetail 中)

**Files:**
- Modify: `src/lib/components/email/EmailDetail.svelte`

**Step 1: 在 EmailDetail 添加 AI 操作区域**

在邮件正文下方添加 AI 操作面板：

```svelte
<!-- AI Actions -->
<div class="border-t border-border p-4">
  <div class="flex items-center gap-2 text-xs text-muted-foreground">
    <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M12 2a4 4 0 014 4c0 1.95-1.4 3.58-3.25 3.93V12h3.75a2.5 2.5 0 012.5 2.5v1.75a1.25 1.25 0 110 2.5V19a2.5 2.5 0 01-2.5 2.5h-9A2.5 2.5 0 013.75 19v-2.75a1.25 1.25 0 110-2.5V12a2.5 2.5 0 012.5-2.5h3.75V9.93A4.002 4.002 0 0112 2z" />
    </svg>
    <span>AI 助手</span>
  </div>

  <!-- AI Summary card (stub) -->
  <div class="mt-3 rounded-lg border border-border bg-muted/30 p-3">
    <div class="flex items-center gap-2 text-xs font-medium text-muted-foreground">
      <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <line x1="9" y1="9" x2="15" y2="9" />
        <line x1="9" y1="13" x2="15" y2="13" />
      </svg>
      智能摘要
    </div>
    <p class="mt-2 text-xs text-muted-foreground italic">此功能开发中，将由 AI 自动生成邮件摘要</p>
  </div>

  <!-- AI action buttons -->
  <div class="mt-3 flex flex-wrap gap-2">
    <button class="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted">
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" /></svg>
      智能回复
    </button>
    <button class="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted">
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" /><polyline points="14 2 14 8 20 8" /></svg>
      摘要
    </button>
    <button class="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted">
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 8l6 6 6-6" /><line x1="3" y1="12" x2="21" y2="12" /></svg>
      翻译
    </button>
    <button class="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted">
      <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="9 11 12 14 22 4" /><path d="M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11" /></svg>
      提取任务
    </button>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev`
Expected: 邮件详情底部显示 AI 操作区

**Step 3: Commit**

```bash
git add src/lib/components/email/EmailDetail.svelte
git commit -m "feat: add AI actions UI stub to email detail"
```

---

### Task 5.6: 实现设置页面

**Files:**
- Create: `src/routes/settings/+page.svelte`

**Step 1: 写设置页面**

```svelte
<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { getThemeState } from '$lib/stores/theme.svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);
  const themeState = getThemeState();
</script>

<div class="flex h-full">
  <!-- Settings Sidebar -->
  <div class="w-48 border-r border-border p-4">
    <h2 class="mb-4 text-sm font-semibold">{t.settings.title}</h2>
    <nav class="space-y-1">
      <button class="flex w-full rounded-md px-3 py-1.5 text-left text-sm bg-muted font-medium">{t.settings.general}</button>
      <button class="flex w-full rounded-md px-3 py-1.5 text-left text-sm text-muted-foreground hover:bg-muted">{t.settings.accounts}</button>
    </nav>
  </div>

  <!-- Settings Content -->
  <div class="flex-1 overflow-y-auto p-6">
    <h3 class="text-lg font-semibold">{t.settings.appearance}</h3>

    <!-- Theme -->
    <div class="mt-6">
      <label class="text-sm font-medium">{t.settings.theme}</label>
      <div class="mt-2 flex gap-2">
        <button
          class="rounded-md border px-4 py-2 text-sm {themeState.theme === 'light' ? 'border-primary bg-primary/5' : 'border-input hover:bg-muted'}"
          onclick={() => themeState.setTheme('light')}
        >
          {t.settings.light}
        </button>
        <button
          class="rounded-md border px-4 py-2 text-sm {themeState.theme === 'dark' ? 'border-primary bg-primary/5' : 'border-input hover:bg-muted'}"
          onclick={() => themeState.setTheme('dark')}
        >
          {t.settings.dark}
        </button>
        <button
          class="rounded-md border px-4 py-2 text-sm {themeState.theme === 'system' ? 'border-primary bg-primary/5' : 'border-input hover:bg-muted'}"
          onclick={() => themeState.setTheme('system')}
        >
          {t.settings.system}
        </button>
      </div>
    </div>

    <!-- Language -->
    <div class="mt-6">
      <label class="text-sm font-medium">{t.settings.language}</label>
      <div class="mt-2 flex gap-2">
        <button
          class="rounded-md border px-4 py-2 text-sm {i18n.locale === 'zh-CN' ? 'border-primary bg-primary/5' : 'border-input hover:bg-muted'}"
          onclick={() => i18n.setLocale('zh-CN')}
        >
          中文
        </button>
        <button
          class="rounded-md border px-4 py-2 text-sm {i18n.locale === 'en-US' ? 'border-primary bg-primary/5' : 'border-input hover:bg-muted'}"
          onclick={() => i18n.setLocale('en-US')}
        >
          English
        </button>
      </div>
    </div>
  </div>
</div>
```

**Step 2: 验证**

Run: `bun run dev` → 导航到 `/settings`
Expected: 设置页面可切换主题和语言

**Step 3: Commit**

```bash
git add src/routes/settings/
git commit -m "feat: implement settings page with theme and language switching"
```

---

### Task 5.7: 最终验证 + 修复

**Step 1: 完整构建**

Run: `bun run build`

**Step 2: 运行测试**

Run: `bun run tauri dev`

验证：
- TipTap 编辑器可输入富文本
- 标签显示在侧边栏
- `/calendar` 日历视图可翻月
- `/workflow` 工作流编辑器节点可点击
- 邮件详情 AI 操作区域可见
- `/settings` 可切换主题/语言

**Step 3: 修复问题**

**Step 4: 最终 Commit**

```bash
git add -A
git commit -m "feat: Phase 5 complete - rich editor, calendar, workflow, AI UI, labels, settings"
```
