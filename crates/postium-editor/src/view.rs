//! EmailEditor 视图：工具栏 + WYSIWYG 编辑区。

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex, v_flex,
};
use gpui_kit::*;
use markdown_editor::{BlockEditor, BlockKind, EditorCommand, MarkKind};

use crate::state::{EmailEditorState, EmailEditorEvent};

/// 邮件编辑器视图：一行工具栏 + `BlockEditor` 编辑区。
///
/// 工具栏激活态由内核 `active_marks()` 与光标块类型驱动；
/// 命令经 [`markdown_editor::BlockEditorState::run`] 分发，
/// 执行后焦点归还编辑器。
#[derive(IntoElement)]
pub struct EmailEditor {
    state: gpui_kit::Entity<EmailEditorState>,
}

impl EmailEditor {
    pub fn new(state: &gpui_kit::Entity<EmailEditorState>) -> Self {
        Self { state: state.clone() }
    }
}

impl RenderOnce for EmailEditor {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = self.state;
        let editor = state.read(cx).editor().clone();

        // 工具栏激活态
        let (marks, kind) = {
            let e = editor.read(cx);
            (e.model().active_marks(), e.block_kind_at_caret())
        };
        let is_heading = |level: u8| {
            matches!(kind, Some(BlockKind::Heading { level: l }) if l == level)
        };

        // （id, label, tooltip, 激活态, 命令）
        let items: Vec<(
            &'static str,
            &str,
            &str,
            bool,
            EditorCommand,
        )> = vec![
            ("bold", "**B**", "加粗 Ctrl+B", marks.has(MarkKind::Bold), EditorCommand::ToggleMark(MarkKind::Bold)),
            ("italic", "*I*", "斜体 Ctrl+I", marks.has(MarkKind::Italic), EditorCommand::ToggleMark(MarkKind::Italic)),
            ("strike", "S̶", "删除线 Ctrl+Shift+X", marks.has(MarkKind::Strikethrough), EditorCommand::ToggleMark(MarkKind::Strikethrough)),
            ("underline", "U", "下划线 Ctrl+U", marks.has(MarkKind::Underline), EditorCommand::ToggleMark(MarkKind::Underline)),
            ("h1", "H1", "一级标题", is_heading(1), EditorCommand::SetBlockKind(BlockKind::Heading { level: 1 })),
            ("h2", "H2", "二级标题", is_heading(2), EditorCommand::SetBlockKind(BlockKind::Heading { level: 2 })),
            ("para", "¶", "正文段落 Ctrl+Alt+0", matches!(kind, Some(BlockKind::Paragraph)), EditorCommand::SetBlockKind(BlockKind::Paragraph)),
            ("bullet", "• 列表", "无序列表", false, EditorCommand::ToggleList { ordered: false }),
            ("ordered", "1. 列表", "有序列表", false, EditorCommand::ToggleList { ordered: true }),
            ("task", "☑ 任务", "任务列表", false, EditorCommand::ToggleTask),
            ("indent", "→", "增加缩进 Tab", false, EditorCommand::Indent),
            ("outdent", "←", "减少缩进 Shift+Tab", false, EditorCommand::Outdent),
        ];

        let mut toolbar = h_flex().gap_1().items_center().flex_none();
        for (id, label, tooltip, active, command) in items {
            let editor = editor.clone();
            let btn = Button::new(id).xsmall().ghost().label(label).tooltip(tooltip);
            let btn = if active { btn.primary() } else { btn };
            toolbar = toolbar.child(btn.on_click(move |_, window, cx| {
                editor.update(cx, |state, cx| {
                    let _ = state.run(command.clone(), cx);
                });
                window.focus(&editor.read(cx).focus_handle(cx), cx);
            }));
        }
        for (id, label, redo) in [("undo", "↶", false), ("redo", "↷", true)] {
            let editor = editor.clone();
            toolbar = toolbar.child(
                Button::new(id)
                    .xsmall()
                    .ghost()
                    .label(label)
                    .tooltip(if redo { "重做 Ctrl+Shift+Z" } else { "撤销 Ctrl+Z" })
                    .on_click(move |_, window, cx| {
                        editor.update(cx, |state, cx| {
                            if redo {
                                state.redo(cx);
                            } else {
                                state.undo(cx);
                            }
                        });
                        window.focus(&editor.read(cx).focus_handle(cx), cx);
                    }),
            );
        }

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .px_2()
                    .py_1()
                    .gap_1()
                    .flex_none()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
                    .child(toolbar),
            )
            .child(
                gpui_kit::div()
                    .id("email-editor-body")
                    .flex_1()
                    .min_h(gpui_kit::px(220.))
                    .overflow_hidden()
                    .p_4()
                    .child(BlockEditor::new(&editor)),
            )
    }
}

// 注：EmailEditorEvent 从 crate 根导出（见 lib.rs）。
