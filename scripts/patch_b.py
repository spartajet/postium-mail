# Batch B: 详情完善 + compose 引用/前缀去重/账号选择
import re

# ---------- compose.rs ----------
p = 'crates/postium-ui/src/views/compose.rs'
s = open(p, encoding='utf-8').read()

# 1. Reply 模式注入引用（旧版：body_text 包 <p>）；转发保持引用块
s = s.replace('''        // 初始内容：回复/转发引入原文
        let (to_init, subject_init, editor_html) = match &mode {
            ComposeMode::New => (String::new(), String::new(), String::new()),
            ComposeMode::Reply { to, subject, .. } => {
                (to.clone(), format!("Re: {subject}"), String::new())
            }
            ComposeMode::Forward { subject, original_html, .. } => {
                (String::new(), format!("Fwd: {subject}"), original_html.clone())
            }
        };

        let editor = if editor_html.is_empty() {
            EmailEditorState::new(window, cx)
        } else {
            // 转发：原文进入编辑器（引用块形式）
            let quoted = format!(
                "<blockquote style=\\"border-left:3px solid #cbd5e1;margin:8px 0;padding:4px 12px;color:#555\\">{editor_html}</blockquote>"
            );
            EmailEditorState::from_html(&quoted, window, cx)
        };''', '''        // 初始内容：回复/转发引入原文（对齐旧版 showReply/showForward 语义）
        let (to_init, subject_init, editor_html) = match &mode {
            ComposeMode::New => (String::new(), String::new(), String::new()),
            ComposeMode::Reply { to, subject, original_html } => {
                let clean = strip_prefixes(subject);
                (to.clone(), format!("Re: {clean}"), original_html.clone())
            }
            ComposeMode::ReplyAll { to, subject, original_html } => {
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
                "<p><br></p><blockquote style=\\"border-left:3px solid #cbd5e1;margin:8px 0;padding:4px 12px;color:#555\\">{editor_html}</blockquote>"
            );
            EmailEditorState::from_html(&quoted, window, cx)
        };''')

# 2. ComposeMode 增加 ReplyAll
s = s.replace('''pub enum ComposeMode {
    New,
    Reply { email_id: i32, to: String, subject: String, original_html: String },
    Forward { email_id: i32, subject: String, original_html: String },
}''', '''pub enum ComposeMode {
    New,
    Reply { email_id: i32, to: String, subject: String, original_html: String },
    ReplyAll { email_id: i32, to: String, subject: String, original_html: String },
    Forward { email_id: i32, subject: String, original_html: String },
}

/// 去除主题的 Re:/Fwd: 前缀（对齐旧版 /^(Re|Fwd):\\s*/i）。
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
}''')
open(p, 'w', encoding='utf-8').write(s)

# ---------- email_detail.rs ----------
p2 = 'crates/postium-ui/src/views/email_detail.rs'
s2 = open(p2, encoding='utf-8').read()

# 头部收件人显示改为可展开 + 抄送；操作条加 replyAll/markUnread/reload
s2 = s2.replace('''                    .child(
                        h_flex()
                            .gap_2()
                            .text_size(px(12.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(sender)
                            .child("·")
                            .child(sent_time)
                            .child("·")
                            .child(t("email.to"))
                            .child(detail.recipient_emails.clone()),
                    ),''', '''                    .child(
                        h_flex()
                            .gap_2()
                            .text_size(px(12.5))
                            .text_color(cx.theme().muted_foreground)
                            .child(sender)
                            .child("·")
                            .child(sent_time)
                            .child("·")
                            .child(t("email.to"))
                            .child(recipients_display(detail, self.recipients_expanded, cx)),
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
                                                t("email.collapseRecipients")
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
                    ),''')

# 操作条按钮：reply 后加 replyAll，浏览器前加标未读/重载
s2 = s2.replace('''                            .child(
                                Button::new("forward")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.forward"))
                                    .on_click(cx.listener(Self::forward)),
                            )''', '''                            .child(
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
                            .child(
                                Button::new("mark-unread")
                                    .ghost()
                                    .xsmall()
                                    .label(t("email.markUnread"))
                                    .on_click(cx.listener(Self::mark_unread)),
                            )
                            .child(
                                Button::new("reload")
                                    .ghost()
                                    .xsmall()
                                    .icon(gpui_kit_assets::IconName::RotateCw)
                                    .tooltip(t("email.reload"))
                                    .on_click(cx.listener(Self::reload)),
                            )''')

# 新方法：reply_all / mark_unread / reload + recipients 展开状态字段 + 辅助函数
s2 = s2.replace('''    fn reply(&mut self''', '''    fn reply_all(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
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
                crate::views::compose::ComposeMode::ReplyAll { email_id: detail.email.id, to, subject, original_html },
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
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.detail = None;
                    s.html = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn reload(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(detail) = self.store.read(cx).detail.clone() else { return };
        let id = detail.email.id;
        let api = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move { api.reload_email(id).await });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(_)) = rx.recv().await {
                store.update(cx, |s, cx| {
                    s.load(id, &api, cx);
                });
            }
        })
        .detach();
    }

    fn reply(&mut self''', 1)

# reply 也注入引用（原实现 editor_html 空 → compose 端已改，Reply 分支现在带 original_html——detail 侧传值已带）
# struct 加字段
s2 = s2.replace('''pub struct EmailDetailView {
    pub store: gpui_kit::Entity<DetailStore>,
    pub api: Arc<CoreApi>,
}''', '''pub struct EmailDetailView {
    pub store: gpui_kit::Entity<DetailStore>,
    pub api: Arc<CoreApi>,
    recipients_expanded: bool,
}''')

# 辅助函数追加文件尾
s2 += '''

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
'''
open(p2, 'w', encoding='utf-8').write(s2)
print('batch B applied')
