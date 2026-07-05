use crate::error::MailError;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const SETTINGS_WINDOW_LABEL: &str = "settings";
const SETTINGS_WINDOW_WIDTH: f64 = 960.0;
const SETTINGS_WINDOW_HEIGHT: f64 = 720.0;

fn centered_position(
    parent_position: PhysicalPosition<i32>,
    parent_size: PhysicalSize<u32>,
    child_width: f64,
    child_height: f64,
) -> PhysicalPosition<i32> {
    let x = parent_position.x + ((parent_size.width as f64 - child_width) / 2.0).round() as i32;
    let y = parent_position.y + ((parent_size.height as f64 - child_height) / 2.0).round() as i32;
    PhysicalPosition::new(x, y)
}

#[tauri::command]
#[specta::specta]
pub async fn open_settings_window(app: AppHandle) -> Result<(), MailError> {
    if let Some(window) = app.get_webview_window(SETTINGS_WINDOW_LABEL) {
        window
            .show()
            .map_err(|err| MailError::InvalidParam(format!("显示设置窗口失败: {err}")))?;
        window
            .set_focus()
            .map_err(|err| MailError::InvalidParam(format!("聚焦设置窗口失败: {err}")))?;
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(
        &app,
        SETTINGS_WINDOW_LABEL,
        WebviewUrl::App("/settings".into()),
    )
    .title("Postium Mail Settings")
    .inner_size(SETTINGS_WINDOW_WIDTH, SETTINGS_WINDOW_HEIGHT)
    .min_inner_size(760.0, 560.0)
    .decorations(false)
    .visible(false)
    .build()
    .map_err(|err| MailError::InvalidParam(format!("创建设置窗口失败: {err}")))?;

    if let Some(main_window) = app.get_webview_window("main")
        && let (Ok(parent_position), Ok(parent_size)) =
            (main_window.outer_position(), main_window.outer_size())
    {
        let position = centered_position(
            parent_position,
            parent_size,
            SETTINGS_WINDOW_WIDTH,
            SETTINGS_WINDOW_HEIGHT,
        );
        let _ = window.set_position(position);
    }

    window
        .show()
        .map_err(|err| MailError::InvalidParam(format!("显示设置窗口失败: {err}")))?;
    window
        .set_focus()
        .map_err(|err| MailError::InvalidParam(format!("聚焦设置窗口失败: {err}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_child_window_in_parent_window() {
        let position = centered_position(
            PhysicalPosition::new(100, 80),
            PhysicalSize::new(1400, 1000),
            960.0,
            720.0,
        );

        assert_eq!(position, PhysicalPosition::new(320, 220));
    }

    #[test]
    fn preserves_negative_coordinates_for_secondary_monitors() {
        let position = centered_position(
            PhysicalPosition::new(-1600, 100),
            PhysicalSize::new(1200, 900),
            960.0,
            720.0,
        );

        assert_eq!(position, PhysicalPosition::new(-1480, 190));
    }
}
