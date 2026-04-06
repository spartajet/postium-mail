use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    App, Emitter, Manager,
};

pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show", "显示窗口").build(app)?;
    let new_email_item = MenuItemBuilder::with_id("new_email", "写邮件").build(app)?;
    let sync_item = MenuItemBuilder::with_id("sync", "同步").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&show_item, &new_email_item, &sync_item, &quit_item])
        .build()?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(move |app, event| {
            tracing::debug!("托盘菜单事件: {}", event.id().as_ref());
            match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "new_email" => {
                let _ = app.emit("tray-action", "compose");
            }
            "sync" => {
                let _ = app.emit("tray-action", "sync");
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
