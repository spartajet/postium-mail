import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Toast, Settings } from "@/types";
import { i18n } from "@/main";
import type { LanguageCode } from "@/locales";

export type ViewType =
  | "email"
  | "calendar"
  | "workflow"
  | "settings"
  | "ai-chat";
export type ThemeType = "light" | "dark" | "system";

export const useUIStore = defineStore("ui", () => {
  // ========================================
  // State
  // ========================================

  // 主题
  const theme = ref<ThemeType>("dark");

  // 当前视图
  const currentView = ref<ViewType>("email");

  // 侧边栏状态
  const isSidebarOpen = ref(true);

  // 模态框状态
  const modals = ref({
    compose: false,
    settings: false,
    addAccount: false,
    event: false,
    aiChat: false,
  });

  // Toast 队列
  const toasts = ref<Toast[]>([]);

  // Toast ID 计数器
  let toastId = 0;

  // 设置面板
  const settingsPanel = ref<
    "general" | "notifications" | "ai" | "appearance" | "shortcuts"
  >("general");

  // 是否全屏模式
  const isFullscreen = ref(false);

  // 是否正在加载
  const isGlobalLoading = ref(false);

  // 当前拖拽的元素
  const draggingElement = ref<string | null>(null);

  // 响应式断点
  const breakpoints = ref({
    isMobile: false,
    isTablet: false,
    isDesktop: true,
  });

  // 设置
  const settings = ref<Settings>({
    theme: "dark",
    language: "zh-CN",
    notifications: {
      enabled: true,
      sound: true,
      desktop: true,
    },
    ai: {
      provider: "openai",
      model: "gpt-4",
    },
  });

  // ========================================
  // Getters
  // ========================================

  // 实际应用的主题
  const appliedTheme = computed(() => {
    if (theme.value === "system") {
      return window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }
    return theme.value;
  });

  // 是否是暗色主题
  const isDarkTheme = computed(() => appliedTheme.value === "dark");

  // 是否有打开的模态框
  const hasOpenModal = computed(() =>
    Object.values(modals.value).some((isOpen) => isOpen),
  );

  // ========================================
  // Theme Actions
  // ========================================

  // 设置主题
  function setTheme(newTheme: ThemeType) {
    theme.value = newTheme;
    settings.value.theme = newTheme;
    applyTheme();
    saveThemePreference();
  }

  // 切换主题
  function toggleTheme() {
    const nextTheme = theme.value === "dark" ? "light" : "dark";
    setTheme(nextTheme);
  }

  // 应用主题到 DOM
  function applyTheme() {
    const themeValue = appliedTheme.value;
    document.documentElement.setAttribute("data-theme", themeValue);
  }

  // 保存主题偏好
  function saveThemePreference() {
    localStorage.setItem("postium-theme", theme.value);
  }

  // 加载主题偏好
  function loadThemePreference() {
    const saved = localStorage.getItem("postium-theme") as ThemeType | null;
    if (saved) {
      theme.value = saved;
    }
    applyTheme();
  }

  // 监听系统主题变化
  function watchSystemTheme() {
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    mediaQuery.addEventListener("change", () => {
      if (theme.value === "system") {
        applyTheme();
      }
    });
  }

  // ========================================
  // Language Actions
  // ========================================

  // 设置语言
  function setLanguage(newLanguage: LanguageCode) {
    settings.value.language = newLanguage;
    applyLanguage();
    saveSettings();
  }

  // 应用语言到 i18n
  function applyLanguage() {
    const currentLanguage = settings.value.language as LanguageCode;
    i18n.global.locale.value = currentLanguage;
  }

  // 获取当前语言
  const currentLocale = computed(() => settings.value.language as LanguageCode);

  // ========================================
  // View Actions
  // ========================================

  // 切换视图
  function setView(view: ViewType) {
    currentView.value = view;
  }

  // 切换到邮件视图
  function showEmailView() {
    setView("email");
  }

  // 切换到日历视图
  function showCalendarView() {
    setView("calendar");
  }

  // 切换到工作流视图
  function showWorkflowView() {
    setView("workflow");
  }

  // 切换到设置视图
  function showSettingsView() {
    setView("settings");
    modals.value.settings = true;
  }

  // ========================================
  // Sidebar Actions
  // ========================================

  // 切换侧边栏
  function toggleSidebar() {
    isSidebarOpen.value = !isSidebarOpen.value;
  }

  // 打开侧边栏
  function openSidebar() {
    isSidebarOpen.value = true;
  }

  // 关闭侧边栏
  function closeSidebar() {
    isSidebarOpen.value = false;
  }

  // ========================================
  // Modal Actions
  // ========================================

  // 打开写信模态框
  function openComposeModal() {
    modals.value.compose = true;
  }

  // 关闭写信模态框
  function closeComposeModal() {
    modals.value.compose = false;
  }

  // 打开设置模态框
  function openSettingsModal() {
    modals.value.settings = true;
  }

  // 关闭设置模态框
  function closeSettingsModal() {
    modals.value.settings = false;
  }

  // 打开添加账号模态框
  function openAddAccountModal() {
    modals.value.addAccount = true;
  }

  // 关闭添加账号模态框
  function closeAddAccountModal() {
    modals.value.addAccount = false;
  }

  // 打开事件模态框
  function openEventModal() {
    modals.value.event = true;
  }

  // 关闭事件模态框
  function closeEventModal() {
    modals.value.event = false;
  }

  // 打开 AI 对话模态框
  function openAIChatModal() {
    modals.value.aiChat = true;
  }

  // 关闭 AI 对话模态框
  function closeAIChatModal() {
    modals.value.aiChat = false;
  }

  // 关闭所有模态框
  function closeAllModals() {
    Object.keys(modals.value).forEach((key) => {
      modals.value[key as keyof typeof modals.value] = false;
    });
  }

  // ========================================
  // Toast Actions
  // ========================================

  // 显示 Toast
  function showToast(
    message: string,
    type: Toast["type"] = "info",
    duration = 3000,
  ) {
    const id = ++toastId;
    const toast: Toast = { id, message, type };
    toasts.value.push(toast);

    if (duration > 0) {
      setTimeout(() => {
        removeToast(id);
      }, duration);
    }

    return id;
  }

  // 显示成功 Toast
  function showSuccess(message: string, duration?: number) {
    return showToast(message, "success", duration);
  }

  // 显示错误 Toast
  function showError(message: string, duration?: number) {
    return showToast(message, "error", duration);
  }

  // 显示信息 Toast
  function showInfo(message: string, duration?: number) {
    return showToast(message, "info", duration);
  }

  // 移除 Toast
  function removeToast(id: number) {
    const index = toasts.value.findIndex((t) => t.id === id);
    if (index !== -1) {
      toasts.value.splice(index, 1);
    }
  }

  // 清空所有 Toast
  function clearToasts() {
    toasts.value = [];
  }

  // ========================================
  // Settings Actions
  // ========================================

  // 设置当前设置面板
  function setSettingsPanel(panel: typeof settingsPanel.value) {
    settingsPanel.value = panel;
  }

  // 更新设置
  function updateSettings(newSettings: Partial<Settings>) {
    Object.assign(settings.value, newSettings);
    saveSettings();
  }

  // 保存设置
  function saveSettings() {
    localStorage.setItem("postium-settings", JSON.stringify(settings.value));
  }

  // 加载设置
  function loadSettings() {
    const saved = localStorage.getItem("postium-settings");
    if (saved) {
      try {
        settings.value = JSON.parse(saved);
        theme.value = settings.value.theme;
      } catch (e) {
        console.error("Failed to load settings:", e);
      }
    }
  }

  // ========================================
  // Responsive Actions
  // ========================================

  // 更新断点
  function updateBreakpoints() {
    const width = window.innerWidth;
    breakpoints.value = {
      isMobile: width < 640,
      isTablet: width >= 640 && width < 1024,
      isDesktop: width >= 1024,
    };

    // 在移动端自动关闭侧边栏
    if (breakpoints.value.isMobile) {
      isSidebarOpen.value = false;
    }
  }

  // 监听窗口大小变化
  function watchBreakpoints() {
    updateBreakpoints();
    window.addEventListener("resize", updateBreakpoints);
  }

  // ========================================
  // Fullscreen Actions
  // ========================================

  // 切换全屏
  async function toggleFullscreen() {
    try {
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen();
        isFullscreen.value = true;
      } else {
        await document.exitFullscreen();
        isFullscreen.value = false;
      }
    } catch (err) {
      console.error("Fullscreen error:", err);
    }
  }

  // ========================================
  // Drag Actions
  // ========================================

  // 开始拖拽
  function startDrag(elementId: string) {
    draggingElement.value = elementId;
  }

  // 结束拖拽
  function endDrag() {
    draggingElement.value = null;
  }

  // ========================================
  // Initialization
  // ========================================

  // 初始化 UI Store
  function init() {
    loadThemePreference();
    loadSettings();
    applyLanguage();
    watchSystemTheme();
    watchBreakpoints();
  }

  // ========================================
  // Return
  // ========================================

  return {
    // State
    theme,
    currentView,
    isSidebarOpen,
    modals,
    toasts,
    settingsPanel,
    isFullscreen,
    isGlobalLoading,
    draggingElement,
    breakpoints,
    settings,

    // Getters
    appliedTheme,
    isDarkTheme,
    hasOpenModal,
    currentLocale,

    // Theme Actions
    setTheme,
    toggleTheme,
    applyTheme,
    loadThemePreference,

    // Language Actions
    setLanguage,
    applyLanguage,

    // View Actions
    setView,
    showEmailView,
    showCalendarView,
    showWorkflowView,
    showSettingsView,

    // Sidebar Actions
    toggleSidebar,
    openSidebar,
    closeSidebar,

    // Modal Actions
    openComposeModal,
    closeComposeModal,
    openSettingsModal,
    closeSettingsModal,
    openAddAccountModal,
    closeAddAccountModal,
    openEventModal,
    closeEventModal,
    openAIChatModal,
    closeAIChatModal,
    closeAllModals,

    // Toast Actions
    showToast,
    showSuccess,
    showError,
    showInfo,
    removeToast,
    clearToasts,

    // Settings Actions
    setSettingsPanel,
    updateSettings,
    loadSettings,
    saveSettings,

    // Responsive Actions
    updateBreakpoints,

    // Fullscreen Actions
    toggleFullscreen,

    // Drag Actions
    startDrag,
    endDrag,

    // Initialization
    init,
  };
});
