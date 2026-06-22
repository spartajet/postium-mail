<!--
  Postium Mail - 工作流页面组件
  +page.svelte

  本组件是应用的工作流自动化配置页面，提供可视化流程编辑功能。

  ==================== 功能说明 ====================
  1. 可视化工作流编辑器：使用节点-边图表示工作流
  2. 支持三种节点类型：触发器、条件、操作
  3. 节点拖拽和连接（当前为静态演示）
  4. 节点选择和详情查看
  5. 工作流节点管理（运行、暂停、删除）

  ==================== 组件状态 ====================
  - nodes: 工作流节点数组
  - edges: 节点间的连接关系
  - selectedNode: 当前选中的节点ID
  - typeColors: 不同类型节点的颜色配置
  - typeLabels: 不同类型节点的显示标签

  ==================== 交互说明 ====================
  - 点击节点：选中节点并显示详情面板
  - 再次点击已选中节点：取消选中
  - 点击运行/暂停按钮：执行/暂停节点（占位符）
  - 点击删除按钮：删除节点（占位符）

  ==================== 技术实现 ====================
  使用 Svelte 5 的 Runes API：
  - $state: 定义响应式状态
  - $derived: 定义派生状态
  使用 SVG 绘制节点间的连接线
-->
<script lang="ts">
    // 导入国际化状态管理
    import { getI18nState } from "$lib/stores/i18n.svelte";
    // 导入图标组件
    import { Plus, Play, Pause, Trash2 } from "lucide-svelte";

    // 获取国际化状态实例
    const i18n = getI18nState();
    // 派生翻译函数，自动响应语言变化
    const t = $derived(i18n.t);

    // 工作流节点接口定义
    interface WorkflowNode {
        id: string; // 节点唯一标识
        type: "trigger" | "condition" | "action"; // 节点类型
        title: string; // 节点标题
        description: string; // 节点描述
        position: { x: number; y: number }; // 节点在画布上的位置
    }

    // 工作流连接边接口定义
    interface WorkflowEdge {
        from: string; // 起始节点ID
        to: string; // 目标节点ID
        label?: string; // 连接线上的标签（可选）
    }

    // 工作流节点数据（演示数据）
    let nodes = $state<WorkflowNode[]>([
        {
            id: "1",
            type: "trigger",
            title: "收到新邮件",
            description: "当收件箱收到新邮件时触发",
            position: { x: 250, y: 40 },
        },
        {
            id: "2",
            type: "condition",
            title: "判断发件人",
            description: "检查发件人是否在联系人中",
            position: { x: 120, y: 160 },
        },
        {
            id: "3",
            type: "action",
            title: "标记星标",
            description: "自动标记为星标邮件",
            position: { x: 380, y: 280 },
        },
    ]);

    // 工作流连接边数据（演示数据）
    let edges = $state<WorkflowEdge[]>([
        { from: "1", to: "2" },
        { from: "2", to: "3", label: "是" },
    ]);

    // 当前选中的节点ID，null表示未选中
    let selectedNode = $state<string | null>(null);

    // 不同类型节点的颜色样式配置
    const typeColors: Record<string, string> = {
        trigger: "border-green-500 bg-green-500/5", // 触发器：绿色
        condition: "border-amber-500 bg-amber-500/5", // 条件：橙色
        action: "border-blue-500 bg-blue-500/5", // 操作：蓝色
    };

    // 不同类型节点的显示标签
    const typeLabels: Record<string, string> = {
        trigger: "触发器",
        condition: "条件",
        action: "操作",
    };

    /**
     * 根据节点ID查找节点对象
     * @param id - 节点ID
     * @returns 找到的节点对象，未找到则返回undefined
     */
    function getNodeById(id: string): WorkflowNode | undefined {
        return nodes.find((n) => n.id === id);
    }
</script>

