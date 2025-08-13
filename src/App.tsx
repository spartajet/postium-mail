import { useEffect } from "react";
import { ThreeColumnLayout } from "./components/ThreeColumnLayout";
import { useEmailStore } from "./stores/useEmailStore";
import { setupGlobalErrorHandler, appLogger } from "./utils/logger";

function App() {
  const { initialize } = useEmailStore();

  useEffect(() => {
    // 设置全局错误处理
    setupGlobalErrorHandler();

    // 初始化应用
    const initApp = async () => {
      await appLogger.info("Starting Postium Mail application");
      await initialize();
      await appLogger.info("Application initialized successfully");
    };

    initApp().catch(async (error) => {
      await appLogger.error("Failed to initialize application", error);
    });
  }, [initialize]);

  return <ThreeColumnLayout />;
}

export default App;
