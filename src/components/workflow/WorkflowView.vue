<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useWorkflowStore, useUIStore } from "@/stores";
import type { NodeType, WorkflowNode } from "@/types";

// Stores
const workflowStore = useWorkflowStore();
const uiStore = useUIStore();

// 画布状态
const canvasOffset = ref({ x: 0, y: 0 });
const isDraggingCanvas = ref(false);
const lastMousePos = ref({ x: 0, y: 0 });
const zoom = ref(1);

// 选中的节点 ID
const selectedNodeId = ref<string | null>(null);

// 是否显示节点面板
const showNodePanel = ref(true);

// 是否显示配置面板
const showConfigPanel = ref(false);

// 拖拽状态
const isDraggingNode = ref(false);
const draggedNodeType = ref<NodeType | null>(null);

// 节点类型定义
const nodeTypes = computed(() => workflowStore.nodeTypes);

// 选中的节点
const selectedNode = computed(() => workflowStore.selectedNode);

// 工具栏操作
const toolbarActions = [
    {
        id: "save",
        label: "保存",
        icon: "save",
        action: handleSave,
    },
    {
        id: "load",
        label: "加载",
        icon: "folder-open",
        action: handleLoad,
    },
    {
        id: "run",
        label: "运行",
        icon: "play",
        action: handleRun,
    },
    {
        id: "validate",
        label: "验证",
        icon: "check-circle",
        action: handleValidate,
    },
    { id: "divider" },
    {
        id: "export",
        label: "导出",
        icon: "download",
        action: handleExport,
    },
    {
        id: "import",
        label: "导入",
        icon: "upload",
        action: handleImport,
    },
    { id: "divider" },
    {
        id: "clear",
        label: "清空",
        icon: "trash",
        action: handleClear,
        danger: true,
    },
];

// 处理画布拖拽
function handleCanvasMouseDown(event: MouseEvent) {
    if (
        event.target === event.currentTarget ||
        (event.target as HTMLElement).classList.contains("workflow-canvas")
    ) {
        isDraggingCanvas.value = true;
        lastMousePos.value = { x: event.clientX, y: event.clientY };
        selectedNodeId.value = null;
        workflowStore.selectNode(null);
        showConfigPanel.value = false;
    }
}

function handleCanvasMouseMove(event: MouseEvent) {
    if (isDraggingCanvas.value) {
        const deltaX = event.clientX - lastMousePos.value.x;
        const deltaY = event.clientY - lastMousePos.value.y;
        canvasOffset.value.x += deltaX;
        canvasOffset.value.y += deltaY;
        lastMousePos.value = { x: event.clientX, y: event.clientY };
    }
}

function handleCanvasMouseUp() {
    isDraggingCanvas.value = false;
}

// 处理节点拖拽
function handleNodeDragStart(event: DragEvent, nodeType: NodeType) {
    isDraggingNode.value = true;
    draggedNodeType.value = nodeType;
    if (event.dataTransfer) {
        event.dataTransfer.setData("nodeType", nodeType);
        event.dataTransfer.effectAllowed = "copy";
    }
}

function handleCanvasDrop(event: DragEvent) {
    event.preventDefault();
    if (!draggedNodeType.value) return;

    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const x = (event.clientX - rect.left - canvasOffset.value.x) / zoom.value;
    const y = (event.clientY - rect.top - canvasOffset.value.y) / zoom.value;

    const newNode = workflowStore.addNode(draggedNodeType.value, { x, y });
    selectNode(newNode.id);

    isDraggingNode.value = false;
    draggedNodeType.value = null;
}

function handleCanvasDragOver(event: DragEvent) {
    event.preventDefault();
    if (event.dataTransfer) {
        event.dataTransfer.dropEffect = "copy";
    }
}

// 选择节点
function selectNode(nodeId: string | null) {
    selectedNodeId.value = nodeId;
    workflowStore.selectNode(nodeId);
    showConfigPanel.value = nodeId !== null;
}

// 更新节点位置
function updateNodePosition(
    nodeId: string,
    position: { x: number; y: number },
) {
    workflowStore.updateNodePosition(nodeId, position);
}