<div class="flex h-full w-full flex-col">
    <!-- Header -->
    <div
        class="flex items-center justify-between border-b border-border px-6 py-4"
    >
        <div>
            <h1 class="text-lg font-semibold text-foreground">工作流</h1>
            <p class="text-sm text-muted-foreground">创建自动化邮件处理规则</p>
        </div>
        <div class="flex items-center gap-2">
            <!-- 新建规则按钮 -->
            <button
                class="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-sm font-medium text-white transition-colors hover:bg-primary/90"
            >
                <Plus size={16} />
                新建规则
            </button>
        </div>
    </div>

    <!-- Canvas: 工作流画布区域 -->
    <div class="relative flex-1 overflow-auto bg-background">
        <!-- SVG Edges: 使用SVG绘制节点间的连接线 -->
        <!-- pointer-events-none: 使SVG不响应鼠标事件，允许点击下方的节点 -->
        <svg class="pointer-events-none absolute inset-0 h-full w-full">
            {#each edges as edge}
                <!-- 获取连接的起始和目标节点 -->
                {@const fromNode = getNodeById(edge.from)}
                {@const toNode = getNodeById(edge.to)}
                {#if fromNode && toNode}
                    <!-- 绘制连接线：从上节点底部中心到下节点顶部中心 -->
                    <!-- x1/y1: 起点坐标（节点宽度180，高度80，加上偏移） -->
                    <!-- x2/y2: 终点坐标 -->
                    <line
                        x1={fromNode.position.x + 80}
                        y1={fromNode.position.y + 40}
                        x2={toNode.position.x + 80}
                        y2={toNode.position.y}
                        stroke="var(--color-border)"
                        stroke-width="2"
                    />
                    <!-- 连接线上的标签（如"是"、"否"等条件判断结果） -->
                    {#if edge.label}
                        <text
                            x={(fromNode.position.x + toNode.position.x) / 2 +
                                75}
                            y={(fromNode.position.y + toNode.position.y) / 2 +
                                25}
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

        <!-- Nodes: 工作流节点渲染 -->
        {#each nodes as node (node.id)}
            <button
                class="workflow-node absolute flex w-45 flex-col rounded-lg border-2 p-3 text-left transition-all {typeColors[
                    node.type
                ]} {selectedNode === node.id
                    ? 'ring-2 ring-primary ring-offset-2 ring-offset-background'
                    : 'hover:shadow-md'}"
                style="left: {node.position.x}px; top: {node.position.y}px;"
                onclick={() =>
                    (selectedNode = selectedNode === node.id ? null : node.id)}
            >
                <!-- 节点类型标签 -->
                <span
                    class="mb-1 text-[11px] font-medium uppercase tracking-wider opacity-60"
                    >{typeLabels[node.type]}</span
                >
                <!-- 节点标题 -->
                <span class="text-sm font-semibold text-foreground"
                    >{node.title}</span
                >
                <!-- 节点描述 -->
                <span class="mt-0.5 text-xs text-muted-foreground"
                    >{node.description}</span
                >
            </button>
        {/each}
    </div>

    <!-- Detail Panel: 选中节点的详情面板 -->
    {#if selectedNode}
        {@const node = getNodeById(selectedNode)}
        {#if node}
            <div class="border-t border-border bg-card px-6 py-4">
                <div class="flex items-center justify-between">
                    <!-- 节点信息 -->
                    <div>
                        <h3 class="text-sm font-semibold text-foreground">
                            {node.title}
                        </h3>
                        <p class="text-xs text-muted-foreground">
                            {node.description}
                        </p>
                    </div>
                    <!-- 节点操作按钮 -->
                    <div class="flex items-center gap-2">
                        <!-- 运行按钮 -->
                        <button
                            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                        >
                            <Play size={16} />
                        </button>
                        <!-- 暂停按钮 -->
                        <button
                            class="flex h-8 w-8 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-glass-hover hover:text-foreground"
                        >
                            <Pause size={16} />
                        </button>
                        <!-- 删除按钮 -->
                        <button
                            class="flex h-8 w-8 items-center justify-center rounded-md text-destructive transition-colors hover:bg-destructive/10"
                        >
                            <Trash2 size={16} />
                        </button>
                    </div>
                </div>
            </div>
        {/if}
    {/if}
</div>
