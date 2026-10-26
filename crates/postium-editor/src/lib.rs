//! postium-editor：Postium Mail 原生邮件编辑器组件。
//!
//! 用法（宿主应用）：
//! ```ignore
//! gpui_kit::init(cx);
//! postium_editor::init(cx);
//! let state = EmailEditorState::from_html(original_html, window, cx);
//! // 渲染：EmailEditor::new(&state)
//! // 导出：state.html(cx) → 发送
//! ```

mod convert;
mod state;
mod view;

pub use convert::{email_html_to_md, md_to_email_html};
pub use state::{EmailEditorEvent, EmailEditorState};
pub use view::EmailEditor;

/// 注册编辑器键绑定。在 `gpui_kit::init` 之后调用一次。
pub fn init(cx: &mut gpui_kit::App) {
    markdown_editor::init(cx);
}