// 删除节点
function deleteNode(nodeId: string) {
    workflowStore.removeNode(nodeId);
    if (selectedNodeId.value === nodeId) {
        selectedNodeId.value = null;
        showConfigPanel.value = false;
    }
}

// 复制节点
function duplicateNode(nodeId: string) {
    const newNode = workflowStore.duplicateNode(nodeId);
    if (newNode) {
        selectNode(newNode.id);
    }
}

// 工具栏操作
async function handleSave() {
    await workflowStore.saveWorkflow();
    uiStore.showSuccess("工作流已保存");
}

async function handleLoad() {
    await workflowStore.loadWorkflow();
    uiStore.showSuccess("工作流已加载");
}

function handleRun() {
    const validation = workflowStore.validateWorkflow();
    if (!validation.valid) {
        uiStore.showError(validation.errors[0]);
        return;
    }
    uiStore.showInfo("工作流开始运行");
    // TODO: 实现工作流运行逻辑
}

function handleValidate() {
    const validation = workflowStore.validateWorkflow();
    if (validation.valid) {
        uiStore.showSuccess("工作流验证通过");
    } else {
        uiStore.showError(`验证失败: ${validation.errors.join(", ")}`);
    }
}

function handleExport() {
    const json = workflowStore.exportWorkflow();
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `workflow-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
    uiStore.showSuccess("工作流已导出");
}

function handleImport() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";
    input.onchange = async (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = (event) => {
            const json = event.target?.result as string;
            const success = workflowStore.importWorkflow(json);
            if (success) {
                uiStore.showSuccess("工作流已导入");
            } else {
                uiStore.showError("导入失败：文件格式不正确");
            }
        };
        reader.readAsText(file);
    };
    input.click();
}

function handleClear() {
    if (confirm("确定要清空所有节点吗？")) {
        workflowStore.clearWorkflow();
        selectedNodeId.value = null;
        showConfigPanel.value = false;
        uiStore.showSuccess("工作流已清空");
    }
}

// 缩放控制
function handleZoomIn() {
    zoom.value = Math.min(zoom.value + 0.1, 2);
}

function handleZoomOut() {
    zoom.value = Math.max(zoom.value - 0.1, 0.5);
}

function handleZoomReset() {
    zoom.value = 1;
    canvasOffset.value = { x: 0, y: 0 };
}

// 键盘快捷键
function handleKeyDown(event: KeyboardEvent) {
    // Delete 删除选中节点
    if (event.key === "Delete" && selectedNodeId.value) {
        deleteNode(selectedNodeId.value);
    }

    // Ctrl+D 复制节点
    if (event.ctrlKey && event.key === "d" && selectedNodeId.value) {
        event.preventDefault();
        duplicateNode(selectedNodeId.value);
    }

    // Ctrl+S 保存
    if (event.ctrlKey && event.key === "s") {
        event.preventDefault();
        handleSave();
    }

    // Escape 取消选择
    if (event.key === "Escape") {
        selectedNodeId.value = null;
        workflowStore.selectNode(null);
        showConfigPanel.value = false;
    }
}

// 获取节点颜色
function getNodeColor(type: NodeType): string {
    const nodeType = nodeTypes.value.find((n) => n.type === type);
    return nodeType?.color || "#7C3AED";
}

// 获取节点图标
function getNodeIcon(type: NodeType): string {
    const nodeType = nodeTypes.value.find((n) => n.type === type);
    return nodeType?.icon || "circle";
}

// 生命周期
onMounted(() => {
    window.addEventListener("keydown", handleKeyDown);
    workflowStore.loadWorkflow();
});

onUnmounted(() => {
    window.removeEventListener("keydown", handleKeyDown);
});
</script>

<template>
    <div class="workflow-view">
        <!-- 工具栏 -->
        <div class="workflow-toolbar">
            <div class="toolbar-left">
                <h2 class="toolbar-title">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm-1 9h-2v2H9v-2H7v-2h2V7h2v2h2v2zm0-6V3.5L17.5 9H13z"
                        />
                    </svg>
                    <span>工作流编辑器</span>
                </h2>

                <div class="toolbar-actions">
                    <template v-for="action in toolbarActions" :key="action.id">
                        <div
                            v-if="action.id === 'divider'"
                            class="toolbar-divider"
                        ></div>
                        <button
                            v-else
                            :class="['toolbar-btn', { danger: action.danger }]"
                            @click="action.action"
                            :title="action.label"
                        >
                            <svg
                                v-if="action.icon === 'save'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M17 3H5c-1.11 0-2 .9-2 2v14c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V7l-4-4zm-5 16c-1.66 0-3-1.34-3-3s1.34-3 3-3 3 1.34 3 3-1.34 3-3 3zm3-10H5V5h10v4z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'folder-open'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M20 6h-8l-2-2H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 12H4V8h16v10z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'play'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path d="M8 5v14l11-7z" />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'check-circle'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'download'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'upload'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M9 16h6v-6h4l-7-7-7 7h4zm-4 2h14v2H5z"
                                />
                            </svg>
                            <svg
                                v-else-if="action.icon === 'trash'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z"
                                />
                            </svg>
                            <span class="btn-label">{{ action.label }}</span>
                        </button>
                    </template>
                </div>
            </div>

            <div class="toolbar-right">
                <!-- 缩放控制 -->
                <div class="zoom-controls">
                    <button
                        class="icon-btn-sm"
                        @click="handleZoomOut"
                        title="缩小"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path d="M19 13H5v-2h14v2z" />
                        </svg>
                    </button>
                    <span class="zoom-level"
                        >{{ Math.round(zoom * 100) }}%</span
                    >
                    <button
                        class="icon-btn-sm"
                        @click="handleZoomIn"
                        title="放大"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z" />
                        </svg>
                    </button>
                    <button
                        class="icon-btn-sm"
                        @click="handleZoomReset"
                        title="重置"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z"
                            />
                        </svg>
                    </button>
                </div>

                <!-- 节点数量 -->
                <div class="node-count">
                    <span>{{ workflowStore.nodeCount }} 个节点</span>
                </div>
            </div>
        </div>

        <!-- 主内容区域 -->
        <div class="workflow-content">
            <!-- 节点面板 -->
            <div v-if="showNodePanel" class="node-panel">
                <div class="panel-header">
                    <h3>可用节点</h3>
                    <button class="icon-btn-sm" @click="showNodePanel = false">
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                            />
                        </svg>
                    </button>
                </div>

                <div class="node-list">
                    <div
                        v-for="nodeType in nodeTypes"
                        :key="nodeType.type"
                        class="node-type-item"
                        :style="{ borderLeftColor: nodeType.color }"
                        draggable="true"
                        @dragstart="handleNodeDragStart($event, nodeType.type)"
                    >
                        <div
                            class="node-icon"
                            :style="{
                                backgroundColor: nodeType.color + '20',
                                color: nodeType.color,
                            }"
                        >
                            <svg
                                v-if="nodeType.icon === 'bolt'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M11 21h-1l1-7H7.5c-.58 0-.57-.32-.38-.66.19-.34.05-.08.07-.12C8.48 10.94 10.42 7.54 13 3h1l-1 7h3.5c.49 0 .56.33.47.51l-.07.15C12.96 17.55 11 21 11 21z"
                                />
                            </svg>
                            <svg
                                v-else-if="nodeType.icon === 'git-branch'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M6 3v6h1v3H6v6h7v-3h-1v-3h3v-2h-3V8h1V3H6zm9 12h1v3h3v-3h1v-2h-5v2z"
                                />
                            </svg>
                            <svg
                                v-else-if="nodeType.icon === 'play'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path d="M8 5v14l11-7z" />
                            </svg>
                            <svg
                                v-else-if="nodeType.icon === 'mail'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"
                                />
                            </svg>
                            <svg
                                v-else-if="nodeType.icon === 'clock'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8zm.5-13H11v6l5.25 3.15.75-1.23-4.5-2.67z"
                                />
                            </svg>
                            <svg
                                v-else-if="nodeType.icon === 'sparkles'"
                                xmlns="http://www.w3.org/2000/svg"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path
                                    d="M12 2L9.19 8.63 2 9.24l5.46 4.73L5.82 21 12 17.27 18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2z"
                                />
                            </svg>
                        </div>
                        <div class="node-info">
                            <div class="node-label">{{ nodeType.label }}</div>
                            <div class="node-desc">拖拽到画布添加</div>
                        </div>
                    </div>
                </div>

                <div class="panel-footer">
                    <button
                        class="btn btn-ghost btn-sm"
                        @click="showNodePanel = false"
                    >
                        隐藏面板
                    </button>
                </div>
            </div>

            <!-- 画布区域 -->
            <div
                class="workflow-canvas"
                @mousedown="handleCanvasMouseDown"
                @mousemove="handleCanvasMouseMove"
                @mouseup="handleCanvasMouseUp"
                @mouseleave="handleCanvasMouseUp"
                @drop="handleCanvasDrop"
                @dragover="handleCanvasDragOver"
            >
                <!-- 网格背景 -->
                <div class="canvas-grid"></div>

                <!-- 节点容器 -->
                <div
                    class="nodes-container"
                    :style="{
                        transform: `translate(${canvasOffset.x}px, ${canvasOffset.y}px) scale(${zoom})`,
                    }"
                >
                    <!-- 渲染所有节点 -->
                    <div
                        v-for="node in workflowStore.nodes"
                        :key="node.id"
                        :class="[
                            'workflow-node',
                            { selected: selectedNodeId === node.id },
                        ]"
                        :style="{
                            left: node.position.x + 'px',
                            top: node.position.y + 'px',
                            borderLeftColor: getNodeColor(node.type),
                        }"
                        @click.stop="selectNode(node.id)"
                    >
                        <!-- 节点头部 -->
                        <div class="node-header">
                            <div
                                class="node-icon"
                                :style="{
                                    backgroundColor:
                                        getNodeColor(node.type) + '20',
                                    color: getNodeColor(node.type),
                                }"
                            >
                                <svg
                                    v-if="getNodeIcon(node.type) === 'bolt'"
                                    xmlns="http://www.w3.org/2000/svg"
                                    viewBox="0 0 24 24"
                                    fill="currentColor"
                                >
                                    <path
                                        d="M11 21h-1l1-7H7.5c-.58 0-.57-.32-.38-.66.19-.34.05-.08.07-.12C8.48 10.94 10.42 7.54 13 3h1l-1 7h3.5c.49 0 .56.33.47.51l-.07.15C12.96 17.55 11 21 11 21z"
                                    />
                                </svg>
                            </div>
                            <div class="node-title">{{ node.label }}</div>
                            <div class="node-actions">
                                <button
                                    class="icon-btn-xs"
                                    @click.stop="duplicateNode(node.id)"
                                    title="复制"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z"
                                        />
                                    </svg>
                                </button>
                                <button
                                    class="icon-btn-xs danger"
                                    @click.stop="deleteNode(node.id)"
                                    title="删除"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                    >
                                        <path
                                            d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                                        />
                                    </svg>
                                </button>
                            </div>
                        </div>

                        <!-- 节点内容 -->
                        <div class="node-body">
                            <div class="node-type">{{ node.type }}</div>
                            <div class="node-config">
                                <div v-if="node.type === 'trigger'">
                                    <span class="config-label">事件:</span>
                                    <span class="config-value">{{
                                        node.config.event || "email_received"
                                    }}</span>
                                </div>
                                <div v-else-if="node.type === 'condition'">
                                    <span class="config-label">条件:</span>
                                    <span class="config-value"
                                        >{{ node.config.field }}
                                        {{ node.config.operator }}</span
                                    >
                                </div>
                                <div v-else-if="node.type === 'action'">
                                    <span class="config-label">动作:</span>
                                    <span class="config-value">{{
                                        node.config.action
                                    }}</span>
                                </div>
                                <div v-else-if="node.type === 'email'">
                                    <span class="config-label">邮件:</span>
                                    <span class="config-value">{{
                                        node.config.template || "未配置"
                                    }}</span>
                                </div>
                                <div v-else-if="node.type === 'delay'">
                                    <span class="config-label">延迟:</span>
                                    <span class="config-value"
                                        >{{ node.config.duration }}
                                        {{ node.config.unit }}</span
                                    >
                                </div>
                                <div v-else-if="node.type === 'ai'">
                                    <span class="config-label">模型:</span>
                                    <span class="config-value">{{
                                        node.config.model
                                    }}</span>
                                </div>
                            </div>
                        </div>

                        <!-- 连接点 -->
                        <div class="node-connector input" title="输入"></div>
                        <div class="node-connector output" title="输出"></div>
                    </div>
                </div>

                <!-- 空状态 -->
                <div
                    v-if="workflowStore.nodes.length === 0"
                    class="empty-state"
                >
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                    >
                        <path
                            d="M14 2H6c-1.1 0-2 .9-2 2v16c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V8l-6-6zm-1 9h-2v2H9v-2H7v-2h2V7h2v2h2v2zm0-6V3.5L17.5 9H13z"
                        />
                    </svg>
                    <h3>还没有节点</h3>
                    <p>从左侧面板拖拽节点到画布开始创建工作流</p>
                    <button
                        v-if="!showNodePanel"
                        class="btn btn-primary"
                        @click="showNodePanel = true"
                    >
                        显示节点面板
                    </button>
                </div>
            </div>

            <!-- 配置面板 -->
            <div v-if="showConfigPanel && selectedNode" class="config-panel">
                <div class="panel-header">
                    <h3>节点配置</h3>
                    <button
                        class="icon-btn-sm"
                        @click="showConfigPanel = false"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            viewBox="0 0 24 24"
                            fill="currentColor"
                        >
                            <path
                                d="M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z"
                            />
                        </svg>
                    </button>
                </div>

                <div class="config-content">
                    <!-- 触发器配置 -->
                    <div
                        v-if="selectedNode.type === 'trigger'"
                        class="config-section"
                    >
                        <label class="config-label">触发事件</label>
                        <select
                            v-model="selectedNode.config.event"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="email_received">收到邮件</option>
                            <option value="email_sent">发送邮件</option>
                            <option value="email_opened">邮件被打开</option>
                            <option value="scheduled">定时触发</option>
                        </select>
                    </div>

                    <!-- 条件配置 -->
                    <div
                        v-else-if="selectedNode.type === 'condition'"
                        class="config-section"
                    >
                        <label class="config-label">字段</label>
                        <select
                            v-model="selectedNode.config.field"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="subject">主题</option>
                            <option value="sender">发件人</option>
                            <option value="body">正文</option>
                            <option value="priority">优先级</option>
                        </select>

                        <label class="config-label">操作符</label>
                        <select
                            v-model="selectedNode.config.operator"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="contains">包含</option>
                            <option value="equals">等于</option>
                            <option value="starts_with">开头是</option>
                            <option value="ends_with">结尾是</option>
                        </select>

                        <label class="config-label">值</label>
                        <input
                            v-model="selectedNode.config.value"
                            type="text"
                            class="config-input"
                            placeholder="输入匹配值"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        />
                    </div>

                    <!-- 动作配置 -->
                    <div
                        v-else-if="selectedNode.type === 'action'"
                        class="config-section"
                    >
                        <label class="config-label">执行动作</label>
                        <select
                            v-model="selectedNode.config.action"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="mark_read">标记为已读</option>
                            <option value="mark_unread">标记为未读</option>
                            <option value="star">添加星标</option>
                            <option value="unstar">移除星标</option>
                            <option value="archive">归档</option>
                            <option value="delete">删除</option>
                            <option value="move">移动到文件夹</option>
                        </select>

                        <div v-if="selectedNode.config.action === 'move'">
                            <label class="config-label">目标文件夹</label>
                            <select
                                v-model="selectedNode.config.folder"
                                class="config-input"
                                @change="
                                    workflowStore.updateNode(selectedNode.id, {
                                        config: selectedNode.config,
                                    })
                                "
                            >
                                <option value="inbox">收件箱</option>
                                <option value="spam">垃圾邮件</option>
                                <option value="trash">已删除</option>
                            </select>
                        </div>
                    </div>

                    <!-- 邮件配置 -->
                    <div
                        v-else-if="selectedNode.type === 'email'"
                        class="config-section"
                    >
                        <label class="config-label">邮件模板</label>
                        <select
                            v-model="selectedNode.config.template"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="">选择模板</option>
                            <option value="welcome">欢迎邮件</option>
                            <option value="confirmation">确认邮件</option>
                            <option value="notification">通知邮件</option>
                            <option value="custom">自定义</option>
                        </select>

                        <label class="config-label">收件人</label>
                        <input
                            v-model="selectedNode.config.recipients"
                            type="text"
                            class="config-input"
                            placeholder="输入收件人邮箱"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        />
                    </div>

                    <!-- 延迟配置 -->
                    <div
                        v-else-if="selectedNode.type === 'delay'"
                        class="config-section"
                    >
                        <label class="config-label">延迟时长</label>
                        <div class="delay-inputs">
                            <input
                                v-model.number="selectedNode.config.duration"
                                type="number"
                                class="config-input"
                                min="1"
                                @change="
                                    workflowStore.updateNode(selectedNode.id, {
                                        config: selectedNode.config,
                                    })
                                "
                            />
                            <select
                                v-model="selectedNode.config.unit"
                                class="config-input"
                                @change="
                                    workflowStore.updateNode(selectedNode.id, {
                                        config: selectedNode.config,
                                    })
                                "
                            >
                                <option value="minutes">分钟</option>
                                <option value="hours">小时</option>
                                <option value="days">天</option>
                            </select>
                        </div>
                    </div>

                    <!-- AI 配置 -->
                    <div
                        v-else-if="selectedNode.type === 'ai'"
                        class="config-section"
                    >
                        <label class="config-label">AI 模型</label>
                        <select
                            v-model="selectedNode.config.model"
                            class="config-input"
                            @change="
                                workflowStore.updateNode(selectedNode.id, {
                                    config: selectedNode.config,
                                })
                            "
                        >
                            <option value="gpt-4">GPT-4</option>
                            <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
                            <option value="claude-3">Claude 3</option>
                        </select>

                        <label class="config-label">提示词</label>
                        <textarea
                            :value="
                                (selectedNode.config.prompt as string) || ''
                            "
                            @input="
                                (e) => {
                                    if (selectedNode && e.target) {
                                        workflowStore.updateNode(
                                            selectedNode.id,
                                            {
                                                config: {
                                                    ...selectedNode.config,
                                                    prompt: (
                                                        e.target as HTMLTextAreaElement
                                                    ).value,
                                                },
                                            },
                                        );
                                    }
                                }
                            "
                            class="config-textarea"
                            placeholder="输入 AI 提示词"
                            rows="4"
                        ></textarea>
                    </div>

                    <!-- 节点信息 -->
                    <div class="node-info-section">
                        <div class="info-item">
                            <span class="info-label">节点 ID:</span>
                            <span class="info-value">{{
                                selectedNode.id
                            }}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">类型:</span>
                            <span class="info-value">{{
                                selectedNode.type
                            }}</span>
                        </div>
                        <div class="info-item">
                            <span class="info-label">位置:</span>
                            <span class="info-value">
                                ({{ Math.round(selectedNode.position.x) }},
                                {{ Math.round(selectedNode.position.y) }})
                            </span>
                        </div>
                    </div>
                </div>

                <div class="panel-footer">
                    <button
                        class="btn btn-danger btn-sm"
                        @click="deleteNode(selectedNode.id)"
                    >
                        删除节点
                    </button>
                </div>
            </div>
        </div>

        <!-- 切换节点面板按钮 -->
        <button
            v-if="!showNodePanel"
            class="toggle-panel-btn"
            @click="showNodePanel = true"
            title="显示节点面板"
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 24 24"
                fill="currentColor"
            >
                <path d="M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z" />
            </svg>
        </button>
    </div>
</template>

<style scoped>
.workflow-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-base);
    overflow: hidden;
}

/* 工具栏 */
.workflow-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    background: var(--bg-elevated);
    border-bottom: 1px solid var(--border-subtle);
    backdrop-filter: blur(var(--blur-md));
}

.toolbar-left {
    display: flex;
    align-items: center;
    gap: 24px;
}

.toolbar-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
}

.toolbar-title svg {
    width: 24px;
    height: 24px;
    color: var(--primary);
}

.toolbar-actions {
    display: flex;
    align-items: center;
    gap: 8px;
}

.toolbar-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    background: var(--bg-glass);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
}

.toolbar-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
    border-color: var(--border-default);
}

.toolbar-btn.danger:hover {
    background: var(--accent-light);
    color: var(--accent);
    border-color: var(--accent);
}

.toolbar-btn svg {
    width: 16px;
    height: 16px;
}

.toolbar-divider {
    width: 1px;
    height: 24px;
    background: var(--border-subtle);
    margin: 0 4px;
}

.toolbar-right {
    display: flex;
    align-items: center;
    gap: 16px;
}

.zoom-controls {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    background: var(--bg-glass);
    border-radius: 6px;
    border: 1px solid var(--border-subtle);
}

.zoom-level {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    min-width: 45px;
    text-align: center;
}

.node-count {
    padding: 4px 12px;
    background: var(--primary-light);
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    color: var(--primary);
}

/* 主内容区域 */
.workflow-content {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
}

/* 节点面板 */
.node-panel {
    width: 280px;
    background: var(--bg-elevated);
    border-right: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(var(--blur-md));
}

.panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-subtle);
}

.panel-header h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
}

.node-list {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
}

.node-type-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    background: var(--bg-glass);
    border: 1px solid var(--border-subtle);
    border-left-width: 3px;
    border-radius: 8px;
    margin-bottom: 8px;
    cursor: grab;
    transition: all 0.15s ease;
}

.node-type-item:hover {
    background: var(--bg-glass-hover);
    border-color: var(--border-default);
    transform: translateX(2px);
}

.node-type-item:active {
    cursor: grabbing;
}

.node-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    flex-shrink: 0;
}

.node-icon svg {
    width: 20px;
    height: 20px;
}

.node-info {
    flex: 1;
}

.node-label {
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
    margin-bottom: 2px;
}

.node-desc {
    font-size: 12px;
    color: var(--text-muted);
}

.panel-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border-subtle);
}

/* 画布区域 */
.workflow-canvas {
    flex: 1;
    position: relative;
    overflow: hidden;
    cursor: grab;
    background: var(--bg-base);
}

.workflow-canvas:active {
    cursor: grabbing;
}

.canvas-grid {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-image: radial-gradient(
        circle,
        var(--border-subtle) 1px,
        transparent 1px
    );
    background-size: 20px 20px;
    pointer-events: none;
    opacity: 0.5;
}

.nodes-container {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    transform-origin: 0 0;
    pointer-events: none;
}

/* 节点样式 */
.workflow-node {
    position: absolute;
    width: 240px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-left-width: 3px;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    cursor: move;
    pointer-events: auto;
    transition: all 0.15s ease;
}

.workflow-node:hover {
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    transform: translateY(-2px);
}

.workflow-node.selected {
    border-color: var(--primary);
    box-shadow:
        0 0 0 2px var(--primary-light),
        0 4px 12px rgba(124, 58, 237, 0.2);
}

.node-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--bg-glass);
}

.node-header .node-icon {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
}

.node-header .node-icon svg {
    width: 16px;
    height: 16px;
}

.node-title {
    flex: 1;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
}

.node-actions {
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.15s ease;
}

.workflow-node:hover .node-actions {
    opacity: 1;
}

.icon-btn-xs {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: var(--text-muted);
    cursor: pointer;
    transition: all 0.15s ease;
}

.icon-btn-xs:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.icon-btn-xs.danger:hover {
    background: var(--accent-light);
    color: var(--accent);
}

.icon-btn-xs svg {
    width: 14px;
    height: 14px;
}

.node-body {
    padding: 12px;
}

.node-type {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 8px;
}

.node-config {
    font-size: 13px;
    color: var(--text-secondary);
}

.config-label {
    font-weight: 500;
    color: var(--text-muted);
    margin-right: 4px;
}

.config-value {
    color: var(--text-primary);
}

/* 连接点 */
.node-connector {
    position: absolute;
    width: 12px;
    height: 12px;
    background: var(--bg-elevated);
    border: 2px solid var(--border-default);
    border-radius: 50%;
    cursor: crosshair;
    transition: all 0.15s ease;
}

.node-connector:hover {
    border-color: var(--primary);
    background: var(--primary-light);
    transform: scale(1.2);
}

.node-connector.input {
    left: -6px;
    top: 50%;
    transform: translateY(-50%);
}

.node-connector.output {
    right: -6px;
    top: 50%;
    transform: translateY(-50%);
}

.node-connector.input:hover,
.node-connector.output:hover {
    transform: translateY(-50%) scale(1.2);
}

/* 空状态 */
.empty-state {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    text-align: center;
    pointer-events: none;
}

.empty-state svg {
    width: 64px;
    height: 64px;
    color: var(--text-muted);
    margin-bottom: 16px;
    opacity: 0.5;
}

.empty-state h3 {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 8px;
}

.empty-state p {
    font-size: 14px;
    color: var(--text-muted);
    margin-bottom: 16px;
}

.empty-state .btn {
    pointer-events: auto;
}

/* 配置面板 */
.config-panel {
    width: 320px;
    background: var(--bg-elevated);
    border-left: 1px solid var(--border-subtle);
    display: flex;
    flex-direction: column;
    backdrop-filter: blur(var(--blur-md));
}

.config-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
}

.config-section {
    margin-bottom: 20px;
}

.config-section .config-label {
    display: block;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    margin-bottom: 8px;
}

.config-input,
.config-textarea {
    width: 100%;
    padding: 8px 12px;
    background: var(--bg-glass);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    color: var(--text-primary);
    font-size: 14px;
    transition: all 0.15s ease;
}

.config-input:focus,
.config-textarea:focus {
    outline: none;
    border-color: var(--primary);
    box-shadow: 0 0 0 3px var(--primary-light);
}

.config-textarea {
    resize: vertical;
    font-family: inherit;
}

.delay-inputs {
    display: flex;
    gap: 8px;
}

.delay-inputs .config-input:first-child {
    flex: 1;
}

.delay-inputs .config-input:last-child {
    width: 100px;
}

.node-info-section {
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid var(--border-subtle);
}

.info-item {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: 13px;
}

.info-label {
    color: var(--text-muted);
}

.info-value {
    color: var(--text-secondary);
    font-family: monospace;
}

/* 切换面板按钮 */
.toggle-panel-btn {
    position: absolute;
    left: 16px;
    top: 50%;
    transform: translateY(-50%);
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    z-index: 10;
}

.toggle-panel-btn:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

.toggle-panel-btn svg {
    width: 20px;
    height: 20px;
}

/* 按钮样式 */
.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 14px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
}

.btn-sm {
    padding: 6px 12px;
    font-size: 13px;
}

.btn-primary {
    background: linear-gradient(
        135deg,
        var(--primary) 0%,
        var(--secondary) 100%
    );
    color: white;
    box-shadow: 0 2px 8px rgba(124, 58, 237, 0.3);
}

.btn-primary:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(124, 58, 237, 0.4);
}

.btn-ghost {
    background: transparent;
    color: var(--text-secondary);
}

.btn-ghost:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.btn-danger {
    background: var(--accent-light);
    color: var(--accent);
}

.btn-danger:hover {
    background: var(--accent);
    color: white;
}

.icon-btn-sm {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.15s ease;
}

.icon-btn-sm:hover {
    background: var(--bg-glass-hover);
    color: var(--text-primary);
}

.icon-btn-sm svg {
    width: 16px;
    height: 16px;
}

/* 响应式 */
@media (max-width: 1024px) {
    .workflow-toolbar {
        padding: 12px 16px;
    }

    .toolbar-left {
        gap: 16px;
    }

    .btn-label {
        display: none;
    }

    .node-panel {
        width: 240px;
    }

    .config-panel {
        width: 280px;
    }
}

@media (max-width: 768px) {
    .workflow-toolbar {
        flex-direction: column;
        gap: 12px;
        align-items: flex-start;
    }

    .toolbar-right {
        width: 100%;
        justify-content: space-between;
    }

    .node-panel,
    .config-panel {
        position: absolute;
        top: 0;
        bottom: 0;
        z-index: 20;
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.2);
    }

    .node-panel {
        left: 0;
    }

    .config-panel {
        right: 0;
    }
}
</style>
