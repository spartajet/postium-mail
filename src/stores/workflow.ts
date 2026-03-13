import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { WorkflowNode, NodeType } from "@/types";

export const useWorkflowStore = defineStore("workflow", () => {
  // ========================================
  // State
  // ========================================

  // 节点列表
  const nodes = ref<WorkflowNode[]>([]);

  // 选中的节点 ID
  const selectedNodeId = ref<string | null>(null);

  // 拖拽状态
  const isDragging = ref(false);
  const draggedNodeId = ref<string | null>(null);

  // 画布偏移
  const canvasOffset = ref({ x: 0, y: 0 });

  // 缩放级别
  const zoom = ref(1);

  // ========================================
  // Getters
  // ========================================

  // 选中的节点
  const selectedNode = computed(() =>
    nodes.value.find((n) => n.id === selectedNodeId.value)
  );

  // 节点数量
  const nodeCount = computed(() => nodes.value.length);

  // 节点类型配置
  const nodeTypes = computed(() => [
    {
      type: "trigger" as NodeType,
      label: "触发器",
      icon: "bolt",
      color: "#F59E0B",
      description: "工作流开始触发条件",
    },
    {
      type: "condition" as NodeType,
      label: "条件",
      icon: "git-branch",
      color: "#3B82F6",
      description: "判断分支条件",
    },
    {
      type: "action" as NodeType,
      label: "操作",
      icon: "play",
      color: "#10B981",
      description: "执行具体操作",
    },
    {
      type: "email" as NodeType,
      label: "邮件",
      icon: "email",
      color: "#8B5CF6",
      description: "邮件相关操作",
    },
    {
      type: "delay" as NodeType,
      label: "延迟",
      icon: "clock",
      color: "#6B7280",
      description: "延迟执行",
    },
    {
      type: "ai" as NodeType,
      label: "AI",
      icon: "sparkles",
      color: "#EC4899",
      description: "AI 智能处理",
    },
  ]);

  // ========================================
  // Actions
  // ========================================

  // 添加节点
  function addNode(type: NodeType, position: { x: number; y: number }) {
    const nodeType = nodeTypes.value.find((t) => t.type === type);
    const newNode: WorkflowNode = {
      id: `node-${Date.now()}`,
      type,
      label: nodeType?.label || "新节点",
      position,
      config: {},
    };
    nodes.value.push(newNode);
    selectNode(newNode.id);
    return newNode;
  }

  // 删除节点
  function removeNode(nodeId: string) {
    const index = nodes.value.findIndex((n) => n.id === nodeId);
    if (index !== -1) {
      nodes.value.splice(index, 1);
      if (selectedNodeId.value === nodeId) {
        selectedNodeId.value = null;
      }
    }
  }

  // 复制节点
  function duplicateNode(nodeId: string): WorkflowNode | null {
    const node = nodes.value.find((n) => n.id === nodeId);
    if (node) {
      const newNode: WorkflowNode = {
        id: `node-${Date.now()}`,
        type: node.type,
        label: `${node.label} 副本`,
        position: {
          x: node.position.x + 50,
          y: node.position.y + 50,
        },
        config: { ...node.config },
      };
      nodes.value.push(newNode);
      return newNode;
    }
    return null;
  }

  // 更新节点（包含位置和配置）
  function updateNode(
    nodeId: string,
    updates: Partial<WorkflowNode>
  ) {
    const node = nodes.value.find((n) => n.id === nodeId);
    if (node) {
      if (updates.position) {
        node.position = updates.position;
      }
      if (updates.label) {
        node.label = updates.label;
      }
      if (updates.config) {
        node.config = { ...node.config, ...updates.config };
      }
    }
  }

  // 保存工作流
  function saveWorkflow() {
    saveToLocalStorage();
  }

  // 更新节点位置
  function updateNodePosition(
    nodeId: string,
    position: { x: number; y: number }
  ) {
    const node = nodes.value.find((n) => n.id === nodeId);
    if (node) {
      node.position = position;
    }
  }

  // 更新节点配置
  function updateNodeConfig(nodeId: string, config: Record<string, unknown>) {
    const node = nodes.value.find((n) => n.id === nodeId);
    if (node) {
      node.config = { ...node.config, ...config };
    }
  }

  // 选择节点
  function selectNode(nodeId: string | null) {
    selectedNodeId.value = nodeId;
  }

  // 清除选择
  function clearSelection() {
    selectedNodeId.value = null;
  }

  // 开始拖拽
  function startDrag(nodeId: string) {
    isDragging.value = true;
    draggedNodeId.value = nodeId;
  }

  // 结束拖拽
  function endDrag() {
    isDragging.value = false;
    draggedNodeId.value = null;
  }

  // 设置画布偏移
  function setCanvasOffset(offset: { x: number; y: number }) {
    canvasOffset.value = offset;
  }

  // 设置缩放
  function setZoom(level: number) {
    zoom.value = Math.max(0.5, Math.min(2, level));
  }

  // 清空工作流
  function clearWorkflow() {
    nodes.value = [];
    selectedNodeId.value = null;
    canvasOffset.value = { x: 0, y: 0 };
    zoom.value = 1;
  }

  // 加载工作流
  function loadWorkflow(workflowNodes?: WorkflowNode[]) {
    if (workflowNodes) {
      nodes.value = workflowNodes;
      selectedNodeId.value = null;
    } else {
      loadFromLocalStorage();
    }
  }

  // 保存工作流到本地存储
  function saveToLocalStorage() {
    localStorage.setItem("postium-workflow", JSON.stringify(nodes.value));
  }

  // 从本地存储加载工作流
  function loadFromLocalStorage() {
    const saved = localStorage.getItem("postium-workflow");
    if (saved) {
      try {
        nodes.value = JSON.parse(saved);
      } catch (e) {
        console.error("Failed to load workflow:", e);
      }
    }
  }

  // 验证工作流
  function validateWorkflow(): {
    valid: boolean;
    errors: string[];
  } {
    const errors: string[] = [];

    // 检查是否有节点
    if (nodes.value.length === 0) {
      errors.push("工作流至少需要一个节点");
    }

    // 检查是否有触发器节点
    const hasTrigger = nodes.value.some((n) => n.type === "trigger");
    if (!hasTrigger) {
      errors.push("工作流需要一个触发器节点");
    }

    // 检查每个节点的配置
    nodes.value.forEach((node) => {
      if (!node.label || node.label.trim() === "") {
        errors.push(`节点 ${node.id} 缺少标签`);
      }
    });

    return {
      valid: errors.length === 0,
      errors,
    };
  }

  // 导出工作流
  function exportWorkflow(): string {
    return JSON.stringify(nodes.value, null, 2);
  }

  // 导入工作流
  function importWorkflow(jsonString: string): boolean {
    try {
      const imported = JSON.parse(jsonString);
      if (Array.isArray(imported)) {
        nodes.value = imported;
        saveToLocalStorage();
        return true;
      }
      return false;
    } catch (e) {
      console.error("Failed to import workflow:", e);
      return false;
    }
  }

  // ========================================
  // Return
  // ========================================

  return {
    // State
    nodes,
    selectedNodeId,
    isDragging,
    draggedNodeId,
    canvasOffset,
    zoom,

    // Getters
    selectedNode,
    nodeTypes,
    nodeCount,

    // Actions
    addNode,
    removeNode,
    duplicateNode,
    updateNodePosition,
    updateNodeConfig,
    updateNode,
    selectNode,
    clearSelection,
    startDrag,
    endDrag,
    setCanvasOffset,
    setZoom,
    clearWorkflow,
    loadWorkflow,
    saveWorkflow,
    saveToLocalStorage,
    loadFromLocalStorage,
    validateWorkflow,
    exportWorkflow,
    importWorkflow,
  };
});
