import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { WorkflowNode, NodeType } from '@/types'
import { delay } from '@/mocks'

export const useWorkflowStore = defineStore('workflow', () => {
  // ========================================
  // State
  // ========================================

  // 工作流节点列表
  const nodes = ref<WorkflowNode[]>([])

  // 当前选中的节点 ID
  const selectedNodeId = ref<string | null>(null)

  // 加载状态
  const isLoading = ref(false)

  // 节点 ID 计数器
  let nodeIdCounter = 0

  // 拖拽状态
  const isDragging = ref(false)
  const draggedNodeId = ref<string | null>(null)

  // ========================================
  // Node Type Definitions
  // ========================================

  const nodeTypes: Array<{
    type: NodeType
    label: string
    icon: string
    color: string
  }> = [
    { type: 'trigger', label: '触发器', icon: 'bolt', color: '#F59E0B' },
    { type: 'condition', label: '条件', icon: 'git-branch', color: '#3B82F6' },
    { type: 'action', label: '操作', icon: 'play', color: '#10B981' },
    { type: 'email', label: '邮件', icon: 'mail', color: '#7C3AED' },
    { type: 'delay', label: '延迟', icon: 'clock', color: '#6B7280' },
    { type: 'ai', label: 'AI', icon: 'sparkles', color: '#EC4899' },
  ]

  // ========================================
  // Getters
  // ========================================

  // 当前选中的节点
  const selectedNode = computed(() => {
    if (!selectedNodeId.value) return null
    return nodes.value.find(n => n.id === selectedNodeId.value) || null
  })

  // 节点数量
  const nodeCount = computed(() => nodes.value.length)

  // 是否有节点
  const hasNodes = computed(() => nodes.value.length > 0)

  // 按类型分组的节点
  const nodesByType = computed(() => {
    const grouped: Record<NodeType, WorkflowNode[]> = {
      trigger: [],
      condition: [],
      action: [],
      email: [],
      delay: [],
      ai: [],
    }

    nodes.value.forEach(node => {
      grouped[node.type].push(node)
    })

    return grouped
  })

  // 节点类型统计
  const nodeTypeStats = computed(() => {
    const stats: Record<NodeType, number> = {
      trigger: 0,
      condition: 0,
      action: 0,
      email: 0,
      delay: 0,
      ai: 0,
    }

    nodes.value.forEach(node => {
      stats[node.type]++
    })

    return stats
  })

  // ========================================
  // Actions
  // ========================================

  // 添加节点
  function addNode(type: NodeType, position?: { x: number; y: number }): WorkflowNode {
    const nodeType = nodeTypes.find(n => n.type === type)
    const id = `node-${++nodeIdCounter}`

    const newNode: WorkflowNode = {
      id,
      type,
      label: nodeType?.label || '节点',
      position: position || { x: 100, y: 100 + nodes.value.length * 80 },
      config: getDefaultConfig(type),
    }

    nodes.value.push(newNode)
    return newNode
  }

  // 获取默认配置
  function getDefaultConfig(type: NodeType): Record<string, unknown> {
    switch (type) {
      case 'trigger':
        return { event: 'email_received' }
      case 'condition':
        return { field: 'subject', operator: 'contains', value: '' }
      case 'action':
        return { action: 'mark_read' }
      case 'email':
        return { template: '', recipients: [] }
      case 'delay':
        return { duration: 1, unit: 'hours' }
      case 'ai':
        return { prompt: '', model: 'gpt-4' }
      default:
        return {}
    }
  }

  // 删除节点
  function removeNode(nodeId: string) {
    const index = nodes.value.findIndex(n => n.id === nodeId)
    if (index !== -1) {
      nodes.value.splice(index, 1)
      if (selectedNodeId.value === nodeId) {
        selectedNodeId.value = null
      }
    }
  }

  // 更新节点
  function updateNode(nodeId: string, updates: Partial<WorkflowNode>) {
    const node = nodes.value.find(n => n.id === nodeId)
    if (node) {
      Object.assign(node, updates)
    }
  }

  // 更新节点位置
  function updateNodePosition(nodeId: string, position: { x: number; y: number }) {
    const node = nodes.value.find(n => n.id === nodeId)
    if (node) {
      node.position = position
    }
  }

  // 选择节点
  function selectNode(nodeId: string | null) {
    selectedNodeId.value = nodeId
  }

  // 清除选择
  function clearSelection() {
    selectedNodeId.value = null
  }

  // 复制节点
  function duplicateNode(nodeId: string): WorkflowNode | null {
    const node = nodes.value.find(n => n.id === nodeId)
    if (!node) return null

    const newNode: WorkflowNode = {
      ...node,
      id: `node-${++nodeIdCounter}`,
      position: {
        x: node.position.x + 20,
        y: node.position.y + 20,
      },
    }

    nodes.value.push(newNode)
    return newNode
  }

  // 移动节点顺序
  function moveNode(fromIndex: number, toIndex: number) {
    if (fromIndex < 0 || toIndex < 0) return
    if (fromIndex >= nodes.value.length || toIndex >= nodes.value.length) return

    const [movedNode] = nodes.value.splice(fromIndex, 1)
    nodes.value.splice(toIndex, 0, movedNode)
  }

  // 开始拖拽
  function startDrag(nodeId: string) {
    isDragging.value = true
    draggedNodeId.value = nodeId
  }

  // 结束拖拽
  function endDrag() {
    isDragging.value = false
    draggedNodeId.value = null
  }

  // 清空工作流
  function clearWorkflow() {
    nodes.value = []
    selectedNodeId.value = null
    nodeIdCounter = 0
  }

  // 保存工作流
  async function saveWorkflow(): Promise<void> {
    isLoading.value = true
    try {
      await delay(500)
      const data = {
        nodes: nodes.value,
        nodeIdCounter,
      }
      localStorage.setItem('postium-workflow', JSON.stringify(data))
    } finally {
      isLoading.value = false
    }
  }

  // 加载工作流
  async function loadWorkflow(): Promise<void> {
    isLoading.value = true
    try {
      await delay(300)
      const saved = localStorage.getItem('postium-workflow')
      if (saved) {
        const data = JSON.parse(saved)
        nodes.value = data.nodes || []
        nodeIdCounter = data.nodeIdCounter || 0
      }
    } finally {
      isLoading.value = false
    }
  }

  // 导出工作流
  function exportWorkflow(): string {
    return JSON.stringify({
      nodes: nodes.value,
      version: '1.0',
    }, null, 2)
  }

  // 导入工作流
  function importWorkflow(json: string): boolean {
    try {
      const data = JSON.parse(json)
      if (data.nodes && Array.isArray(data.nodes)) {
        nodes.value = data.nodes
        nodeIdCounter = nodes.value.length
        return true
      }
      return false
    } catch {
      return false
    }
  }

  // 验证工作流
  function validateWorkflow(): { valid: boolean; errors: string[] } {
    const errors: string[] = []

    // 检查是否有触发器
    const triggers = nodes.value.filter(n => n.type === 'trigger')
    if (triggers.length === 0) {
      errors.push('工作流必须包含至少一个触发器')
    }
    if (triggers.length > 1) {
      errors.push('工作流只能有一个触发器')
    }

    // 检查节点配置
    nodes.value.forEach(node => {
      if (!node.config) {
        errors.push(`节点 "${node.label}" 缺少配置`)
      }
    })

    return {
      valid: errors.length === 0,
      errors,
    }
  }

  return {
    // State
    nodes,
    selectedNodeId,
    isLoading,
    isDragging,
    draggedNodeId,

    // Constants
    nodeTypes,

    // Getters
    selectedNode,
    nodeCount,
    hasNodes,
    nodesByType,
    nodeTypeStats,

    // Actions
    addNode,
    removeNode,
    updateNode,
    updateNodePosition,
    selectNode,
    clearSelection,
    duplicateNode,
    moveNode,
    startDrag,
    endDrag,
    clearWorkflow,
    saveWorkflow,
    loadWorkflow,
    exportWorkflow,
    importWorkflow,
    validateWorkflow,
  }
})
