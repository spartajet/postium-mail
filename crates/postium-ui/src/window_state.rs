//! 窗口位置/尺寸持久化（替代 tauri-plugin-window-state）：~/.postium/window-state.json

use std::path::PathBuf;

use gpui_kit::{Bounds, Pixels, Window};

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

fn state_path() -> PathBuf {
    crate::resolve_data_dir().join("window-state.json")
}

/// 启动时恢复（无记录返回 None 用默认）。
pub fn saved_bounds() -> Option<Bounds<Pixels>> {
    let content = std::fs::read_to_string(state_path()).ok()?;
    let state: WindowState = serde_json::from_str(&content).ok()?;
    Some(Bounds {
        origin: gpui_kit::Point {
            x: gpui_kit::px(state.x as f32),
            y: gpui_kit::px(state.y as f32),
        },
        size: gpui_kit::size(
            gpui_kit::px(state.width as f32),
            gpui_kit::px(state.height as f32),
        ),
    })
}

/// 保存当前窗口状态。
pub fn save(window: &Window) {
    let bounds = window.bounds();
    // 最小化时 Windows 返回 -32000 系坐标，写入会毁掉已记忆的位置
    if f32::from(bounds.origin.x) < -10000.0 || f32::from(bounds.origin.y) < -10000.0 {
        return;
    }
    let state = WindowState {
        x: f32::from(bounds.origin.x) as i32,
        y: f32::from(bounds.origin.y) as i32,
        width: f32::from(bounds.size.width) as u32,
        height: f32::from(bounds.size.height) as u32,
        maximized: window.is_maximized(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&state) {
        let _ = std::fs::write(state_path(), json);
    }
}
