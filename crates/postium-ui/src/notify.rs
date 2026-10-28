//! 全局 Toast 总线：任何上下文发消息，主窗口消费并弹 gpui-kit 通知。

use gpui_kit::component::WindowExt as _;
use gpui_kit::{AnyWindowHandle, App, SharedString};
use std::sync::OnceLock;

static MAIN_WINDOW: OnceLock<AnyWindowHandle> = OnceLock::new();
static TX: OnceLock<smol::channel::Sender<SharedString>> = OnceLock::new();

/// 记录主窗口句柄并启动消费循环（AppShell 装配时调用一次）。
pub fn setup(window: &gpui_kit::Window, cx: &mut App) {
    let _ = MAIN_WINDOW.set(window.window_handle());
    let (tx, rx) = smol::channel::bounded::<SharedString>(64);
    let _ = TX.set(tx);
    cx.spawn(async move |cx| {
        while let Ok(msg) = rx.recv().await {
            let Some(handle) = MAIN_WINDOW.get().copied() else { continue };
            cx.update(|cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    window.push_notification(msg.clone(), cx);
                });
            });
        }
    })
    .detach();
}

/// 发送 Toast（线程安全，任意上下文可用）。
///
/// Toast 即用户可见提示，统一在此记录 info 日志，保证「UI 提示必进日志」。
pub fn toast(msg: impl Into<SharedString>) {
    let msg = msg.into();
    tracing::info!(target: "postium::ui", "Toast: {msg}");
    if let Some(tx) = TX.get() {
        let _ = tx.try_send(msg);
    }
}
