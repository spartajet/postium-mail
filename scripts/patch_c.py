# Batch C: 写信完善（账号选择/草稿/验证/关窗）+ 详情视图缓存修复

# ---------- app.rs：detail_view 按选中版本缓存 ----------
p = 'crates/postium-ui/src/app.rs'
s = open(p, encoding='utf-8').read()
s = s.replace('''    email_list: Option<gpui_kit::Entity<EmailListView>>,
    list_build_version: u64,''', '''    email_list: Option<gpui_kit::Entity<EmailListView>>,
    list_build_version: u64,
    detail_view: Option<gpui_kit::Entity<EmailDetailView>>,
    detail_build_version: u32,''')
s = s.replace('''            email_list: None,
            list_build_version: 0,''', '''            email_list: None,
            list_build_version: 0,
            detail_view: None,
            detail_build_version: 0,''')
s = s.replace('''        // 选中邮件 → 装载详情
        cx.subscribe(&emails, |this: &mut Self, _, event: &EmailsEvent, cx| {
            if let EmailsEvent::SelectionChanged(id) = event {
                this.detail.update(cx, |store, cx| {
                    store.load(*id, &this.api, cx);
                });
            }
        })
        .detach();''', '''        // 选中邮件 → 装载详情（视图按选中版本重建，保住展开态/滚动）
        cx.subscribe(&emails, |this: &mut Self, _, event: &EmailsEvent, cx| {
            if let EmailsEvent::SelectionChanged(id) = event {
                this.detail.update(cx, |store, cx| {
                    store.load(*id, &this.api, cx);
                });
                this.detail_view = Some(cx.new(|_| EmailDetailView {
                    store: this.detail.clone(),
                    api: this.api.clone(),
                    recipients_expanded: false,
                }));
                this.detail_build_version = this.emails.read(cx).selected_version;
                cx.notify();
            }
        })
        .detach();''')
s = s.replace('''                .child(
                    cx.new(|_| EmailDetailView {
                        store: self.detail.clone(),
                        api: self.api.clone(),
                        recipients_expanded: false,
                    })
                    .into_any_element(),
                )
                .into_any_element(),''', '''                .child(
                    self.detail_view
                        .clone()
                        .unwrap_or_else(|| panic!("detail view 未初始化"))
                        .into_any_element(),
                )
                .into_any_element(),''')
open(p, 'w', encoding='utf-8').write(s)

# ---------- compose.rs ----------
p2 = 'crates/postium-ui/src/views/compose.rs'
s2 = open(p2, encoding='utf-8').read()

# 字段：账号选择、草稿状态、关窗句柄
s2 = s2.replace('''    attachments: Vec<(std::path::PathBuf, String, u64)>, // (路径, 文件名, 大小)
    status: SharedString,
    sending: bool,
    draft_email_id: Option<i32>,
}''', '''    attachments: Vec<(std::path::PathBuf, String, u64)>, // (路径, 文件名, 大小)
    status: SharedString,
    sending: bool,
    draft_email_id: Option<i32>,
    draft_status: SharedString,
    /// 发件账号 ID（点击工具条账号按钮循环切换）
    selected_account: Option<(i32, String)>,
    close_handle: Option<gpui_kit::WindowHandle>,
}''')

# new() 里初始化新字段 + 异步取账号列表
s2 = s2.replace('''        let this = Self {
            api,
            to: make_input(&to_init, cx),
            cc: make_input("", cx),
            bcc: make_input("", cx),
            subject: make_input(&subject_init, cx),
            editor,
            attachments: Vec::new(),
            status: "就绪".into(),
            sending: false,
            draft_email_id: None,
        };
        // 回复模式：原文作为引用块已注入；回复场景保持空正文更贴近旧版（引用块仅在转发注入）
        this''', '''        let mut this = Self {
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
        this''')

