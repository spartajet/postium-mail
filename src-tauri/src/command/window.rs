///
/// 窗口管理命令模块
///
/// 本模块提供与窗口管理相关的 Tauri 命令。
/// 当前主要功能是在独立窗口中打开应用设置页，
/// 并将设置窗口居中显示在主窗口之上，以提供一致的多窗口体验。
///
/// 功能说明：
/// 1. 在独立窗口中打开设置页（前端路由 /settings）
/// 2. 若设置窗口已存在，则直接显示并聚焦，避免重复创建
/// 3. 根据主窗口的位置和尺寸，计算设置窗口的居中坐标
/// 4. 设置窗口采用无装饰（无边框）样式，先以不可见状态构建、
///    居中定位后再显示，避免窗口出现位置的闪烁
///
/// 数据流：
/// 前端 invoke 调用 → open_settings_window 命令 → Tauri 窗口管理 API
///
/// 错误处理：
/// - 所有命令返回 Result<T, MailError>
/// - 窗口的创建、显示、聚焦失败时统一返回 MailError::InvalidParam
///
/// 依赖项：
/// - AppHandle: Tauri 应用句柄，用于查找和管理窗口
/// - WebviewWindowBuilder: 构建新的 webview 窗口
///
use crate::error::MailError;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

/// 设置窗口的唯一标识（label），用于查找、显示和聚焦已存在的设置窗口。
const SETTINGS_WINDOW_LABEL: &str = "settings";
/// 设置窗口的初始宽度（逻辑像素），与居中计算时使用的子窗口宽度一致。
const SETTINGS_WINDOW_WIDTH: f64 = 960.0;
/// 设置窗口的初始高度（逻辑像素），与居中计算时使用的子窗口高度一致。
const SETTINGS_WINDOW_HEIGHT: f64 = 720.0;

/// 计算子窗口在父窗口内的居中坐标。
///
/// 通过父窗口的左上角位置和尺寸，减去子窗口的宽高后取中点，
/// 得到使子窗口在父窗口中居中显示的物理坐标。
/// 该函数不依赖任何 Tauri 运行时状态，便于进行纯函数式单元测试。
///
/// # 参数
/// - `parent_position`: 父窗口左上角的物理坐标（含装饰/边框）
/// - `parent_size`: 父窗口的物理尺寸（含装饰/边框）
/// - `child_width`: 子窗口的宽度，用于计算水平偏移
/// - `child_height`: 子窗口的高度，用于计算垂直偏移
///
/// # 返回
/// 返回子窗口左上角应放置的物理坐标 `PhysicalPosition<i32>`，
/// 当父窗口较小时坐标可能为负值（表示子窗口超出父窗口范围）。
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

///
/// 打开设置窗口
///
/// 在独立窗口中打开应用设置页，并将窗口居中显示在主窗口之上。
/// 若设置窗口已存在，则不会重复创建，而是直接显示并聚焦。
///
/// 功能说明：
/// 1. 查找是否已存在 label 为 "settings" 的窗口
/// 2. 若已存在，则直接 show 并 set_focus，立即返回（避免重复窗口）
/// 3. 若不存在，则构建一个新的无装饰（无边框）窗口，初始不可见
/// 4. 获取主窗口（"main"）的外部位置和尺寸，计算设置窗口的居中坐标
/// 5. 将设置窗口移动到居中位置后再显示，避免窗口先在默认位置闪现
/// 6. 显示并聚焦新创建的设置窗口
///
/// 参数：
/// - app: Tauri 应用句柄，用于查找和创建窗口
///
/// 返回值：
/// - Ok(()): 窗口成功打开（或已存在窗口成功聚焦）
/// - Err(MailError): 窗口的显示、聚焦或创建失败时返回
///   MailError::InvalidParam，错误信息包含底层原因
///
/// 使用场景：
/// 1. 用户点击界面中的"设置"按钮，弹出独立的设置窗口
/// 2. 设置窗口需要相对主窗口居中，提供视觉一致的多窗口体验
/// 3. 用户多次点击设置时，复用已存在的设置窗口而非重复创建
///
/// 调用示例：
/// ```typescript
/// import { invoke } from '@tauri-apps/api/tauri';
///
/// // 打开设置窗口（已存在时会自动聚焦）
/// await invoke('open_settings_window');
/// ```
///
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

///
/// 窗口管理命令的单元测试模块。
///
/// 这些测试针对 `centered_position` 这一纯函数进行验证，
/// 覆盖正常的居中场景以及副显示器（负坐标）场景，
/// 确保居中坐标计算逻辑在不同显示器布局下都正确。
///
#[cfg(test)]
mod tests {
    use super::*;

    /// 验证子窗口能在主窗口内正确居中：主窗口 (100, 80)、尺寸 1400×1000，
    /// 子窗口 960×720，期望居中坐标为 (320, 220)。
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

    /// 验证副显示器（负坐标）场景：主窗口位于 (-1600, 100)、尺寸 1200×900，
    /// 子窗口 960×720，期望居中坐标为 (-1480, 190)，确保负坐标被正确保留。
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
