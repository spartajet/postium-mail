//! 邮件详情视图（移植自 src/lib/components/email/EmailDetail.svelte）。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    text::html,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::ContactWrite;

use crate::i18n::{t, t_format};
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::detail::DetailStore;

pub struct EmailDetailView {
    pub store: gpui_kit::Entity<DetailStore>,
    pub api: Arc<CoreApi>,
    /// 邮件列表弱引用：归档/删除/标未读后刷新列表（已删邮件不再留在列表）
    pub emails: gpui_kit::WeakEntity<crate::stores::emails::EmailsStore>,
    pub recipients_expanded: bool,
}

impl EmailDetailView {
    fn toggle_star(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let id = detail.email.id;
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.toggle_star(id).await });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(starred)) => {
                    store.update(cx, |s, cx| {
                        s.apply_star(starred);
                        cx.notify();
                    });
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "切换星标失败: {e}"),
                Err(_) => {}
            }
        })
        .detach();
    }

    fn archive(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.archive_email(detail.email.id).await });
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(())) => {
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "归档邮件失败: {e}"),
                Err(_) => {}
            }
        })
        .detach();
    }

    fn delete(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokok_delete(api, detail.email.id);
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(_)) => {
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "删除邮件失败: {e}"),
                Err(_) => {}
            }
        })
        .detach();
    }

    fn reply_all(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        // 收件人 = 发件人 + 原收件人（去重）
        let mut to = vec![detail.email.sender_email.clone()];
        for r in detail.recipient_emails.split([',', ';']) {
            let r = r.trim();
            if !r.is_empty() && !to.iter().any(|x| x.eq_ignore_ascii_case(r)) {
                to.push(r.to_string());
            }
        }
        let to = to.join(", ");
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::ReplyAll { email_id: detail.email.id, account_id: detail.email.account_id, to, subject, original_html },
                &api,
                cx,
            );
        });
    }

    fn mark_unread(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let id = detail.email.id;
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.mark_as_read(id, false).await });
        let emails = self.emails.clone();
        let api_keep = self.api.clone();
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(())) => {
                    store.update(cx, |s, cx| {
                        s.detail = None;
                        s.html = None;
                        cx.notify();
                    });
                    // 详情清空后主窗口标题恢复默认（此路径不发 Loaded 事件）
                    restore_window_title(cx);
                    if let Some(list) = emails.upgrade() {
                        list.update(cx, |s, cx| {
                            s.refresh_now(&api_keep, cx);
                        });
                    }
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "标记未读失败: {e}"),
                Err(_) => {}
            }
        })
        .detach();
    }

    fn reload(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let id = detail.email.id;
        let api = self.api.clone();
        let api_keep = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.reload_email(id).await });
        cx.spawn(async move |_, cx| {
            match rx.recv().await {
                Ok(Ok(_)) => {
                    store.update(cx, |s, cx| {
                        s.force_load(id, &api_keep, cx);
                    });
                }
                Ok(Err(e)) => tracing::warn!(target: "postium::ui", "从服务器重载邮件失败: {e}"),
                Err(_) => {}
            }
        })
        .detach();
    }

    fn reply(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let to = detail.email.sender_email.clone();
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::Reply { email_id: detail.email.id, account_id: detail.email.account_id, to, subject, original_html },
                &api,
                cx,
            );
        });
    }

    fn forward(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let subject = detail.email.subject.clone().unwrap_or_default();
        let original_html = detail.body_html.clone().unwrap_or_default();
        let api = self.api.clone();
        cx.defer(move |cx| {
            crate::views::compose::open_compose_window(
                crate::views::compose::ComposeMode::Forward { email_id: detail.email.id, account_id: detail.email.account_id, subject, original_html },
                &api,
                cx,
            );
        });
    }

    fn open_attachment(&mut self, att_id: i32, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let _ = spawn_tokio(async move { api.open_attachment(att_id).await });
        let _ = cx;
    }

    fn save_attachment_as(&mut self, att_id: i32, filename: String, _: &ClickEvent, _: &mut Window, _: &mut Context<Self>) {
        // 文件对话框在后台线程（rfd async）
        let api = self.api.clone();
        std::thread::spawn(move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name(&filename)
                .save_file()
            {
                let target = path.to_string_lossy().to_string();
                let _ = crate::runtime::runtime()
                    .spawn(async move { api.save_attachment_as(att_id, target).await });
            }
        });
    }

    /// 浏览器精确查看（table 布局保真兜底）。
    fn open_in_browser(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        if let Some(body) = detail.body_html.clone() {
            // 写临时 HTML 并用系统默认浏览器打开
            let dir = std::env::temp_dir().join("postium-view");
            let _ = std::fs::create_dir_all(&dir);
            let file = dir.join(format!("email-{}.html", detail.email.id));
            let page = format!(
                "<!doctype html><meta charset=\"utf-8\"><body style=\"font-family:sans-serif\">{body}</body>"
            );
            if std::fs::write(&file, page).is_ok() {
                let _ = open::that(&file);
            }
        }
    }

    /// 操作行「联系人」联动：按发件人邮箱查联系人——已存在则经 MainShell
    /// 全局注册表切主窗口到联系人页并按邮箱选中（视图未建时由壳渲染期
    /// 消费待选邮箱）；不存在则一步创建 + toast「已添加」。
    pub(crate) fn add_contact(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let sender = detail.email.sender_email.trim().to_string();
        if sender.is_empty() {
            return;
        }
        let sender_name = detail
            .email
            .sender_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string);
        let api = self.api.clone();
        let sender_link = sender.clone();
        let rx = spawn_tokio(async move {
            // 查 → 无则建：一条异步链完成判定，UI 侧只消费结果
            match api.get_contact_by_email(sender.clone()).await {
                Ok(Some(_)) => Ok(false),
                Ok(None) => api
                    .create_contact(ContactWrite {
                        email: sender,
                        extra_emails: None,
                        name: sender_name,
                        phones: None,
                        notes: None,
                        org: None,
                    })
                    .await
                    .map(|_| true),
                Err(e) => Err(e),
            }
        });
        cx.spawn(async move |_, cx| {
            let Ok(Ok(created)) = rx.recv().await else { return };
            if created {
                crate::notify::toast(t("contacts.added"));
            } else {
                // 已存在：经全局弱注册表切联系人页，待选邮箱由壳渲染期消费
                cx.update(|cx| {
                    let Some(shell) = cx
                        .try_global::<crate::app::MainShell>()
                        .and_then(|g| g.0.upgrade())
                    else {
                        return;
                    };
                    shell.update(cx, |shell, cx| {
                        shell.nav = NavTarget::Contacts;
                        shell.contacts_pending_email = Some(sender_link.clone());
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }
}

fn spawn_tokok_delete(api: Arc<CoreApi>, id: i32) -> smol::channel::Receiver<Result<usize, postium_core::error::MailError>> {
    spawn_tokio(async move { api.delete_emails(vec![id]).await })
}

/// 详情面板被操作清空（归档/删除/标未读）后恢复主窗口默认标题。
/// 这些路径不经过 DetailStore::load，不会发 DetailEvent::Loaded。
fn restore_window_title(cx: &mut gpui_kit::AsyncApp) {
    let _ = cx.update(|app| {
        if let Some(main) = app.try_global::<crate::app::MainShell>()
            && let Some(shell) = main.0.upgrade()
        {
            let _ = shell.update(app, |shell, cx| shell.update_window_title(cx));
        }
    });
}

/// 邮件正文 TextView 样式。
///
/// `html()` 组件内部把 HTML 降级为富文本结构（内联 CSS 不生效），
/// 这里把观感对齐常见邮件客户端：段落间距收紧、标题按层级放大加粗。
/// （组件层不暴露链接颜色定制，链接色跟随应用主题主色。）
fn email_text_view_style(cx: &gpui_kit::App) -> gpui_kit::component::text::TextViewStyle {
    use std::sync::Arc;

    let base = gpui_kit::component::text::TextViewStyle::default();
    gpui_kit::component::text::TextViewStyle {
        // 默认 0.75rem 段距在邮件里显得松散（模板还叠空段，已由
        // collapse_empty_blocks 折叠）
        paragraph_gap: gpui_kit::rems(0.35),
        heading_base_font_size: px(14.),
        heading_font_size: Some(Arc::new(|level, base_size| match level {
            1 => px(20.),
            2 => px(17.),
            3 => px(15.5),
            _ => base_size,
        })),
        is_dark: cx.theme().is_dark(),
        ..base
    }
}

/// 折叠邮件 HTML 里的空段落 / 空 div / mso 空标签。
///
/// Outlook、163 等客户端的邮件模板用 `<p>&nbsp;</p>`、`<o:p></o:p>`
/// 之类的空块撑视觉间距，逐段渲染时叠加段落 gap 产生巨大空隙。
fn collapse_empty_blocks(source: &str) -> String {
    static EMPTY_BLOCKS: std::sync::OnceLock<Vec<regex::Regex>> = std::sync::OnceLock::new();
    // 忽略型内容：空白、&nbsp;、<br>、HTML 注释；span 只包忽略型内容时也算空
    let ignore = r"(?:\s|&nbsp;|&#160;|<br\s*/?>|<!--.*?-->)";
    let patterns = [
        // 空 p：可含忽略型内容或纯空的 span
        format!(r"(?is)<p(\s[^>]*)?>(?:{ignore}|<span(\s[^>]*)?>(?:{ignore})*</span>)*</p>"),
        // 空 div（空 p 已在前一轮删掉后，此处内容只剩忽略型）
        format!(r"(?is)<div(\s[^>]*)?>(?:{ignore})*</div>"),
        // mso 空段
        format!(r"(?is)<o:p(\s[^>]*)?>(?:{ignore})*</o:p>"),
    ];
    let res = EMPTY_BLOCKS.get_or_init(|| {
        patterns
            .iter()
            .map(|p| regex::Regex::new(p).expect("空块正则"))
            .collect()
    });

    let mut out = source.to_string();
    // 嵌套空块（div 套空 p）需要迭代收敛，上限防止病态输入死循环
    for _ in 0..4 {
        let before = out.len();
        for re in res.iter() {
            out = re.replace_all(&out, "").into_owned();
        }
        if out.len() == before {
            break;
        }
    }
    out
}

impl Render for EmailDetailView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);

        let Some(detail) = &store.detail else {
            return v_flex()
                .flex_1()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(cx.theme().muted_foreground)
                        .child(t("email.noEmailSelected")),
                )
                .into_any_element();
        };

        let sent_time = chrono::DateTime::from_timestamp(detail.email.sent_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let sender = detail
            .email
            .sender_name
            .clone()
            .unwrap_or_else(|| detail.email.sender_email.clone());

        let body_html_str = store.html.clone().unwrap_or_default();
        let body: AnyElement = if !body_html_str.is_empty() {
            div()
                .flex_1()
                .overflow_hidden()
                .child(
                    html(collapse_empty_blocks(&body_html_str))
                        .style(email_text_view_style(cx))
                        .p_4()
                        .scrollable(true)
                        .selectable(true),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .overflow_hidden()
                .p_4()
                .child(
                    div()
                        .text_size(px(13.5))
                        .line_height(relative(1.8))
                        .child(detail.body_text.clone().unwrap_or_default()),
                )
                .into_any_element()
        };

        v_flex()
            .flex_1()
            .size_full()
            .min_w(px(380.))
            .border_l_1()
            .border_color(cx.theme().border)
            // 头部：主题 + 操作
            .child(
                v_flex()
                    .p_3()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .flex_none()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .flex_1()
                                    .child(
                                        detail
                                            .email
                                            .subject
                                            .clone()
                                            .unwrap_or_else(|| "（无主题）".into()),
                                    ),
                            )
                            // 操作分组：主操作（回复/全部回复/转发）｜邮件态（星标/已读/重载）｜归置（归档/删除）｜浏览器
                            .child(
                                Button::new("reply")
                                    .primary()
                                    .xsmall()
                                    .label(t("email.reply"))
                                    .on_click(cx.listener(Self::reply)),
                            )
                            .child(
                                Button::new("reply-all")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.replyAll"))
                                    .on_click(cx.listener(Self::reply_all)),
                            )
                            .child(
                                Button::new("forward")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.forward"))
                                    .on_click(cx.listener(Self::forward)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("star")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Star)
                                    .when(detail.email.is_starred, |b| {
                                        b.label("★").tooltip(t("email.unstar"))
                                    })
                                    .when(!detail.email.is_starred, |b| {
                                        b.tooltip(t("email.star"))
                                    })
                                    .on_click(cx.listener(Self::toggle_star)),
                            )
                            .child(
                                Button::new("mark-unread")
                                    .ghost()
                                    .xsmall()
                                    .label(if detail.email.is_read { t("email.markUnread") } else { t("email.markRead") })
                                    .on_click(cx.listener(Self::mark_unread)),
                            )
                            .child(
                                Button::new("reload")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit_assets::IconName::RotateCw)
                                    .tooltip(t("email.reload"))
                                    .on_click(cx.listener(Self::reload)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("archive")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.archive"))
                                    .on_click(cx.listener(Self::archive)),
                            )
                            .child(
                                Button::new("delete")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.delete"))
                                    .on_click(cx.listener(Self::delete)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("browser")
                                    .ghost()
                                    .xsmall()
                                    .label("浏览器打开")
                                    .on_click(cx.listener(Self::open_in_browser)),
                            )
                            .child(separator(cx))
                            .child(
                                Button::new("contact-link")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Contact)
                                    .label(t("contacts.link"))
                                    .on_click(cx.listener(Self::add_contact)),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .text_size(px(12.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(sender)
                            .child("·")
                            .child(sent_time)
                            .child("·")
                            .child(t("email.to"))
                            .child(recipients_display(detail, self.recipients_expanded, cx))
                            .when_some(cc_display(detail), |this, cc| {
                                this.child("·").child(t("email.cc")).child(cc)
                            })
                            .when(
                                recipient_count(detail) > 2,
                                |this| {
                                    this.child(
                                        Button::new("expand-recipients")
                                            .ghost()
                                            .xsmall()
                                            .label(if self.recipients_expanded {
                                                t("email.collapseRecipients").to_string()
                                            } else {
                                                t_format("email.otherRecipients", &[("count", &(recipient_count(detail) - 2).to_string())]).to_string()
                                            })
                                            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                                                this.recipients_expanded = !this.recipients_expanded;
                                                cx.notify();
                                            })),
                                    )
                                },
                            ),
                    ),
            )
            // 附件条
            .when(!detail.attachments.is_empty(), |this| {
                this.child(
                    h_flex()
                        .flex_none()
                        .flex_wrap()
                        .gap_2()
                        .p_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .children(detail.attachments.iter().map(|att| {
                            let att_id = att.id;
                            let filename = att.filename.clone();
                            let api = self.api.clone();
                            let _ = api;
                            h_flex()
                                .gap_1()
                                .px_2()
                                .py_1()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .child(gpui_kit::component::Icon::new(IconName::Paperclip).size_3())
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .child(format!("{}", att.filename)),
                                )
                                .child(
                                    Button::new(format!("open-att-{att_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label(t("email.attachmentOpen"))
                                        .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                            this.open_attachment(att_id, e, w, cx)
                                        })),
                                )
                                .child(
                                    Button::new(format!("save-att-{att_id}"))
                                        .ghost()
                                        .xsmall()
                                        .label(t("email.attachmentSaveAs"))
                                        .on_click(
                                            cx.listener(
                                                move |this: &mut Self, e, w, cx| {
                                                    this.save_attachment_as(att_id, filename.clone(), e, w, cx)
                                                },
                                            ),
                                        ),
                                )
                        })),
                )
            })
            // 正文
            .child(body)
            .into_any_element()
    }
}