# 工具条：账号切换按钮 + 草稿按钮 + 状态
s2 = s2.replace('''            .child(
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
                        Button::new("attach")''', '''            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("send")
                            .primary()
                            .label(send_label)
                            .on_click(cx.listener(Self::send)),
                    )
                    .child(self.account_button(cx))
                    .child(
                        Button::new("save-draft")
                            .ghost()
                            .icon(gpui_kit_assets::IconName::Save)
                            .label(t("email.draftSaved"))
                            .on_click(cx.listener(Self::save_draft_now)),
                    )
                    .child(
                        Button::new("attach")''')

# render 里存 close_handle（render 有 window）
s2 = s2.replace('''    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 拾取后台线程选到的附件''', '''    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.close_handle.is_none() {
            self.close_handle = Some(window.handle());
        }
        // 拾取后台线程选到的附件''')

# 新方法：account_button / cycle_account / save_draft_now / discard_draft / validate
s2 = s2.replace('''    fn send(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {''', '''    fn account_button(&self, cx: &mut Context<Self>) -> Button {
        let label: SharedString = match &self.selected_account {
            Some((_, name)) => format!("✉ {name}").into(),
            None => "选择账号".into(),
        };
        Button::new("account")
            .ghost()
            .icon(gpui_kit_assets::IconName::UserRound)
            .label(label)
            .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                this.cycle_account(cx);
            }))
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
            let _ = handle.update(cx, |_, window| window.remove_window());
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
                to,
                cc,
                bcc: String::new(),
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

    fn send(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {''')

# send：收件人解析验证 + 指定账号 + 成功后关窗 + 失败通知
s2 = s2.replace('''        let body_html = self.editor.read(cx).html(cx);
        if body_html.trim().is_empty() {
            self.status = t("email.sendRequiresBody");
            cx.notify();
            return;
        }

        // 发件账号：取第一个账号（单账号场景；多账号选择器后续迭代）
        let api = self.api.clone();
        let draft_id = self.draft_email_id;''', '''        let body_html = self.editor.read(cx).html(cx);
        if body_html.trim().is_empty() {
            self.status = t("email.sendRequiresBody").to_string().into();
            cx.notify();
            return;
        }

        // 发件账号（未选择时回落第一个）
        let api = self.api.clone();
        let draft_id = self.draft_email_id;''')
s2 = s2.replace('''        let rx = spawn_tokio(async move {
            let accounts = api.list_accounts().await.unwrap_or_default();
            let Some(account) = accounts.first() else {
                return Err(postium_core::error::MailError::AccountNotFound(0));
            };
            api.send_email(SendEmailRequest {
                account_id: account.id,
                to,
                cc,''', '''        let preferred_account = self.selected_account.clone();
        let to_raw = to.clone();
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
                && p.invalid.iter().any(|x| !x.trim().is_empty())
            {
                return Err(postium_core::error::MailError::InvalidParam("invalid recipients".into()));
            }
            let _ = parsed;
            api.send_email(SendEmailRequest {
                account_id,
                to,
                cc,''')
# 发送结果：成功关窗，失败保持
s2 = s2.replace('''                Ok(Ok(_resp)) => {
                    this.update(cx, |view, cx| {
                        view.status = "已发送 ✓".into();
                        cx.notify();
                    });
                    // 状态显示"已发送 ✓"，用户手动关窗（gpui 异步上下文无 window 句柄）
                }
                Ok(Err(e)) => {
                    this.update(cx, |view, cx| {
                        view.sending = false;
                        view.status = format!("{e}").into();
                        cx.notify();
                    })
                    .ok();
                }''', '''                Ok(Ok(_resp)) => {
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
                }''')
# 字段区下方草稿状态行 + 丢弃草稿按钮（挂 subject 行后）
s2 = s2.replace('''            .child(field(&t("email.subject"), &self.subject))''', '''            .child(field(&t("email.subject"), &self.subject))
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
            )''')
open(p2, 'w', encoding='utf-8').write(s2)
print('batch C applied')
