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
                    if has_notification_layer(window) {
                        window.push_notification(msg.clone(), cx);
                    }
                });
            });
        }
    })
    .detach();
}

/// 发送 Toast（线程安全，任意上下文可用）。
pub fn toast(msg: impl Into<SharedString>) {
    if let Some(tx) = TX.get() {
        let _ = tx.try_send(msg.into());
    }
}

/// 该窗口是否挂了 gpui-base Root（即 gpui_kit::open_window 打开的主窗口）。
/// e2e headless 直接建壳的窗口没有组件通知层，推送会 panic——跳过。
fn has_notification_layer(window: &gpui_kit::Window) -> bool {
    matches!(window.root::<gpui_kit::base::Root>(), Some(Some(_)))
}
