//! 自定义标题栏：跟随应用主题的深浅色，替代跟随系统的原生标题栏。
//!
//! Windows 平台的 DWM 标题栏深浅只随系统外观（App::set_window_appearance
//! 在 Windows 是 no-op），应用内主题切换无法带动它；改为
//! TitlebarOptions::appears_transparent + 应用自绘。
//! 关闭按钮语义与托盘注释一致：保存窗口状态后最小化，退出走托盘菜单。

use gpui_kit::component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
    Sizable as _,
    h_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;

/// 标题栏高度（px）。
pub const TITLE_BAR_HEIGHT: f32 = 32.;

/// 最大化/还原切换。
///
/// gpui-pre Windows 的 `zoom_window()` 只会 `SW_MAXIMIZE`、不还原，
/// 这里经 raw-window-handle 拿 HWND 直接调 `ShowWindowAsync` 切换；
/// 其他平台回落 `zoom_window()`（macOS 的 zoom 本身就是切换语义）。
fn toggle_maximize(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // Window 的固有 window_handle() 返回 GPUI 句柄，显式走 trait 拿原生句柄
        if let Ok(handle) = HasWindowHandle::window_handle(window)
            && let RawWindowHandle::Win32(win) = handle.as_raw()
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                SW_MAXIMIZE, SW_RESTORE, ShowWindowAsync,
            };
            let cmd = if window.is_maximized() { SW_RESTORE } else { SW_MAXIMIZE };
            unsafe { ShowWindowAsync(win.hwnd.get(), cmd) };
            return;
        }
    }
    window.zoom_window();
}

/// 标题栏空白区鼠标按下：单击拖动窗口，双击切换最大化。
fn on_title_bar_mouse_down(event: &MouseDownEvent, window: &mut Window) {
    match event.click_count {
        1 => window.start_window_move(),
        _ => toggle_maximize(window),
    }
}

/// 渲染主窗口标题栏。
pub fn render(
    shell: &crate::app::AppShell,
    _window: &mut Window,
    cx: &mut Context<crate::app::AppShell>,
) -> impl IntoElement {
    let theme = cx.theme();
    let title_bar_bg = theme.title_bar;
    let border = theme.border;
    let muted_fg = theme.muted_foreground;
    // 选中邮件时标题栏居中显示邮件主题；无选中时中间留空
    let subject = shell
        .detail
        .read(cx)
        .detail
        .as_ref()
        .and_then(|d| d.email.subject.clone())
        .filter(|s| !s.trim().is_empty());

    let ctrl_btn =
        |id: &'static str, icon: IconName, on_click: fn(&mut Window, &mut Context<crate::app::AppShell>)| {
            Button::new(id)
                .ghost()
                .xsmall()
                .icon(icon)
                .on_click(cx.listener(move |_, _, window, cx| on_click(window, cx)))
        };

    h_flex()
        .w_full()
        .h(px(TITLE_BAR_HEIGHT))
        .flex_none()
        .items_center()
        .px_2()
        .gap_1()
        .bg(title_bar_bg)
        .border_b_1()
        .border_color(border)
        // 左：应用名
        .child(
            div()
                .flex_none()
                .px_1()
                .text_size(px(12.5))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(muted_fg)
                .child(crate::app::MAIN_WINDOW_TITLE),
        )
        // 中：邮件主题居中（整块可拖动/双击最大化）；无选中时为空白拖动区
        .child(
            div()
                .flex_1()
                .h_full()
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |event, window, _| {
                    on_title_bar_mouse_down(event, window);
                })
                .when_some(subject, |this, s| {
                    this.child(
                        h_flex()
                            .w_full()
                            .h_full()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .child(
                                div()
                                    .max_w(px(520.))
                                    .text_size(px(12.5))
                                    .text_color(muted_fg)
                                    .line_clamp(1)
                                    .child(s),
                            ),
                    )
                }),
        )
        // 右：窗口控制按钮
        .child(ctrl_btn("win-minimize", IconName::Minus, |window, _| {
            window.minimize_window();
        }))
        .child(ctrl_btn("win-maximize", IconName::Square, |window, _| {
            toggle_maximize(window);
        }))
        .child(ctrl_btn("win-close", IconName::X, |window, _| {
            // 与原生关闭行为对齐既有设计：保存状态后最小化（退出走托盘菜单）
            crate::window_state::save(window);
            window.minimize_window();
        }))
}
