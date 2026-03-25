//! 系统托盘模块
//!
//! 管理应用程序的系统托盘图标和菜单

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, Runtime};

/// 初始化系统托盘
///
/// # 参数
///
/// * `app` - Tauri 应用实例
///
/// # 返回
///
/// 成功返回 Ok(())
pub fn init_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    // 创建菜单项
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    // 创建菜单
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    // 创建系统托盘
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                }
            }
        })
        .build(app)?;

    tracing::info!("系统托盘初始化完成");
    setup_window_close_behavior(app);

    Ok(())
}

/// 设置窗口关闭事件处理
///
/// 当用户点击关闭按钮时，隐藏窗口而不是退出应用
///
/// # 参数
///
/// * `app` - Tauri 应用实例
pub fn setup_window_close_behavior<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_clone.hide();
                tracing::info!("窗口已隐藏到系统托盘");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    // 托盘功能需要在实际应用中测试
    // 这里只放置单元测试占位
}
