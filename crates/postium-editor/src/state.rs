//! EmailEditor 状态实体：封装 markdown-editor 内核，对外提供邮件域接口。

use gpui_kit::*;
use markdown_editor::{BlockEditorEvent, BlockEditorState};

use crate::convert::{email_html_to_md, md_to_email_html};

/// 对外事件（邮件域语义）。
pub enum EmailEditorEvent {
    /// 内容变化，`html` 为当前导出结果。
    Changed { html: String },
    /// Ctrl+K：宿主应弹出链接输入并调用 [`EmailEditorState::set_link`]。
    EditLinkRequested { url: Option<SharedString> },
}

/// 编辑器状态：持有内核实体，桥接事件。
pub struct EmailEditorState {
    editor: Entity<BlockEditorState>,
}

impl EventEmitter<EmailEditorEvent> for EmailEditorState {}

impl EmailEditorState {
    /// 空白编辑器。
    pub fn new(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let editor = cx.new(|cx| BlockEditorState::new(window, cx));
        Self::wrap(editor, cx)
    }

    /// 从既有邮件 HTML 创建（回复/草稿编辑场景）。
    pub fn from_html(html: &str, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let editor = cx.new(|cx| {
            BlockEditorState::new(window, cx).with_source(&email_html_to_md(html))
        });
        Self::wrap(editor, cx)
    }

    fn wrap(editor: Entity<BlockEditorState>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.subscribe(&editor, |this: &mut Self, _, event: &BlockEditorEvent, cx| {
                match event {
                    BlockEditorEvent::Changed { .. } => {
                        cx.emit(EmailEditorEvent::Changed {
                            html: this.html(cx),
                        });
                    }
                    BlockEditorEvent::EditLinkRequested { url } => {
                        cx.emit(EmailEditorEvent::EditLinkRequested { url: url.clone() });
                    }
                    _ => {}
                }
            })
            .detach();
            Self { editor }
        })
    }

    /// 内核实体（宿主需要 focus / 集成滚动容器时使用）。
    pub fn editor(&self) -> &Entity<BlockEditorState> {
        &self.editor
    }

    /// 导出为可发送的邮件 HTML。
    pub fn html(&self, cx: &App) -> String {
        md_to_email_html(&self.editor.read(cx).source())
    }

    /// 导入邮件 HTML（替换当前内容）。
    pub fn set_html(&mut self, html: &str, cx: &mut Context<Self>) {
        let md = email_html_to_md(html);
        self.editor.update(cx, |editor, cx| {
            let _ = editor.set_source(&md, cx);
        });
        cx.emit(EmailEditorEvent::Changed { html: self.html(cx) });
        cx.notify();
    }

    /// 是否为空文档。
    pub fn is_empty(&self, cx: &App) -> bool {
        self.editor.read(cx).source().trim().is_empty()
    }

    /// 设置/取消光标处链接（配合 [`EmailEditorEvent::EditLinkRequested`]）。
    pub fn set_link(&mut self, url: Option<&str>, cx: &mut App) {
        self.editor.update(cx, |editor, cx| {
            editor.set_link(url, cx);
        });
    }
}
