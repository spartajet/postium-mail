//! 主窗口壳：侧栏 + 主区域（各视图由后续任务充实）。

use std::sync::Arc;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use postium_core::api::{CoreApi, SyncEventRx};

/// 打开主窗口（应用启动时调用一次）。
pub fn open_main_window(api: Arc<CoreApi>, sync_rx: SyncEventRx, cx: &mut App) {
    gpui_kit::open_window(
        WindowOptions::default(),
        cx,
        |_, cx| cx.new(|cx| AppShell::new(api, sync_rx, cx)),
    )
    .expect("打开主窗口失败");
    cx.activate(true);
}

/// 应用主壳实体：持有核心服务与全局状态，后续任务逐步充实。
pub struct AppShell {
    #[allow(dead_code)] // Task 3+ 起消费
    api: Arc<CoreApi>,
    #[allow(dead_code)] // Task 7 起消费
    sync_rx: SyncEventRx,
}

impl AppShell {
    fn new(api: Arc<CoreApi>, sync_rx: SyncEventRx, _cx: &mut Context<Self>) -> Self {
        Self { api, sync_rx }
    }
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            // 侧栏（Task 3 实装）
            .child(
                v_flex()
                    .w(px(220.))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .p_3()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::BOLD)
                            .child("Postium Mail"),
                    ),
            )
            // 主区域（Task 4 实装邮件列表）
            .child(
                v_flex()
                    .flex_1()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(cx.theme().muted_foreground)
                            .child("邮件列表（Task 4 实装）"),
                    ),
            )
    }
}
