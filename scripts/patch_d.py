# Batch D: 设置完善（编辑账号/改密码/手动 IMAP·SMTP/OAuth 同步范围）

p = 'crates/postium-ui/src/views/settings.rs'
s = open(p, encoding='utf-8').read()

# 1) 新字段
s = s.replace('''    add_open: bool,
    status: SharedString,
    detected_provider: Option<String>,
    oauth_state: Option<String>, // 轮询中的 state
}''', '''    add_open: bool,
    status: SharedString,
    detected_provider: Option<String>,
    oauth_state: Option<String>, // 轮询中的 state
    /// 手动 IMAP/SMTP 配置（添加账号展开）
    manual_open: bool,
    imap_host: Entity<InputState>,
    imap_port: Entity<InputState>,
    smtp_host: Entity<InputState>,
    smtp_port: Entity<InputState>,
    /// 编辑中的账号（名称/显示名）
    editing: Option<i32>,
    edit_name: Entity<InputState>,
    edit_display: Entity<InputState>,
    edit_password_for: Option<i32>,
    edit_password: Entity<InputState>,
    /// OAuth 完成后的同步范围选择
    oauth_pending_sync: Option<i32>,
}''')

# 2) new() 初始化
s = s.replace('''        let mut input = |cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
        Self {
            accounts,
            api,
            add_email: input(cx),
            add_password: input(cx),
            add_name: input(cx),
            add_open: false,
            status: " ".into(),
            detected_provider: None,
            oauth_state: None,
        }
    }''', '''        let mut input = |cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
        Self {
            accounts,
            api,
            add_email: input(cx),
            add_password: input(cx),
            add_name: input(cx),
            add_open: false,
            status: " ".into(),
            detected_provider: None,
            oauth_state: None,
            manual_open: false,
            imap_host: input(cx),
            imap_port: input(cx),
            smtp_host: input(cx),
            smtp_port: input(cx),
            editing: None,
            edit_name: input(cx),
            edit_display: input(cx),
            edit_password_for: None,
            edit_password: input(cx),
            oauth_pending_sync: None,
        }
    }''')

# 3) 手动配置字段进 CreateAccountRequest
s = s.replace('''                smtp_host: None,
                smtp_port: None,
                smtp_ssl_mode: None,
                color: None,
                account_type: Some("personal".into()),
            };''', '''                imap_host: if self.manual_open { Some(Self::value(&self.imap_host, cx)) } else { None },
                imap_port: Self::value(&self.imap_port, cx).trim().parse::<i32>().ok(),
                imap_ssl_mode: None,
                smtp_host: if self.manual_open { Some(Self::value(&self.smtp_host, cx)) } else { None },
                smtp_port: Self::value(&self.smtp_port, cx).trim().parse::<i32>().ok(),
                smtp_ssl_mode: None,
                color: None,
                account_type: Some("personal".into()),
            };''')

# 4) OAuth 完成后进入范围选择（替换直接刷新完成）
s = s.replace('''                            Ok(Some(_acct_email)) => {
                                accounts_store.update(cx, |store, cx| {
                                    store.reload(&api_reload, cx);
                                });
                                this.update(cx, |view, cx| {
                                    view.status = "账号已添加 ✓".into();
                                    view.add_open = false;
                                    cx.notify();
                                })
                                .ok();
                            }''', '''                            Ok(Some(_acct_email)) => {
                                accounts_store.update(cx, |store, cx| {
                                    store.reload(&api_reload, cx);
                                });
                                // 进入同步范围选择（对齐旧版流程）
                                let api_range = api_reload.clone();
                                let rx = crate::runtime::spawn_tokio(async move {
                                    api_range.list_accounts().await.unwrap_or_default()
                                });
                                if let Ok(accs) = rx.recv().await {
                                    let latest = accs.last().map(|a| a.id);
                                    this.update(cx, |view, cx| {
                                        view.oauth_pending_sync = latest;
                                        view.status = "授权完成，请选择同步范围".into();
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            }''')

# 5) 新方法：toggle_manual / start_edit / save_edit / start_pw / save_pw / pick_sync_range / value
s = s.replace('''    fn toggle_add(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {''', '''    fn value(input: &Entity<InputState>, cx: &gpui_kit::App) -> String {
        input.read(cx).value().to_string()
    }

    fn toggle_manual(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.manual_open = !self.manual_open;
        cx.notify();
    }

    fn start_edit(&mut self, id: i32, name: String, display: String, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some(id);
        self.edit_password_for = None;
        let _ = (name, display); // 输入框初值留空编辑（简化），名称以提交为准
        cx.notify();
    }

    fn save_edit(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.editing else { return };
        let name = Self::value(&self.edit_name, cx);
        let display = Self::value(&self.edit_display, cx);
        if name.trim().is_empty() {
            return;
        }
        let api = self.api.clone();
        let api_keep = api.clone();
        let accounts = self.accounts.clone();
        let rx = spawn_tokio(async move {
            api.update_account(UpdateAccountRequest {
                id,
                name: Some(name),
                display_name: if display.trim().is_empty() { None } else { Some(display) },
                color: None,
                sync_enabled: None,
            })
            .await
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(_)) = rx.recv().await {
                accounts.update(cx, |store, cx| {
                    store.reload(&api_keep, cx);
                });
                this.update(cx, |view, cx| {
                    view.editing = None;
                    view.status = "已保存 ✓".into();
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn start_pw(&mut self, id: i32, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.edit_password_for = Some(id);
        self.editing = None;
        cx.notify();
    }

    fn save_pw(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.edit_password_for else { return };
        let pw = Self::value(&self.edit_password, cx);
        if pw.is_empty() {
            return;
        }
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.update_account_password(id, pw).await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                this.update(cx, |view, cx| {
                    view.edit_password_for = None;
                    view.status = "密码已更新 ✓".into();
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// OAuth 后选择同步范围并开始首次同步。
    fn pick_sync_range(&mut self, range: postium_core::domain::sync::InitialSyncRange, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.oauth_pending_sync.take() else { return };
        let api = self.api.clone();
        let (tx, _rx) = smol::channel::bounded(256);
        let rx = spawn_tokio(async move { api.sync_account_with_range(tx, id, range).await });
        self.status = "首次同步已开始".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                this.update(cx, |view, cx| {
                    match result {
                        Ok(()) => view.status = "首次同步完成 ✓".into(),
                        Err(e) => view.status = format!("{e}").into(),
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_add(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {''')

