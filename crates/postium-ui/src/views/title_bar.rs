//! 自定义标题栏：跟随应用主题的深浅色，替代跟随系统的原生标题栏。
//!
//! Windows 平台的 DWM 标题栏深浅只随系统外观（App::set_window_appearance
//! 在 Windows 是 no-op），应用内主题切换无法带动它；改为
//! TitlebarOptions::appears_transparent + 应用自绘。
//!
//! 拖动与窗口控制按钮走 WM_NCHITTEST 机制：元素标记
//! `window_control_area` 后，系统接管拖动、双击最大化与按钮点击
//! （gpui-pre 的 `start_window_move` 在 Windows 是空操作，不能用于拖动）。
//! 关闭按钮语义与托盘注释一致：保存窗口状态后最小化，退出走托盘菜单。

use gpui_kit::component::{
    ActiveTheme as _,
    h_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;

/// 标题栏高度（px）。
pub const TITLE_BAR_HEIGHT: f32 = 32.;

/// 系统控制按钮（拖动/点击均由 WM_NCHITTEST 交给系统处理）。
fn control_button(
    id: &'static str,
    icon: IconName,
    area: WindowControlArea,
    hover_bg: gpui_kit::Hsla,
    cx: &Context<crate::app::AppShell>,
) -> impl IntoElement {
    let fg = cx.theme().secondary_foreground;
    div()
        .id(id)
        .flex()
        .w(px(35.))
        .h_full()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_color(fg)
        .hover(move |s| s.bg(hover_bg))
        .window_control_area(area)
        .child(gpui_kit::component::Icon::new(icon).size_3p5())
}

/// 渲染主窗口标题栏。
pub fn render(
    shell: &crate::app::AppShell,
    window: &mut Window,
    cx: &mut Context<crate::app::AppShell>,
) -> impl IntoElement {
    let theme = cx.theme();
    let title_bar_bg = theme.title_bar;
    let border = theme.border;
    let muted_fg = theme.muted_foreground;
    let secondary_hover = theme.secondary_hover;
    let maximized = window.is_maximized();
    // 选中邮件时标题栏居中显示邮件主题；无选中时中间留空
    let subject = shell
        .detail
        .read(cx)
        .detail
        .as_ref()
        .and_then(|d| d.email.subject.clone())
        .filter(|s| !s.trim().is_empty());

    h_flex()
        .w_full()
        .h(px(TITLE_BAR_HEIGHT))
        .flex_none()
        .items_center()
        .bg(title_bar_bg)
        .border_b_1()
        .border_color(border)
        // 左：应用名（点击穿透给标题栏拖动即可，无需交互）
        .child(
            div()
                .flex_none()
                .px_2()
                .text_size(px(12.5))
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(muted_fg)
                .window_control_area(WindowControlArea::Drag)
                .child(crate::app::MAIN_WINDOW_TITLE),
        )
        // 中：邮件主题居中（整块标记为拖动区，双击最大化由系统处理）
        .child(
            div()
                .flex_1()
                .h_full()
                .overflow_hidden()
                .window_control_area(WindowControlArea::Drag)
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
        // 右：系统窗口控制按钮（最小化/最大化-还原）
        .child(control_button(
            "win-minimize",
            IconName::WindowMinimize,
            WindowControlArea::Min,
            secondary_hover,
            cx,
        ))
        .child(control_button(
            "win-maximize",
            if maximized {
                IconName::WindowRestore
            } else {
                IconName::WindowMaximize
            },
            WindowControlArea::Max,
            secondary_hover,
            cx,
        ))
        // 关闭：应用层处理（保存窗口状态后最小化；退出走托盘菜单）
        .child(
            div()
                .id("win-close")
                .flex()
                .w(px(35.))
                .h_full()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .text_color(theme.foreground)
                .hover(move |s| s.bg(theme.danger).text_color(theme.danger_foreground))
                .on_mouse_down(MouseButton::Left, |_, window, _| {
                    crate::window_state::save(window);
                    window.minimize_window();
                })
                .child(gpui_kit::component::Icon::new(IconName::WindowClose).size_3p5()),
        )
}
