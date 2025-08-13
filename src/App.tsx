import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ThreeColumnLayout } from "./components/ThreeColumnLayout";
import { useEmailStore } from "./stores/useEmailStore";
import { setupGlobalErrorHandler, appLogger } from "./utils/logger";
import "./App.css";

function App() {
  const { initialize } = useEmailStore();
  const { i18n } = useTranslation();

  useEffect(() => {
    // 设置全局错误处理
    setupGlobalErrorHandler();

    // 禁用默认右键菜单
    const handleContextMenu = (e: MouseEvent) => {
      // 检查是否是可选择文本的元素
      const target = e.target as HTMLElement;
      const isSelectable =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.contentEditable === "true" ||
        target.classList.contains("selectable") ||
        target.classList.contains("email-content") ||
        target.classList.contains("email-body") ||
        target.closest(".email-content") ||
        target.closest(".email-body");

      // 如果不是可选择文本的元素，阻止默认右键菜单
      if (!isSelectable) {
        e.preventDefault();

        // 可以在这里添加自定义右键菜单逻辑
        // 例如：showCustomContextMenu(e.clientX, e.clientY);
      }
    };

    // 禁用拖拽选择（除了特定元素）
    const handleSelectStart = (e: Event) => {
      const target = e.target as HTMLElement;
      const isSelectable =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.contentEditable === "true" ||
        target.classList.contains("selectable") ||
        target.classList.contains("email-content") ||
        target.classList.contains("email-body");

      if (!isSelectable) {
        e.preventDefault();
      }
    };

    // 禁用拖拽
    const handleDragStart = (e: DragEvent) => {
      const target = e.target as HTMLElement;
      // 只允许特定元素被拖拽（如果需要的话）
      if (!target.classList.contains("draggable")) {
        e.preventDefault();
      }
    };

    // 添加事件监听器
    document.addEventListener("contextmenu", handleContextMenu);
    document.addEventListener("selectstart", handleSelectStart);
    document.addEventListener("dragstart", handleDragStart);

    // 初始化应用
    const initApp = async () => {
      await appLogger.info("Starting Postium Mail application");

      // 检测并设置用户语言偏好
      const savedLanguage = localStorage.getItem("i18nextLng");
      if (!savedLanguage) {
        // 如果没有保存的语言，检测系统语言
        const systemLanguage = navigator.language.toLowerCase();
        if (systemLanguage.startsWith("zh")) {
          await i18n.changeLanguage("zh");
        } else {
          await i18n.changeLanguage("en");
        }
      }

      await initialize();
      await appLogger.info("Application initialized successfully");
    };

    initApp().catch(async (error) => {
      await appLogger.error("Failed to initialize application", error);
    });

    // 清理事件监听器
    return () => {
      document.removeEventListener("contextmenu", handleContextMenu);
      document.removeEventListener("selectstart", handleSelectStart);
      document.removeEventListener("dragstart", handleDragStart);
    };
  }, [initialize, i18n]);

  return <ThreeColumnLayout />;
}

export default App;