# 6) 渲染：手动配置展开、范围选择区、编辑/密码表单、卡片按钮
s = s.replace('''                        .child(field(&t("account.password"), &self.add_password))''', '''                        .child(field(&t("account.password"), &self.add_password))
                        .child(
                            Button::new("manual")
                                .ghost()
                                .xsmall()
                                .label(if self.manual_open { "收起手动配置" } else { "手动配置 IMAP/SMTP" })
                                .on_click(cx.listener(Self::toggle_manual)),
                        )
                        .when(self.manual_open, |this| {
                            this.child(
                                v_flex()
                                    .gap_2()
                                    .pl_2()
                                    .border_l_1()
                                    .border_color(cx.theme().border)
                                    .child(field("IMAP", &self.imap_host))
                                    .child(field("IMAP端口", &self.imap_port))
                                    .child(field("SMTP", &self.smtp_host))
                                    .child(field("SMTP端口", &self.smtp_port)),
                            )
                        })''')

s = s.replace('''            // 账号卡片''', '''            // OAuth 后同步范围选择
            .when_some(self.oauth_pending_sync, |this, _id| {
                let range_btn = |key: &str, label: &str, range: postium_core::domain::sync::InitialSyncRange, this: &Self| {
                    let _ = this;
                    Button::new(key)
                        .ghost()
                        .xsmall()
                        .label(label)
                        .on_click(cx.listener(move |s: &mut Self, e, w, cx| {
                            s.pick_sync_range(range, e, w, cx);
                        }))
                };
                this.child(
                    v_flex()
                        .p_3()
                        .gap_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(t("account.syncScopeTitle")))
                        .child(div().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(t("account.syncScopeHint")))
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_2()
                                .child(range_btn("r-week", &t("account.syncRangeWeek"), postium_core::domain::sync::InitialSyncRange::Week, self))
                                .child(range_btn("r-month", &t("account.syncRangeMonth"), postium_core::domain::sync::InitialSyncRange::Month, self))
                                .child(range_btn("r-3m", &t("account.syncRangeThreeMonths"), postium_core::domain::sync::InitialSyncRange::ThreeMonths, self))
                                .child(range_btn("r-year", &t("account.syncRangeYear"), postium_core::domain::sync::InitialSyncRange::Year, self))
                                .child(range_btn("r-all", &t("account.syncRangeAll"), postium_core::domain::sync::InitialSyncRange::All, self)),
                        ),
                )
            })
            // 账号卡片''')

# 卡片：编辑/改密码按钮 + 展开表单
s = s.replace('''                    .child(
                        Button::new(format!("del-{id}"))
                            .ghost()
                            .xsmall()
                            .label(t("account.delete"))
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                this.delete_account(id, e, w, cx)
                            })),
                    )
            }))''', '''                    .child(
                        Button::new(format!("edit-{id}"))
                            .ghost()
                            .xsmall()
                            .label(t("account.edit"))
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                this.start_edit(id, String::new(), String::new(), e, w, cx)
                            })),
                    )
                    .child(
                        Button::new(format!("pw-{id}"))
                            .ghost()
                            .xsmall()
                            .label("改密码")
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                this.start_pw(id, e, w, cx)
                            })),
                    )
                    .child(
                        Button::new(format!("del-{id}"))
                            .ghost()
                            .xsmall()
                            .label(t("account.delete"))
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                this.delete_account(id, e, w, cx)
                            })),
                    )
            }))
            // 编辑表单（选中账号下方）
            .when_some(self.editing, |this, id| {
                let _ = id;
                this.child(
                    h_flex()
                        .gap_2()
                        .p_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .child(div().w(px(80.)).text_size(px(12.)).child(t("account.name")))
                        .child(div().flex_1().child(Input::new(&self.edit_name)))
                        .child(div().w(px(80.)).text_size(px(12.)).child(t("account.displayName")))
                        .child(div().flex_1().child(Input::new(&self.edit_display)))
                        .child(
                            Button::new("save-edit")
                                .primary()
                                .xsmall()
                                .label(t("common.save"))
                                .on_click(cx.listener(Self::save_edit)),
                        ),
                )
            })
            // 改密码表单
            .when_some(self.edit_password_for, |this, id| {
                let _ = id;
                this.child(
                    h_flex()
                        .gap_2()
                        .p_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .child(div().w(px(80.)).text_size(px(12.)).child(t("account.password")))
                        .child(div().flex_1().child(Input::new(&self.edit_password)))
                        .child(
                            Button::new("save-pw")
                                .primary()
                                .xsmall()
                                .label(t("common.confirm"))
                                .on_click(cx.listener(Self::save_pw)),
                        ),
                )
            })''')

open(p, 'w', encoding='utf-8').write(s)
print('batch D applied')