fn recipient_count(detail: &postium_core::service::email_service::EmailDetail) -> usize {
    detail.recipient_emails.split([',', ';']).filter(|s| !s.trim().is_empty()).count()
}

fn recipients_display(
    detail: &postium_core::service::email_service::EmailDetail,
    expanded: bool,
    cx: &gpui_kit::App,
) -> SharedString {
    let list: Vec<&str> = detail
        .recipient_emails
        .split([',', ';'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let _ = cx;
    if expanded || list.len() <= 2 {
        list.join(", ").into()
    } else {
        format!("{}, {}", list[0], list[1]).into()
    }
}

fn cc_display(detail: &postium_core::service::email_service::EmailDetail) -> Option<SharedString> {
    detail
        .cc_emails
        .as_ref()
        .map(|cc| cc.trim())
        .filter(|cc| !cc.is_empty())
        .map(|cc| cc.into())
}


/// 工具条分组竖分隔线。
fn separator(cx: &gpui_kit::App) -> impl IntoElement {
    let _ = cx;
    div().w(px(1.)).h(px(14.)).mx_1().bg(gpui_kit::black().opacity(0.15))
}

#[cfg(test)]
mod tests {
    use super::collapse_empty_blocks;

    #[test]
    fn collapses_empty_paragraphs_and_divs() {
        let src = r#"<p style="margin:0">正文一</p><p style="margin:0">&nbsp;</p><p>正文二</p><div></div><div><br></div>"#;
        let out = collapse_empty_blocks(src);
        assert!(out.contains("正文一"));
        assert!(out.contains("正文二"));
        assert!(!out.contains("&nbsp;"));
        assert!(!out.contains("<div>"), "空 div 应被折叠: {out}");
    }

    #[test]
    fn collapses_nested_empty_blocks_and_mso_tags() {
        let src = r#"<div><p><span>&nbsp;</span></p></div><o:p></o:p><p>内容</p>"#;
        let out = collapse_empty_blocks(src);
        assert!(!out.contains("<div>"));
        assert!(!out.contains("o:p"));
        assert!(out.contains("内容"));
    }

    #[test]
    fn keeps_non_empty_blocks_untouched() {
        let src = r#"<p style="color:#0067b8">新应用有权访问你的数据</p><p>onedriver 已连接。</p>"#;
        let out = collapse_empty_blocks(src);
        assert_eq!(out, src, "无空块时原样返回");
    }

    #[test]
    fn keeps_comments_and_real_content_inside_blocks() {
        let src = r#"<p><!--[if mso]><o:p>&nbsp;</o:p><![endif]--><br>真实文本</p>"#;
        let out = collapse_empty_blocks(src);
        assert!(out.contains("真实文本"), "含真实文本的块不能删: {out}");
    }
}
