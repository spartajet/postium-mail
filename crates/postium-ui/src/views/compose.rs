//! 写信窗口（独立常驻窗口——spike 验证模式）：postium-editor 集成。
//! 移植自 src/lib/components/email/ComposeModal.svelte 的字段与发送状态机。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use postium_core::api::CoreApi;
use postium_core::service::email_service::{
    ComposeAttachmentInput,
    SendEmailRequest,
};
use postium_editor::{EmailEditor, EmailEditorState};

use crate::i18n::t;
use crate::runtime::spawn_tokio;

/// 写信模式。
#[derive(Clone, Debug)]
pub enum ComposeMode {
    New,
    Reply {
        #[allow(dead_code)] // 保留：草稿关联原邮件用
        email_id: i32,
        account_id: i32,
        to: String,
        subject: String,
        original_html: String,
    },
    ReplyAll {
        #[allow(dead_code)]
        email_id: i32,
        account_id: i32,
        to: String,
        subject: String,
        original_html: String,
    },
    Forward {
        #[allow(dead_code)]
        email_id: i32,
        account_id: i32,
        subject: String,
        original_html: String,
    },
}

/// 去除主题的 Re:/Fwd: 前缀（对齐旧版 /^(Re|Fwd):\s*/i）。
fn strip_prefixes(subject: &str) -> String {
    let mut s = subject.trim().to_string();
    loop {
        let lower = s.to_ascii_lowercase();
        if let Some(_rest) = lower.strip_prefix("re:") {
            s = s[3..].trim_start().to_string();
        } else if let Some(_rest) = lower.strip_prefix("fwd:") {
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
    pub(crate) api: Arc<CoreApi>,
    /// 所属窗口句柄（Enter 提交 chips 需要 &mut Window）
    window_ref: gpui_kit::AnyWindowHandle,
    pub(crate) to_field: crate::views::recipient_field::RecipientField,
    pub(crate) cc_field: crate::views::recipient_field::RecipientField,
    pub(crate) bcc_field: crate::views::recipient_field::RecipientField,
    pub(crate) subject: Entity<InputState>,
    pub(crate) editor: Entity<EmailEditorState>,
    attachments: Vec<(std::path::PathBuf, String, u64)>, // (路径, 文件名, 大小)
    pub(crate) status: SharedString,
    sending: bool,
    pub(crate) draft_email_id: Option<i32>,
    pub(crate) draft_status: SharedString,
    /// 抄送/密送渐进显示（对齐旧版 showCcField/showBccField，默认收起）
    cc_visible: bool,
    bcc_visible: bool,
    /// 发件账号 ID（点击工具条账号按钮循环切换）
    pub(crate) selected_account: Option<(i32, String)>,
    /// 收件人自动补全防抖代数（三字段共用；每次输入变更 +1，超时后比对丢弃过期查询）
    sugg_gen: u64,
    close_handle: Option<gpui_kit::AnyWindowHandle>,
}

impl ComposeView {
    pub(crate) fn new(mode: ComposeMode, api: Arc<CoreApi>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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

        let mk_field = |ph: &str, initial: &str, window: &mut Window, cx: &mut Context<Self>| {
            crate::views::recipient_field::RecipientField::new(ph, initial, window, cx)
        };
        let subject = cx.new(|cx| {
            if subject_init.is_empty() {
                InputState::new(window, cx)
            } else {
                InputState::new(window, cx).default_value(subject_init.clone())
            }
        });

        let window_ref = window.window_handle();
        let this = Self {
            api: api.clone(),
            window_ref,
            to_field: mk_field("you@example.com", &to_init, window, cx),
            cc_field: mk_field("you@example.com", "", window, cx),
            bcc_field: mk_field("you@example.com", "", window, cx),
            subject,
            editor,
            attachments: Vec::new(),
            status: "就绪".into(),
            sending: false,
            draft_email_id: None,
            draft_status: " ".into(),
            cc_visible: false,
            bcc_visible: false,
            selected_account: None,
            sugg_gen: 0,
            close_handle: None,
        };

        // 收件人字段：回车提交 chips（浮层开时先取第一项候选）；
        // 输入变更 300ms 防抖查联系人回填候选（对齐顶栏/联系人页搜索的代数模式）
        for which in [0usize, 1, 2] {
            let input = match which {
                0 => this.to_field.input.clone(),
                1 => this.cc_field.input.clone(),
                _ => this.bcc_field.input.clone(),
            };
            cx.subscribe(&input, move |this: &mut Self, _, event: &InputEvent, cx| {
                match event {
                    InputEvent::PressEnter { .. } => {
                        // 提交后输入清空：作废在途防抖查询，防止迟到结果重开浮层
                        this.sugg_gen += 1;
                        let handle = this.window_ref;
                        // 空输入 Enter（防抖窗口内候选尚挂旧值）：不取候选直接收浮层，
                        // 避免把陈旧候选提交成 chip（T12 遗留边缘）
                        let input_empty = match which {
                            0 => this.to_field.input.read(cx).value().trim().is_empty(),
                            1 => this.cc_field.input.read(cx).value().trim().is_empty(),
                            _ => this.bcc_field.input.read(cx).value().trim().is_empty(),
                        };
                        if input_empty {
                            match which {
                                0 => this.to_field.close_suggestions(),
                                1 => this.cc_field.close_suggestions(),
                                _ => this.bcc_field.close_suggestions(),
                            }
                        }
                        let pick_first = if input_empty {
                            String::new()
                        } else {
                            match which {
                                // 浮层开且有候选 → Enter 取第一项（set_value 后 commit）
                                0 if this.to_field.sugg_open
                                    && !this.to_field.suggestions.is_empty() =>
                                {
                                    this.to_field.suggestions[0].email.clone()
                                }
                                1 if this.cc_field.sugg_open
                                    && !this.cc_field.suggestions.is_empty() =>
                                {
                                    this.cc_field.suggestions[0].email.clone()
                                }
                                _ if which == 2
                                    && this.bcc_field.sugg_open
                                    && !this.bcc_field.suggestions.is_empty() =>
                                {
                                    this.bcc_field.suggestions[0].email.clone()
                                }
                                _ => String::new(),
                            }
                        };
                        let _ = handle.update(cx, |_, window, cx| {
                            if !pick_first.is_empty() {
                                match which {
                                    0 => this.to_field.input.update(cx, |s, cx| {
                                        s.set_value(pick_first.clone(), window, cx)
                                    }),
                                    1 => this.cc_field.input.update(cx, |s, cx| {
                                        s.set_value(pick_first.clone(), window, cx)
                                    }),
                                    _ => this.bcc_field.input.update(cx, |s, cx| {
                                        s.set_value(pick_first.clone(), window, cx)
                                    }),
                                };
                            }
                            match which {
                                0 => this.to_field.commit(window, cx),
                                1 => this.cc_field.commit(window, cx),
                                _ => this.bcc_field.commit(window, cx),
                            }
                        });
                    }
                    InputEvent::Change => {
                        this.sugg_gen += 1;
                        let debounce_gen = this.sugg_gen;
                        let api = this.api.clone();
                        cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(300))
                                .await;
                            let q = this.update(cx, |view, cx| {
                                // 后续输入已接管 → 丢弃过期查询
                                if view.sugg_gen != debounce_gen {
                                    return None;
                                }
                                let (value, field_is_open) = match which {
                                    0 => (
                                        view.to_field.input.read(cx).value().to_string(),
                                        view.to_field.sugg_open,
                                    ),
                                    1 => (
                                        view.cc_field.input.read(cx).value().to_string(),
                                        view.cc_field.sugg_open,
                                    ),
                                    _ => (
                                        view.bcc_field.input.read(cx).value().to_string(),
                                        view.bcc_field.sugg_open,
                                    ),
                                };
                                if value.trim().is_empty() {
                                    // 输入清空 → 关浮层清候选
                                    match which {
                                        0 => view.to_field.close_suggestions(),
                                        1 => view.cc_field.close_suggestions(),
                                        _ => view.bcc_field.close_suggestions(),
                                    }
                                    if field_is_open {
                                        cx.notify();
                                    }
                                    return None;
                                }
                                Some(value)
                            })
                            .ok()
                            .flatten();
                            let Some(q) = q else { return };
                            let rx = spawn_tokio(async move {
                                api.list_contacts(Some(q), None, false, 0, 8)
                                    .await
                                    .map(|resp| resp.contacts)
                            });
                            if let Ok(Ok(list)) = rx.recv().await {
                                this.update(cx, |view, cx| {
                                    if view.sugg_gen != debounce_gen {
                                        return; // 查询期间输入又变了，丢弃
                                    }
                                    match which {
                                        0 => view.to_field.set_suggestions(list),
                                        1 => view.cc_field.set_suggestions(list),
                                        _ => view.bcc_field.set_suggestions(list),
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        })
                        .detach();
                    }
                    _ => {}
                }
            })
            .detach();
        }

        // 装载账号列表：回复/转发优先原邮件所属账号，否则第一个
        let preferred_account = match &mode {
            ComposeMode::Reply { account_id, .. }
            | ComposeMode::ReplyAll { account_id, .. }
            | ComposeMode::Forward { account_id, .. } => Some(*account_id),
            ComposeMode::New => None,
        };
        let rx = spawn_tokio(async move {
            match api.list_accounts().await {
                Ok(accs) => accs,
                Err(e) => {
                    tracing::warn!(target: "postium::ui", "写信窗口加载账号列表失败: {e}");
                    Vec::new()
                }
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(accounts) = rx.recv().await {
                let _ = this.update(cx, |view, cx| {
                    let chosen = preferred_account
                        .and_then(|id| accounts.iter().find(|a| a.id == id))
                        .or_else(|| accounts.first());
                    if let Some(acc) = chosen {
                        view.selected_account = Some((acc.id, acc.name.clone()));
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        this
    }

    fn add_attachment(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        // rfd 同步对话框放后台线程；结果经实体回调回本窗口（替代全局队列，
        // 多写信窗口不再互相串件），大小在选择时读取一次（不在 render 中做 IO）
        let (tx, rx) = smol::channel::bounded(1);
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new().pick_files();
            let _ = tx.try_send(picked);
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Some(files)) = rx.recv().await {
                this.update(cx, |view, cx| {
                    for path in files {
                        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        view.attachments.push((path, name, size));
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn value_of(input: &Entity<InputState>, cx: &App) -> String {
        input.read(cx).value().to_string()
    }

    /// chips 删除路由：which 0/1/2 → to/cc/bcc 字段。
    fn make_chip_remover(
        which: usize,
        cx: &mut Context<Self>,
    ) -> std::rc::Rc<dyn Fn(String, &ClickEvent, &mut Window, &mut App)> {
        let weak = cx.entity().downgrade();
        std::rc::Rc::new(move |email: String, _, _, cx: &mut App| {
            let _ = weak.update(cx, |view: &mut Self, cx| {
                match which {
                    0 => view.to_field.remove_by_email(&email),
                    1 => view.cc_field.remove_by_email(&email),
                    _ => view.bcc_field.remove_by_email(&email),
                }
                cx.notify();
            });
        })
    }

    /// 候选浮层点击路由：which 0/1/2 → 对应字段 set_value(email) + commit。
    /// 点击回调只有 &mut App，经窗口句柄 update 拿 &mut Window（同 PressEnter 模式）。
    fn make_suggestion_picker(
        which: usize,
        handle: gpui_kit::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> std::rc::Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)> {
        let weak = cx.entity().downgrade();
        std::rc::Rc::new(move |idx: usize, _, _, cx: &mut App| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = weak.update(cx, |view: &mut Self, cx| {
                    let email = match which {
                        0 => view.to_field.suggestions.get(idx).map(|c| c.email.clone()),
                        1 => view.cc_field.suggestions.get(idx).map(|c| c.email.clone()),
                        _ => view.bcc_field.suggestions.get(idx).map(|c| c.email.clone()),
                    };
                    let Some(email) = email else { return };
                    // 作废在途防抖查询（与 PressEnter 提交一致）
                    view.sugg_gen += 1;
                    match which {
                        0 => view.to_field.input.update(cx, |s, cx| {
                            s.set_value(email, window, cx)
                        }),
                        1 => view.cc_field.input.update(cx, |s, cx| {
                            s.set_value(email, window, cx)
                        }),
                        _ => view.bcc_field.input.update(cx, |s, cx| {
                            s.set_value(email, window, cx)
                        }),
                    }
                    match which {
                        0 => view.to_field.commit(window, cx),
                        1 => view.cc_field.commit(window, cx),
                        _ => view.bcc_field.commit(window, cx),
                    }
                    cx.notify();
                });
            });
        })
    }

    fn collect_recipients(&self, cx: &App) -> Vec<String> {
        self.to_field.emails(cx)
    }

    /// 循环切换发件账号。
    fn cycle_account(&mut self, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let current = self.selected_account.clone();
        let rx = spawn_tokio(async move {
            match api.list_accounts().await {
                Ok(accs) => accs,
                Err(e) => {
                    tracing::warn!(target: "postium::ui", "切换发件账号失败（加载账号列表）: {e}");
                    Vec::new()
                }
            }
        });
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
    pub(crate) fn discard_draft(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn close_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.close_handle {
            let alive = cx.windows().iter().any(|w| w.window_id() == handle.window_id());
            if alive {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        }
    }

    pub(crate) fn persist_draft(&mut self, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let api = self.api.clone();
        let (account_id, _) = match self.selected_account.clone() {
            Some(a) => a,
            None => return,
        };
        let to = self.to_field.text(cx);
        let cc = self.cc_field.text(cx);
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

    pub(crate) fn send(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.sending {
            return;
        }
        let to = self.collect_recipients(cx);
        if to.is_empty() {
            let msg = t("email.sendRequiresRecipient");
            tracing::info!(target: "postium::ui", "发送校验未通过: {msg}");
            self.status = msg;
            cx.notify();
            return;
        }
        let subject = Self::value_of(&self.subject, cx);
        if subject.trim().is_empty() {
            let msg = t("email.sendRequiresSubject");
            tracing::info!(target: "postium::ui", "发送校验未通过: {msg}");
            self.status = msg;
            cx.notify();
            return;
        }
        let body_html = self.editor.read(cx).html(cx);
        if body_html.trim().is_empty() {
            let msg: SharedString = t("email.sendRequiresBody").to_string().into();
            tracing::info!(target: "postium::ui", "发送校验未通过: {msg}");
            self.status = msg;
            cx.notify();
            return;
        }

        // 发件账号（未选择时回落第一个）
        let api = self.api.clone();
        let draft_id = self.draft_email_id;
        let cc: Vec<String> = self.cc_field.emails(cx);
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
        tracing::info!(target: "postium::ui", subject = %subject, "开始发送邮件");
        cx.notify();

        let preferred_account = self.selected_account.clone();
        let to_raw = to.join(", ");
        let api_parse = api.clone();
        let rx = spawn_tokio(async move {
            let account_id = match preferred_account {
                Some((id, _)) => id,
                None => {
                    let accounts = api.list_accounts().await.unwrap_or_else(|e| {
                        tracing::warn!(target: "postium::ui", "发送前加载账号列表失败: {e}");
                        Vec::new()
                    });
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
                    tracing::warn!(target: "postium::ui", "邮件发送失败: {e}");
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

impl Render for ComposeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        if self.close_handle.is_none() {
            self.close_handle = Some(window.window_handle());
        }
        let muted_fg = cx.theme().muted_foreground;
        let field = move |label: &str, input: &Entity<InputState>| {
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .w(px(48.))
                        .flex_none()
                        .text_size(px(12.))
                        .text_color(muted_fg)
                        .child(label.to_string()),
                )
                .child(div().flex_1().child(Input::new(input)))
        };

        let _ = window;
        // 浮层候选点击需要经窗口句柄取 &mut Window（set_value + commit）
        let window_handle = window.window_handle();
        let pick = |which: usize, cx: &mut Context<Self>| {
            Self::make_suggestion_picker(which, window_handle, cx)
        };
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
                    // 发送失败等长错误文本：占据剩余宽度并自动换行，不溢出窗口
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .line_clamp(3)
                            .child(self.status.clone()),
                    ),
            )
            // 字段区
            .child(h_flex().gap_2().items_center()
                .child(div().w(px(48.)).flex_none().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(t("email.to").to_string()))
                .child(div().flex_1().child(self.to_field.render(Self::make_chip_remover(0, cx), pick(0, cx), &theme)))
                // 收起的抄送/密送以小按钮渐进展开（对齐旧版）
                .when(!self.cc_visible, |this| {
                    this.child(
                        Button::new("show-cc")
                            .ghost()
                            .xsmall()
                            .label(t("email.showCc").to_string())
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.cc_visible = true;
                                cx.notify();
                            })),
                    )
                })
                .when(!self.bcc_visible, |this| {
                    this.child(
                        Button::new("show-bcc")
                            .ghost()
                            .xsmall()
                            .label(t("email.showBcc").to_string())
                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                this.bcc_visible = true;
                                cx.notify();
                            })),
                    )
                }))
            .when(self.cc_visible, |this| this.child(
                h_flex().gap_2().items_center()
                    .child(div().w(px(48.)).flex_none().text_size(px(12.)).text_color(muted_fg).child(t("email.cc").to_string()))
                    .child(div().flex_1().child(self.cc_field.render(Self::make_chip_remover(1, cx), pick(1, cx), &theme)))
            ))
            .when(self.bcc_visible, |this| this.child(
                h_flex().gap_2().items_center()
                    .child(div().w(px(48.)).flex_none().text_size(px(12.)).text_color(muted_fg).child(t("email.bcc").to_string()))
                    .child(div().flex_1().child(self.bcc_field.render(Self::make_chip_remover(2, cx), pick(2, cx), &theme)))
            ))
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
