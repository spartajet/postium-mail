<script lang="ts">
  import { getI18nState } from '$lib/stores/i18n.svelte';
  import { Plus, Play, Pause, Trash2 } from 'lucide-svelte';

  const i18n = getI18nState();
  const t = $derived(i18n.t);

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
    { id: '2', type: 'condition', title: '判断发件人', description: '检查发件人是否在联系人中', position: { x: 120, y: 160 } },
    { id: '3', type: 'action', title: '标记星标', description: '自动标记为星标邮件', position: { x: 380, y: 280 } },
  ]);

  let edges = $state<WorkflowEdge[]>([
    { from: '1', to: '2' },
    { from: '2', to: '3', label: '是' },
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

  function getNodeById(id: string): WorkflowNode | undefined {
    return nodes.find(n => n.id === id);
  }
</script>

<div class="flex h-full w-full flex-col">
  <!-- Header -->
  <div class="flex items-center justify-between border-b border-border px-6 py-4">
    <div>
      <h1 class="text-lg font-semibold text-foreground">工作流</h1>
      <p class="text-sm text-muted-foreground">创建自动化邮件处理规则</p>
    </div>
    <div class="flex items-center gap-2">
      <button class="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-primary/90">
        <Plus size={16} />
        新建规则
      </button>
    </div>
  </div>

  <!-- Canvas -->
  <div class="relative flex-1 overflow-auto bg-background">
    <!-- SVG Edges -->
    <svg class="pointer-events-none absolute inset-0 h-full w-full">
      {#each edges as edge}
        {@const fromNode = getNodeById(edge.from)}
        {@const toNode = getNodeById(edge.to)}
        {#if fromNode && toNode}
          <line
            x1={fromNode.position.x + 80}
            y1={fromNode.position.y + 40}
            x2={toNode.position.x + 80}
            y2={toNode.position.y}
            stroke="var(--color-border)"
            stroke-width="2"
          />
          {#if edge.label}
            <text
              x={(fromNode.position.x + toNode.position.x) / 2 + 75}
              y={(fromNode.position.y + toNode.position.y) / 2 + 25}
              fill="var(--color-muted-foreground)"
              font-size="12"
              text-anchor="middle"
            >
              {edge.label}
            </text>
          {/if}
        {/if}
      {/each}
    </svg>

    <!-- Nodes -->
    {#each nodes as node (node.id)}
      <button
        class="workflow-node absolute flex w-[180px] flex-col rounded-lg border-2 p-3 text-left transition-all {typeColors[node.type]} {selectedNode === node.id ? 'ring-2 ring-primary ring-offset-2 ring-offset-background' : 'hover:shadow-md'}"
        style="left: {node.position.x}px; top: {node.position.y}px;"
        onclick={() => selectedNode = selectedNode === node.id ? null : node.id}
      >
        <span class="mb-1 text-[11px] font-medium uppercase tracking-wider opacity-60">{typeLabels[node.type]}</span>
        <span class="text-sm font-semibold text-foreground">{node.title}</span>
        <span class="mt-0.5 text-xs text-muted-foreground">{node.description}</span>
      </button>
    {/each}
  </div>

  <!-- Detail Panel -->
  {#if selectedNode}
    {@const node = getNodeById(selectedNode)}
    {#if node}
      <div class="border-t border-border bg-card px-6 py-4">
        <div class="flex items-center justify-between">
          <div>
            <h3 class="text-sm font-semibold text-foreground">{node.title}</h3>
            <p class="text-xs text-muted-foreground">{node.description}</p>
          </div>
          <div class="flex items-center gap-2">
            <button class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground">
              <Play size={16} />
            </button>
            <button class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground">
              <Pause size={16} />
            </button>
            <button class="flex h-8 w-8 items-center justify-center rounded-md text-destructive transition-colors hover:bg-destructive/10">
              <Trash2 size={16} />
            </button>
          </div>
        </div>
      </div>
    {/if}
  {/if}
</div>
