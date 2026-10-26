//! 写信窗口（独立常驻窗口——spike 验证模式）：postium-editor 集成。
//! 移植自 src/lib/components/email/ComposeModal.svelte 的字段与发送状态机。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use postium_core::api::CoreApi;
use postium_core::service::email_service::{
    ComposeAttachmentInput,
    SendEmailRequest,
    SendEmailResponse,
};
use postium_editor::{EmailEditor, EmailEditorState};

use crate::i18n::t;
use crate::runtime::spawn_tokio;

/// 写信模式。
#[derive(Clone, Debug)]
pub enum ComposeMode {
    New,
    Reply { email_id: i32, to: String, subject: String, original_html: String },
    ReplyAll { email_id: i32, to: String, subject: String, original_html: String },
    Forward { email_id: i32, subject: String, original_html: String },
}

/// 去除主题的 Re:/Fwd: 前缀（对齐旧版 /^(Re|Fwd):\s*/i）。
fn strip_prefixes(subject: &str) -> String {
    let mut s = subject.trim().to_string();
    loop {
        let lower = s.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("re:") {
            s = s[3..].trim_start().to_string();
        } else if let Some(rest) = lower.strip_prefix("fwd:") {
            s = s[4..].trim_start().to_string();
        } else {
            return s;
        }
    }
}

/// 打开独立写信窗口（居中 920×720）。
pub fn open_compose_window(mode: ComposeMode, api: &Arc<CoreApi>, cx: &mut App) {
    let api = api.clone();
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(920.), px(720.)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| ComposeView::new(mode, api, window, cx)),
    )
    .expect("打开写信窗口失败");
    cx.activate(true);
}

pub struct ComposeView {
    api: Arc<CoreApi>,
    to: Entity<InputState>,
    cc: Entity<InputState>,
    bcc: Entity<InputState>,
    subject: Entity<InputState>,
    editor: Entity<EmailEditorState>,
    attachments: Vec<(std::path::PathBuf, String, u64)>, // (路径, 文件名, 大小)
    status: SharedString,
    sending: bool,
    draft_email_id: Option<i32>,
    draft_status: SharedString,
    /// 发件账号 ID（点击工具条账号按钮循环切换）
    selected_account: Option<(i32, String)>,
    close_handle: Option<gpui_kit::AnyWindowHandle>,
}

