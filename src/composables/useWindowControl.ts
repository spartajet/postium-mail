import { ref, onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

// 检查是否在 Tauri 环境中运行
function isTauri(): boolean {
    return "__TAURI__" in window;
}

export function useWindowControl() {
    const appWindow = getCurrentWindow();
    const isMaximized = ref(false);
    const isFullscreen = ref(false);
    const isTauriEnv = ref(isTauri());
    let unlistenResized: (() => void) | null = null;

    // 更新窗口状态
    async function updateWindowState() {
        if (!isTauriEnv.value) return;

        try {
            isMaximized.value = await appWindow.isMaximized();
            isFullscreen.value = await appWindow.isFullscreen();
            console.log("Window state updated:", {
                isMaximized: isMaximized.value,
                isFullscreen: isFullscreen.value,
            });
        } catch (error) {
            console.error("Failed to update window state:", error);
        }
    }

    // 最小化窗口
    async function minimizeWindow() {
        if (!isTauriEnv.value) {
            console.log("Minimize called (not in Tauri environment)");
            return;
        }

        try {
            console.log("Minimizing window...");
            await appWindow.minimize();
        } catch (error) {
            console.error("Failed to minimize window:", error);
        }
    }

    // 切换最大化/还原
    async function toggleMaximize() {
        if (!isTauriEnv.value) {
            console.log("Toggle maximize called (not in Tauri environment)");
            // 在浏览器环境中模拟切换
            isMaximized.value = !isMaximized.value;
            return;
        }

        try {
            console.log("Toggling maximize, current state:", isMaximized.value);
            if (isMaximized.value) {
                await appWindow.unmaximize();
            } else {
                await appWindow.maximize();
            }
            // 更新状态
            await updateWindowState();
        } catch (error) {
            console.error("Failed to toggle maximize:", error);
        }
    }

    // 关闭窗口
    async function closeWindow() {
        if (!isTauriEnv.value) {
            console.log("Close called (not in Tauri environment)");
            return;
        }

        try {
            console.log("Closing window...");
            await appWindow.close();
        } catch (error) {
            console.error("Failed to close window:", error);
        }
    }

    // 开始拖动窗口
    async function startDragging() {
        if (!isTauriEnv.value) return;

        try {
            await appWindow.startDragging();
        } catch (error) {
            console.error("Failed to start dragging:", error);
        }
    }

    // 组件挂载时初始化
    onMounted(async () => {
        if (isTauriEnv.value) {
            await updateWindowState();

            // 监听窗口大小变化
            appWindow.onResized(async () => {
                await updateWindowState();
            }).then((unlisten) => {
                unlistenResized = unlisten;
            }).catch((error) => {
                console.error("Failed to listen to window resize:", error);
            });

        } else {
            console.log("Not running in Tauri environment");
        }
    });

    // 组件卸载时清理监听器
    onUnmounted(() => {
        if (unlistenResized) {
            unlistenResized();
        }
    });

    return {
        isMaximized,
        isFullscreen,
        isTauriEnv,
        minimizeWindow,
        toggleMaximize,
        closeWindow,
        startDragging,
        updateWindowState,
    };
}
