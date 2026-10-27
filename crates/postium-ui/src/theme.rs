//! Postium 品牌主题（对齐旧 Tauri 版 app.css 的紫色渐变视觉）：
//! - 主色 #7c3aed / 次色 #a78bfa
//! - 暗色为深紫蓝基调（#1a1a35 系），亮色为微紫白
//! - 主内容区叠加低透明度渐变光球（模拟旧版 orb 装饰）

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, linear_gradient};

/// 品牌主色
pub fn primary() -> Hsla {
    gpui_kit::rgb(0x7c3aed).into()
}
/// 品牌次色（浅紫）
pub fn secondary() -> Hsla {
    gpui_kit::rgb(0xa78bfa).into()
}

/// 在 `Theme::change` 之后调用：覆盖品牌配色（change 会重载注册主题，必须后置）。
pub fn apply_brand(cx: &mut App) {
    use gpui_kit::component::ActiveTheme as _;
    let mode = cx.theme().mode;
    Theme::update(cx, |theme| {
        let c = &mut theme.colors;
        match mode {
            ThemeMode::Dark => {
                c.background = gpui_kit::rgb(0x161632).into();
                c.sidebar = gpui_kit::rgb(0x181832).into();
                c.title_bar = gpui_kit::rgb(0x15152c).into();
                c.border = gpui_kit::rgb(0x32325c).into();
                c.muted = gpui_kit::rgb(0x22224a).into();
            }
            ThemeMode::Light => {
                c.background = gpui_kit::rgb(0xf7f5fc).into();
                c.sidebar = gpui_kit::rgb(0xf1eefc).into();
                c.title_bar = gpui_kit::rgb(0xedeaf9).into();
                c.border = gpui_kit::rgb(0xe4e1f0).into();
                c.muted = gpui_kit::rgb(0xece9f8).into();
            }
        }
        // 品牌主色（深浅同色）
        c.accent = primary();
        c.accent_foreground = gpui_kit::white().into();
        c.secondary = secondary();
    });
}

/// 主内容区渐变背景（旧版 bg 渐变的 GPUI 等价，元素级 .bg()）。
pub fn brand_gradient(cx: &App) -> gpui_kit::Background {
    use gpui_kit::component::ActiveTheme as _;
    if cx.theme().mode == ThemeMode::Dark {
        linear_gradient(
            135.0,
            gpui_kit::linear_color_stop(gpui_kit::rgb(0x131328), 0.0),
            gpui_kit::linear_color_stop(gpui_kit::rgb(0x1d1d40), 1.0),
        )
    } else {
        linear_gradient(
            135.0,
            gpui_kit::linear_color_stop(gpui_kit::rgb(0xf8fafc), 0.0),
            gpui_kit::linear_color_stop(gpui_kit::rgb(0xf1ecfd), 1.0),
        )
    }
}

/// 旧版 orb 光球的 GPUI 近似：大面积低透明度渐变圆，绝对定位、不接收事件。
/// 放在主内容区容器（relative + overflow_hidden）的第一个 child。
pub fn orbs(cx: &App) -> impl IntoElement {
    use gpui_kit::component::ActiveTheme as _;
    let opacity = if cx.theme().mode == ThemeMode::Dark { 0.16 } else { 0.10 };

    // 三个独立 orb（各自 absolute，无交互处理即不影响点击）
    gpui_kit::div()
        .absolute()
        .size_full()
        .overflow_hidden()
        // orb-1：右上，紫渐变
        .child(
            gpui_kit::div()
                .absolute()
                .top(px(-180.))
                .right(px(-160.))
                .size(px(560.))
                .rounded_full()
                .bg(linear_gradient(135.0, gpui_kit::linear_color_stop(gpui_kit::rgb(0x7c3aed), 0.0), gpui_kit::linear_color_stop(gpui_kit::rgb(0xa78bfa), 1.0)))
                .opacity(opacity),
        )
        // orb-2：左下，紫→玫瑰
        .child(
            gpui_kit::div()
                .absolute()
                .bottom(px(-200.))
                .left(px(-140.))
                .size(px(520.))
                .rounded_full()
                .bg(linear_gradient(135.0, gpui_kit::linear_color_stop(gpui_kit::rgb(0xa78bfa), 0.0), gpui_kit::linear_color_stop(gpui_kit::rgb(0xf43f5e), 1.0)))
                .opacity(opacity * 0.7),
        )
        // orb-3：中上偏左，紫→蓝（更小更淡）
        .child(
            gpui_kit::div()
                .absolute()
                .top(px(80.))
                .left(px(280.))
                .size(px(380.))
                .rounded_full()
                .bg(linear_gradient(135.0, gpui_kit::linear_color_stop(gpui_kit::rgb(0x7c3aed), 0.0), gpui_kit::linear_color_stop(gpui_kit::rgb(0x3b82f6), 1.0)))
                .opacity(opacity * 0.6),
        )
}

/// 侧栏「写邮件」渐变按钮（旧版 compose-btn：紫渐变 + 白字）。
pub fn compose_button(
    label: gpui_kit::SharedString,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut App) + 'static,
) -> impl IntoElement {
    gpui_kit::div()
        .id("compose-btn")
        .h(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded(gpui_kit::px(8.))
        .cursor_pointer()
        .bg(linear_gradient(135.0, gpui_kit::linear_color_stop(gpui_kit::rgb(0x7c3aed), 0.0), gpui_kit::linear_color_stop(gpui_kit::rgb(0xa78bfa), 1.0)))
        .hover(|s| s.opacity(1.1f32))
        .text_color(gpui_kit::white())
        .text_size(gpui_kit::px(13.))
        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
        .child(
            gpui_kit::component::Icon::new(gpui_kit_assets::IconName::MailPlus).size_4(),
        )
        .child(label)
        .on_click(move |e, w, cx| on_click(e, w, cx))
}

use gpui_kit::{px, ParentElement as _, InteractiveElement as _, IntoElement, StatefulInteractiveElement as _, Styled};

/// 应用指定主题模式（Theme::change + 品牌覆盖），供壳/设置页共用。
pub fn apply_mode(mode: gpui_kit::component::ThemeMode, cx: &mut App) {
    gpui_kit::component::Theme::change(mode, None, cx);
    apply_brand(cx);
}

/// 由偏好与系统外观解析实际主题模式（"system" 跟随系统）。
pub fn resolve_mode(pref: &str, appearance: gpui_kit::WindowAppearance) -> gpui_kit::component::ThemeMode {
    match pref {
        "light" => gpui_kit::component::ThemeMode::Light,
        "system" => match appearance {
            gpui_kit::WindowAppearance::Light => gpui_kit::component::ThemeMode::Light,
            _ => gpui_kit::component::ThemeMode::Dark,
        },
        _ => gpui_kit::component::ThemeMode::Dark,
    }
}
