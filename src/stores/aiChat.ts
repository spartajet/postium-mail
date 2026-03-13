import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ChatMessage, PendingAction, Email } from "@/types";
import { mockAISummary, mockSmartReply, delay } from "@/mocks";

export type ChatMode = "query" | "action" | "compose";
export type MessageStatus = "pending" | "streaming" | "complete" | "error";

export const useAIChatStore = defineStore("aiChat", () => {
  // ========================================
  // State
  // ========================================

  // 对话消息列表
  const messages = ref<ChatMessage[]>([]);

  // 当前输入的消息
  const inputMessage = ref("");

  // 加载状态
  const isLoading = ref(false);

  // 流式输出状态
  const isStreaming = ref(false);

  // 当前流式输出的内容
  const streamingContent = ref("");

  // 当前对话模式
  const chatMode = ref<ChatMode>("query");

  // 待确认的操作
  const pendingAction = ref<PendingAction | null>(null);

  // 查询结果邮件列表
  const currentResults = ref<Email[]>([]);

  // 上下文邮件（当前正在查看的邮件）
  const contextEmail = ref<Email | null>(null);

  // 对话 ID（用于会话管理）
  const conversationId = ref(`conv-${Date.now()}`);

  // 错误信息
  const error = ref<string | null>(null);

  // ========================================
  // Getters
  // ========================================

  // 消息数量
  const messageCount = computed(() => messages.value.length);

  // 是否有消息
  const hasMessages = computed(() => messages.value.length > 0);

  // 最后一条消息
  const lastMessage = computed(() => {
    return messages.value.length > 0
      ? messages.value[messages.value.length - 1]
      : null;
  });

  // 用户消息列表
  const userMessages = computed(() => {
    return messages.value.filter((m) => m.role === "user");
  });

  // 助手消息列表
  const assistantMessages = computed(() => {
    return messages.value.filter((m) => m.role === "assistant");
  });

  // 是否有待确认的操作
  const hasPendingAction = computed(() => pendingAction.value !== null);

  // 是否有查询结果
  const hasResults = computed(() => currentResults.value.length > 0);

  // ========================================
  // Quick Prompts
  // ========================================

  const quickPrompts = [
    { id: "unread", label: "显示未读邮件", prompt: "显示所有未读邮件" },
    { id: "today", label: "今天的邮件", prompt: "显示今天的邮件" },
    { id: "important", label: "重要邮件", prompt: "显示标记为重要的邮件" },
    { id: "summary", label: "总结收件箱", prompt: "帮我总结一下收件箱的邮件" },
    { id: "clean", label: "清理垃圾邮件", prompt: "帮我清理垃圾邮件" },
    { id: "unsubscribe", label: "查找订阅邮件", prompt: "找出所有订阅邮件" },
  ];

  // ========================================
  // Actions
  // ========================================

  // 发送消息
  async function sendMessage(content?: string) {
    const messageContent = content || inputMessage.value.trim();
    if (!messageContent || isLoading.value) return;

    // 清空输入
    inputMessage.value = "";
    error.value = null;

    // 添加用户消息
    const userMessage: ChatMessage = {
      id: `msg-${Date.now()}`,
      role: "user",
      content: messageContent,
      timestamp: new Date(),
    };
    messages.value.push(userMessage);

    // 处理消息
    await processMessage(messageContent);
  }

  // 处理消息（解析意图并生成响应）
  async function processMessage(content: string) {
    isLoading.value = true;
    isStreaming.value = true;
    streamingContent.value = "";

    try {
      // 解析意图
      const intent = parseIntent(content);

      // 根据意图处理
      let response = "";

      switch (intent.type) {
        case "query":
          response = await handleQueryIntent(intent, content);
          break;
        case "action":
          response = await handleActionIntent(intent, content);
          break;
        case "summary":
          response = await handleSummaryIntent(intent, content);
          break;
        case "compose":
          response = await handleComposeIntent(intent, content);
          break;
        default:
          response = await handleGeneralQuery(content);
      }

      // 模拟流式输出
      await streamResponse(response);
    } catch (err) {
      error.value = err instanceof Error ? err.message : "处理消息时出错";
      addAssistantMessage("抱歉，处理您的请求时出现了错误。请稍后再试。");
    } finally {
      isLoading.value = false;
      isStreaming.value = false;
      streamingContent.value = "";
    }
  }

  // 解析意图
  function parseIntent(content: string): {
    type: string;
    params: Record<string, unknown>;
  } {
    const lowerContent = content.toLowerCase();

    // 查询类意图
    if (
      lowerContent.includes("显示") ||
      lowerContent.includes("查找") ||
      lowerContent.includes("搜索") ||
      lowerContent.includes("找出")
    ) {
      return {
        type: "query",
        params: {
          queryType: extractQueryType(lowerContent),
          filters: extractFilters(lowerContent),
        },
      };
    }

    // 操作类意图
    if (
      lowerContent.includes("删除") ||
      lowerContent.includes("标记") ||
      lowerContent.includes("移动") ||
      lowerContent.includes("归档")
    ) {
      return {
        type: "action",
        params: {
          actionType: extractActionType(lowerContent),
          target: extractTarget(lowerContent),
        },
      };
    }

    // 摘要类意图
    if (
      lowerContent.includes("总结") ||
      lowerContent.includes("摘要") ||
      lowerContent.includes("概要")
    ) {
      return {
        type: "summary",
        params: {
          target: extractSummaryTarget(lowerContent),
        },
      };
    }

    // 写作类意图
    if (
      lowerContent.includes("写") ||
      lowerContent.includes("回复") ||
      lowerContent.includes("起草")
    ) {
      return {
        type: "compose",
        params: {
          composeType: extractComposeType(lowerContent),
        },
      };
    }

    return { type: "general", params: {} };
  }

  // 处理查询意图
  async function handleQueryIntent(
    _intent: { params: Record<string, unknown> },
    _content: string,
  ): Promise<string> {
    await delay(500);

    // 模拟查询结果
    const resultCount = Math.floor(Math.random() * 10) + 1;

    return `我找到了 ${resultCount} 封符合条件的邮件。您可以在下方查看详细列表。\n\n需要我对这些邮件进行任何操作吗？`;
  }

  // 处理操作意图
  async function handleActionIntent(
    intent: { params: Record<string, unknown> },
    content: string,
  ): Promise<string> {
    await delay(300);

    // 设置待确认操作
    pendingAction.value = {
      type: (intent.params.actionType as string) || "unknown",
      description: `确认要执行此操作吗？`,
      affectedCount: Math.floor(Math.random() * 5) + 1,
      emailIds: [],
    };

    return `我理解您想要${content}。这是一个需要确认的操作。\n\n请确认是否继续执行？`;
  }

  // 处理摘要意图
  async function handleSummaryIntent(
    _intent: { params: Record<string, unknown> },
    _content: string,
  ): Promise<string> {
    await delay(800);

    if (contextEmail.value) {
      return mockAISummary(contextEmail.value);
    }

    return `以下是您收件箱的摘要：\n\n• 您有 5 封未读邮件\n• 2 封邮件标记为重要\n• 3 封邮件需要回复\n\n需要我帮您处理这些邮件吗？`;
  }

  // 处理写作意图
  async function handleComposeIntent(
    _intent: { params: Record<string, unknown> },
    _content: string,
  ): Promise<string> {
    await delay(600);

    const replies = mockSmartReply("formal");
    return `我为您准备了以下回复建议：\n\n${replies[0]}\n\n您可以直接使用或根据需要进行修改。`;
  }

  // 处理一般查询
  async function handleGeneralQuery(_content: string): Promise<string> {
    await delay(500);

    const responses = [
      "我理解您的问题。让我帮您处理。",
      "好的，我来帮您查看相关信息。",
      "收到，我正在处理您的请求。",
      "我明白了，请稍等片刻。",
    ];

    return responses[Math.floor(Math.random() * responses.length)];
  }

  // 流式输出响应
  async function streamResponse(content: string) {
    const chars = content.split("");
    for (const char of chars) {
      streamingContent.value += char;
      await delay(20);
    }

    // 添加助手消息
    addAssistantMessage(content);
  }

  // 添加助手消息
  function addAssistantMessage(
    content: string,
    pendingActionData?: PendingAction,
  ) {
    const message: ChatMessage = {
      id: `msg-${Date.now()}`,
      role: "assistant",
      content,
      timestamp: new Date(),
      pendingAction: pendingActionData,
    };
    messages.value.push(message);
  }

  // 确认执行操作
  async function confirmAction() {
    if (!pendingAction.value) return;

    isLoading.value = true;
    try {
      await delay(500);

      // 模拟执行操作
      const successCount = pendingAction.value.affectedCount;
      const failedCount = 0;

      // 添加操作结果消息
      addAssistantMessage(
        `操作已完成。\n\n成功：${successCount} 封邮件\n失败：${failedCount} 封邮件`,
        undefined,
      );

      // 清除待确认操作
      pendingAction.value = null;
    } finally {
      isLoading.value = false;
    }
  }

  // 取消操作
  function cancelAction() {
    pendingAction.value = null;
    addAssistantMessage("操作已取消。还有其他我可以帮助您的吗？");
  }

  // 清空对话
  function clearMessages() {
    messages.value = [];
    pendingAction.value = null;
    currentResults.value = [];
    streamingContent.value = "";
    error.value = null;
  }

  // 开始新对话
  function startNewConversation() {
    clearMessages();
    conversationId.value = `conv-${Date.now()}`;
  }

  // 设置上下文邮件
  function setContextEmail(email: Email | null) {
    contextEmail.value = email;
  }

  // 设置查询结果
  function setResults(emails: Email[]) {
    currentResults.value = emails;
  }

  // 使用快捷提示
  function useQuickPrompt(prompt: string) {
    inputMessage.value = prompt;
    sendMessage(prompt);
  }

  // 保存对话历史
  function saveConversation() {
    const data = {
      id: conversationId.value,
      messages: messages.value,
      timestamp: new Date(),
    };
    localStorage.setItem("postium-chat", JSON.stringify(data));
  }

  // 加载对话历史
  function loadConversation() {
    const saved = localStorage.getItem("postium-chat");
    if (saved) {
      try {
        const data = JSON.parse(saved);
        conversationId.value = data.id;
        messages.value = data.messages.map((m: ChatMessage) => ({
          ...m,
          timestamp: new Date(m.timestamp),
        }));
      } catch (e) {
        console.error("Failed to load conversation:", e);
      }
    }
  }

  // ========================================
  // Helper Functions
  // ========================================

  function extractQueryType(content: string): string {
    if (content.includes("未读")) return "unread";
    if (content.includes("重要") || content.includes("星标"))
      return "important";
    if (content.includes("今天")) return "today";
    if (content.includes("本周")) return "week";
    if (content.includes("附件")) return "attachments";
    return "general";
  }

  function extractFilters(content: string): Record<string, unknown> {
    const filters: Record<string, unknown> = {};

    if (content.includes("来自") || content.includes("发件人")) {
      filters.sender = true;
    }
    if (content.includes("包含") || content.includes("含有")) {
      filters.contains = true;
    }
    if (content.includes("今天")) {
      filters.date = "today";
    }

    return filters;
  }

  function extractActionType(content: string): string {
    if (content.includes("删除")) return "delete";
    if (content.includes("标记已读") || content.includes("标为已读"))
      return "mark_read";
    if (content.includes("标记未读") || content.includes("标为未读"))
      return "mark_unread";
    if (content.includes("移动")) return "move";
    if (content.includes("归档")) return "archive";
    if (content.includes("星标") || content.includes("标记重要")) return "star";
    return "unknown";
  }

  function extractTarget(content: string): string {
    if (content.includes("所有")) return "all";
    if (content.includes("未读")) return "unread";
    if (content.includes("已选")) return "selected";
    return "filtered";
  }

  function extractSummaryTarget(content: string): string {
    if (content.includes("收件箱")) return "inbox";
    if (content.includes("今天")) return "today";
    if (content.includes("这封")) return "current";
    return "all";
  }

  function extractComposeType(content: string): string {
    if (content.includes("回复")) return "reply";
    if (content.includes("转发")) return "forward";
    if (content.includes("起草") || content.includes("写")) return "compose";
    return "general";
  }

  return {
    // State
    messages,
    inputMessage,
    isLoading,
    isStreaming,
    streamingContent,
    chatMode,
    pendingAction,
    currentResults,
    contextEmail,
    conversationId,
    error,

    // Getters
    messageCount,
    hasMessages,
    lastMessage,
    userMessages,
    assistantMessages,
    hasPendingAction,
    hasResults,

    // Constants
    quickPrompts,

    // Actions
    sendMessage,
    processMessage,
    confirmAction,
    cancelAction,
    clearMessages,
    startNewConversation,
    setContextEmail,
    setResults,
    useQuickPrompt,
    saveConversation,
    loadConversation,
  };
});