impl ComposeView {
    fn new(mode: ComposeMode, api: Arc<CoreApi>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 初始内容：回复/转发引入原文（对齐旧版 showReply/showForward 语义）
        let (to_init, subject_init, editor_html) = match &mode {
            ComposeMode::New => (String::new(), String::new(), String::new()),
            ComposeMode::Reply { to, subject, original_html, .. } => {
                let clean = strip_prefixes(subject);
                (to.clone(), format!("Re: {clean}"), original_html.clone())
            }
            ComposeMode::ReplyAll { to, subject, original_html, .. } => {
                let clean = strip_prefixes(subject);
                (to.clone(), format!("Re: {clean}"), original_html.clone())
            }
            ComposeMode::Forward { subject, original_html, .. } => {
                let clean = strip_prefixes(subject);
                (String::new(), format!("Fwd: {clean}"), original_html.clone())
            }
        };

        let editor = if editor_html.is_empty() {
            EmailEditorState::new(window, cx)
        } else {
            let quoted = format!(
                "<p><br></p><blockquote style=\"border-left:3px solid #cbd5e1;margin:8px 0;padding:4px 12px;color:#555\">{editor_html}</blockquote>"
            );
            EmailEditorState::from_html(&quoted, window, cx)
        };

        let mut make_input = |v: &str, cx: &mut Context<Self>| {
            let v = v.to_string();
            cx.new(|cx| {
                if v.is_empty() {
                    InputState::new(window, cx)
                } else {
                    InputState::new(window, cx).default_value(v.clone())
                }
            })
        };

        let mut this = Self {
            api: api.clone(),
            to: make_input(&to_init, cx),
            cc: make_input("", cx),
            bcc: make_input("", cx),
            subject: make_input(&subject_init, cx),
            editor,
            attachments: Vec::new(),
            status: "就绪".into(),
            sending: false,
            draft_email_id: None,
            draft_status: " ".into(),
            selected_account: None,
            close_handle: None,
        };

        // 装载账号列表（默认选中第一个；回复/转发优先原邮件账号）
        let preferred_account = match &mode {
            ComposeMode::Reply { email_id, .. } | ComposeMode::ReplyAll { email_id, .. } | ComposeMode::Forward { email_id, .. } => Some(*email_id),
            ComposeMode::New => None,
        };
        let rx = spawn_tokio(async move { api.list_accounts().await.unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            if let Ok(accounts) = rx.recv().await {
                this.update(cx, |view, cx| {
                    if let Some(acc) = accounts.first() {
                        view.selected_account = Some((acc.id, acc.name.clone()));
                    }
                    let _ = preferred_account;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        this
    }

    fn add_attachment(&mut self, _: &ClickEvent, _: &mut Window, _cx: &mut Context<Self>) {
        // rfd 同步对话框放后台线程
        let (tx, rx) = smol::channel::bounded(1);
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new().pick_files();
            let _ = tx.try_send(picked);
        });
        cx_spawn_attachments(rx);
    }

    fn value_of(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().to_string()
    }

    fn collect_recipients(&self, cx: &App) -> Vec<String> {
        let raw = Self::value_of(&self.to, cx);
        raw.split([',', ';', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// 循环切换发件账号。
    fn cycle_account(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let current = self.selected_account.clone();
        let rx = spawn_tokio(async move { api.list_accounts().await.unwrap_or_default() });
        cx.spawn(async move |this, cx| {
            if let Ok(accounts) = rx.recv().await
                && accounts.len() > 1
            {
                let idx = current
                    .and_then(|(id, _)| accounts.iter().position(|a| a.id == id))
                    .map(|i| (i + 1) % accounts.len())
                    .unwrap_or(0);
                let acc = &accounts[idx];
                this.update(cx, |view, cx| {
                    view.selected_account = Some((acc.id, acc.name.clone()));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// 立即保存草稿。
    fn save_draft_now(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.persist_draft(cx);
    }

    /// 丢弃草稿（删除服务器草稿并关窗）。
    fn discard_draft(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.draft_email_id else {
            self.close_window(cx);
            return;
        };
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.delete_draft(id).await });
        cx.spawn(async move |this, cx| {
            if rx.recv().await.is_ok() {
                this.update(cx, |view, cx| {
                    view.close_window(cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn close_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.close_handle {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    fn persist_draft(&mut self, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let api = self.api.clone();
        let (account_id, _) = match self.selected_account.clone() {
            Some(a) => a,
            None => return,
        };
        let to = Self::value_of(&self.to, cx);
        let cc = Self::value_of(&self.cc, cx);
        let subject = Self::value_of(&self.subject, cx);
        let body_html = self.editor.read(cx).html(cx);
        let draft_id = self.draft_email_id;
        self.draft_status = t("email.draftSaving").to_string().into();
        cx.notify();
        let rx = spawn_tokio(async move {
            api.save_draft(postium_core::service::email_service::SaveDraftRequest {
                account_id,
                to: split_addresses(&to),
                cc: split_addresses(&cc),
                bcc: Vec::new(),
                subject,
                body_html,
                body_text: String::new(),
                attachments: Vec::new(),
                draft_id,
            })
            .await
        });
        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(resp)) => {
                    this.update(cx, |view, cx| {
                        view.draft_email_id = Some(resp.draft_id);
                        view.draft_status = t("email.draftSaved").to_string().into();
                        cx.notify();
                    })
                    .ok();
                }
                _ => {
                    this.update(cx, |view, cx| {
                        view.draft_status = t("email.draftSaveFailed").to_string().into();
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    fn send(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let to = self.collect_recipients(cx);
        if to.is_empty() {
            self.status = t("email.sendRequiresRecipient");
            cx.notify();
            return;
        }
        let subject = Self::value_of(&self.subject, cx);
        if subject.trim().is_empty() {
            self.status = t("email.sendRequiresSubject");
            cx.notify();
            return;
        }
        let body_html = self.editor.read(cx).html(cx);
        if body_html.trim().is_empty() {
            self.status = t("email.sendRequiresBody").to_string().into();
            cx.notify();
            return;
        }

        // 发件账号（未选择时回落第一个）
        let api = self.api.clone();
        let draft_id = self.draft_email_id;
        let cc: Vec<String> = Self::value_of(&self.cc, cx)
            .split([',', ';', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let attachments: Vec<ComposeAttachmentInput> = self
            .attachments
            .iter()
            .map(|(path, name, size)| ComposeAttachmentInput {
                path: path.to_string_lossy().to_string(),
                filename: Some(name.clone()),
                content_type: None,
                size: Some(*size as i64),
            })
            .collect();

        self.sending = true;
        self.status = format!("{}...", t("email.send")).into();
        cx.notify();

        let preferred_account = self.selected_account.clone();
        let to_raw = to.join(", ");
        let api_parse = api.clone();
        let rx = spawn_tokio(async move {
            let account_id = match preferred_account {
                Some((id, _)) => id,
                None => {
                    let accounts = api.list_accounts().await.unwrap_or_default();
                    let Some(account) = accounts.first() else {
                        return Err(postium_core::error::MailError::AccountNotFound(0));
                    };
                    account.id
                }
            };
            // 收件人解析验证（对齐旧版 RecipientField 校验）
            let parsed = api_parse.parse_email_addresses(to_raw.clone()).await;
            if let Ok(p) = &parsed
                && !p.invalid.is_empty()
            {
                return Err(postium_core::error::MailError::InvalidParam("invalid recipients".into()));
            }
            let _ = parsed;
            api.send_email(SendEmailRequest {
                account_id,
                to,
                cc,
                bcc: Vec::new(),
                subject,
                body_html,
                body_text: String::new(),
                attachments,
                draft_id,
            })
            .await
        });

        cx.spawn(async move |this, cx| {
            match rx.recv().await {
                Ok(Ok(_resp)) => {
                    crate::notify::toast("邮件已发送 ✓");
                    this.update(cx, |view, cx| {
                        view.status = "已发送 ✓".into();
                        view.close_window(cx);
                        cx.notify();
                    })
                    .ok();
                }
                Ok(Err(e)) => {
                    this.update(cx, |view, cx| {
                        view.sending = false;
                        view.status = format!("{e}").into();
                        cx.notify();
                    })
                    .ok();
                }
                Err(_) => {}
            }
        })
        .detach();
    }
}

// 附件选择的线程桥（线程 → 队列消费由全局任务处理，spike 简化：直接全局 spawn）
fn cx_spawn_attachments(rx: smol::channel::Receiver<Option<Vec<std::path::PathBuf>>>) {
    std::thread::spawn(move || {
        if let Ok(Some(files)) = rx.recv_blocking() {
            ATTACHMENT_QUEUE.lock().unwrap().extend(files);
        }
    });
}

static ATTACHMENT_QUEUE: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

impl Render for ComposeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.close_handle.is_none() {
            self.close_handle = Some(window.window_handle());
        }
        // 拾取后台线程选到的附件
        {
            let mut q = ATTACHMENT_QUEUE.lock().unwrap();
            for path in q.drain(..) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                self.attachments.push((path, name, size));
            }
        }

        let field = |label: &str, input: &Entity<InputState>| {
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .w(px(48.))
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(label.to_string()),
                )
                .child(div().flex_1().child(Input::new(input)))
        };

        let _ = window;
        let send_label: SharedString = if self.sending {
            format!("{}...", t("email.send")).into()
        } else {
            t("email.send")
        };

        v_flex()
            .size_full()
            .p_3()
            .gap_2()
            // 工具行
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("send")
                            .primary()
                            .label(send_label)
                            .on_click(cx.listener(Self::send)),
                    )
                    .child(
                        Button::new("account")
                            .ghost()
                            .icon(gpui_kit_assets::IconName::UserRound)
                            .label(match &self.selected_account {
                                Some((_, name)) => format!("✉ {name}"),
                                None => "选择账号".to_string(),
                            })
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.cycle_account(cx);
                            })),
                    )
                    .child(
                        Button::new("save-draft")
                            .ghost()
                            .icon(gpui_kit_assets::IconName::Save)
                            .label(t("email.draftSaved"))
                            .on_click(cx.listener(Self::save_draft_now)),
                    )
                    .child(
                        Button::new("attach")
                            .ghost()
                            .icon(gpui_kit_assets::IconName::Paperclip)
                            .label(t("email.attach"))
                            .on_click(cx.listener(Self::add_attachment)),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.status.clone()),
                    ),
            )
            // 字段区
            .child(field(&t("email.to"), &self.to))
            .child(field(&t("email.cc"), &self.cc))
            .child(field(&t("email.bcc"), &self.bcc))
            .child(field(&t("email.subject"), &self.subject))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(self.draft_status.clone()),
                    )
                    .child(
                        Button::new("discard-draft")
                            .ghost()
                            .xsmall()
                            .label("丢弃草稿")
                            .on_click(cx.listener(Self::discard_draft)),
                    ),
            )
            // 附件 chips
            .when(!self.attachments.is_empty(), |this| {
                this.child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .children(
                            self.attachments
                                .iter()
                                .enumerate()
                                .map(|(i, (_path, name, size))| {
                                    h_flex()
                                        .gap_1()
                                        .px_2()
                                        .py_1()
                                        .rounded(cx.theme().radius)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .text_size(px(12.))
                                        .child(format!("{name} ({size})"))
                                        .child(
                                            Button::new(format!("rm-att-{i}"))
                                                .ghost()
                                                .xsmall()
                                                .label("×")
                                                .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                                                    this.attachments.remove(i);
                                                    cx.notify();
                                                })),
                                        )
                                }),
                        ),
                )
            })
            // 编辑器
            .child(
                div()
                    .flex_1()
                    .min_h(px(220.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .overflow_hidden()
                    .child(EmailEditor::new(&self.editor)),
            )
    }
}


fn split_addresses(raw: &str) -> Vec<String> {
    raw.split([',', ';', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}
