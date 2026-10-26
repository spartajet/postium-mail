//! 设置窗口（独立窗口）+ 添加账号（密码型 + OAuth2 轮询）。
//! 移植自 src/routes/settings/+page.svelte 与 AddAccountModal.svelte。

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
use postium_core::service::account_service::{CreateAccountRequest, UpdateAccountRequest};

use crate::i18n::t;
use crate::runtime::spawn_tokio;
use crate::stores::accounts::AccountsStore;

/// 打开设置窗口（居中 960×720，参照旧 open_settings_window）。
pub fn open_settings_window(accounts: &Entity<AccountsStore>, api: &Arc<CoreApi>, cx: &mut App) {
    let accounts = accounts.clone();
    let api = api.clone();
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(760.), px(560.)),
                cx,
            ))),
            ..Default::default()
        },
        cx,
        move |window, cx| cx.new(|cx| SettingsView::new(accounts.clone(), api.clone(), window, cx)),
    )
    .expect("打开设置窗口失败");
    cx.activate(true);
}

pub struct SettingsView {
    accounts: Entity<AccountsStore>,
    api: Arc<CoreApi>,
    // 添加账号表单
    add_email: Entity<InputState>,
    add_password: Entity<InputState>,
    add_name: Entity<InputState>,
    add_open: bool,
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
}

impl SettingsView {
    fn new(
        accounts: Entity<AccountsStore>,
        api: Arc<CoreApi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut input = |cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
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
    }

    fn value(input: &Entity<InputState>, cx: &gpui_kit::App) -> String {
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

    fn toggle_add(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.add_open = !self.add_open;
        cx.notify();
    }

    fn detect(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let email = self.add_email.read(cx).value().to_string();
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.detect_provider(&email) });
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                this.update(cx, |view, cx| {
                    if result.detected {
                        view.detected_provider = Some(format!(
                            "{}: {}",
                            t("account.detected"),
                            result.provider_name.unwrap_or_default()
                        ));
                    } else {
                        view.detected_provider = Some(t("account.notDetected").to_string().into());
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// 提交添加：密码型直接创建；OAuth 型走 start→浏览器→poll。
    fn submit_add(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let email = self.add_email.read(cx).value().to_string();
        let password = self.add_password.read(cx).value().to_string();
        let name = self.add_name.read(cx).value().to_string();
        if email.trim().is_empty() {
            self.status = format!("{}: {}", t("common.error"), t("account.email")).into();
            cx.notify();
            return;
        }

        let api = self.api.clone();
        let accounts_store = self.accounts.clone();

        if password.trim().is_empty() {
            // OAuth 路径：start → 系统浏览器 → 轮询
            let email = email.trim().to_string();
            let display_name = if name.trim().is_empty() { None } else { Some(name.trim().to_string()) };
            let provider = api.detect_provider(&email).provider_id.unwrap_or_else(|| "generic".into());
            let api_start = api.clone();
            let api_poll_outer = api.clone();
            let rx = spawn_tokio(async move {
                api_start
                    .start_oauth2(provider, email, display_name)
                    .await
            });
            cx.spawn(async move |this, cx| {
                match rx.recv().await {
                    Ok(Ok(auth)) => {
                        let _ = open::that_detached(&auth.url);
                        let state = auth.state;
                        this.update(cx, |view, cx| {
                            view.status = "请在浏览器中完成授权...".into();
                            cx.notify();
                        })
                        .ok();

                        // 轮询（tokio 侧循环，每 1.2s，最长 5 分钟）
                        let api_poll = api_poll_outer.clone();
                        let api_reload = api_poll_outer.clone();
                        let state_poll = state.clone();
                        let rx = spawn_tokio(async move {
                            for _ in 0..250 {
                                match api_poll.poll_oauth2(&state_poll).await {
                                    Ok(postium_core::infrastructure::auth::oauth2::OAuth2PollResult::Completed(info)) => {
                                        return Some(info.email);
                                    }
                                    Ok(_) => {}
                                    Err(_) => return None,
                                }
                                smol::Timer::after(std::time::Duration::from_millis(1200)).await;
                            }
                            None
                        });
                        match rx.recv().await {
                            Ok(Some(_acct_email)) => {
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
                            }
                            _ => {
                                this.update(cx, |view, cx| {
                                    view.status = "授权超时或失败".into();
                                    view.oauth_state = None;
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        this.update(cx, |view, cx| {
                            view.status = format!("{e}").into();
                            cx.notify();
                        })
                        .ok();
                    }
                    Err(_) => {}
                }
            })
            .detach();
        } else {
            // 密码型：直接创建
            let req = CreateAccountRequest {
                name: if name.trim().is_empty() { email.clone() } else { name.clone() },
                email: email.trim().to_string(),
                display_name: None,
                provider: api.detect_provider(&email).provider_id.unwrap_or_else(|| "generic".into()),
                auth_type: "password".into(),
                password,
                imap_host: if self.manual_open { Some(Self::value(&self.imap_host, cx)) } else { None },
                imap_port: Self::value(&self.imap_port, cx).trim().parse::<i32>().ok(),
                imap_ssl_mode: None,
                smtp_host: if self.manual_open { Some(Self::value(&self.smtp_host, cx)) } else { None },
                smtp_port: Self::value(&self.smtp_port, cx).trim().parse::<i32>().ok(),
                smtp_ssl_mode: None,
                color: None,
                account_type: Some("personal".into()),
            };
            let api_keep = api.clone();
            let rx = spawn_tokio(async move { api.create_account(req).await });
            cx.spawn(async move |this, cx| {
                match rx.recv().await {
                    Ok(Ok(_)) => {
                        accounts_store.update(cx, |store, cx| {
                            store.reload(&api_keep, cx);
                        });
                        this.update(cx, |view, cx| {
                            view.status = "账号已添加 ✓".into();
                            view.add_open = false;
                            cx.notify();
                        })
                        .ok();
                    }
                    Ok(Err(e)) => {
                        this.update(cx, |view, cx| {
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

    fn delete_account(&mut self, id: i32, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        let api_keep = api.clone();
        let accounts = self.accounts.clone();
        let rx = spawn_tokio(async move { api.delete_account(id).await });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(())) = rx.recv().await {
                let api2 = api_keep.clone();
                accounts.update(cx, |store, cx| {
                    store.reload(&api2, cx);
                });
                this.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = self.accounts.read(cx).groups.clone();

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .id("settings-scroll")
            .overflow_y_scroll()
            .child(
                h_flex()
                    .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).child(t("settings.accounts")))
                    .child(
                        Button::new("add")
                            .primary()
                            .xsmall()
                            .ml_auto()
                            .label(t("account.add"))
                            .on_click(cx.listener(Self::toggle_add)),
                    ),
            )
            // 添加账号表单
            .when(self.add_open, |this| {
                let field = |label: &str, input: &Entity<InputState>| {
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().w(px(80.)).text_size(px(12.)).child(label.to_string()))
                        .child(div().flex_1().child(Input::new(input)))
                };
                this.child(
                    v_flex()
                        .p_3()
                        .gap_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .child(field(&t("account.email"), &self.add_email))
                        .child(field(&t("account.name"), &self.add_name))
                        .child(field(&t("account.password"), &self.add_password))
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
                        })
                        .when_some(self.detected_provider.clone(), |this, p| {
                            this.child(div().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(p))
                        })
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new("detect")
                                        .ghost()
                                        .xsmall()
                                        .label(t("account.detecting"))
                                        .on_click(cx.listener(Self::detect)),
                                )
                                .child(
                                    Button::new("submit")
                                        .primary()
                                        .xsmall()
                                        .label(t("common.confirm"))
                                        .on_click(cx.listener(Self::submit_add)),
                                ),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(cx.theme().muted_foreground)
                                .child(self.status.clone()),
                        ),
                )
            })
            // OAuth 后同步范围选择
            .when_some(self.oauth_pending_sync, |this, _id| {
                let mk = |key: &'static str,
                          label: SharedString,
                          range: postium_core::domain::sync::InitialSyncRange,
                          cx: &mut Context<Self>| {
                    Button::new(key)
                        .ghost()
                        .xsmall()
                        .label(label)
                        .on_click(cx.listener(move |s: &mut Self, e, w, cx| {
                            s.pick_sync_range(range, e, w, cx);
                        }))
                };
                let mut row = h_flex().flex_wrap().gap_2();
                for (key, label, range) in [
                    ("r-week", t("account.syncRangeWeek"), postium_core::domain::sync::InitialSyncRange::Week),
                    ("r-month", t("account.syncRangeMonth"), postium_core::domain::sync::InitialSyncRange::Month),
                    ("r-3m", t("account.syncRangeThreeMonths"), postium_core::domain::sync::InitialSyncRange::ThreeMonths),
                    ("r-year", t("account.syncRangeYear"), postium_core::domain::sync::InitialSyncRange::Year),
                    ("r-all", t("account.syncRangeAll"), postium_core::domain::sync::InitialSyncRange::All),
                ] {
                    row = row.child(mk(key, label, range, cx));
                }
                this.child(
                    v_flex()
                        .p_3()
                        .gap_2()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius)
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(t("account.syncScopeTitle")))
                        .child(div().text_size(px(12.)).text_color(cx.theme().muted_foreground).child(t("account.syncScopeHint")))
                        .child(row),
                )
            })
            // 账号卡片
            .children(groups.iter().map(|g| {
                let acc = &g.account;
                let id = acc.id;
                h_flex()
                    .p_3()
                    .gap_2()
                    .items_center()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .child(
                        div()
                            .w(px(10.))
                            .h(px(10.))
                            .rounded_full()
                            .bg(gpui_kit::rgb(0x3b82f6)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(acc.name.clone()))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(acc.email.clone()),
                            ),
                    )
                    .child(
                        Button::new(format!("sync-toggle-{id}"))
                            .ghost()
                            .xsmall()
                            .label(if acc.sync_enabled { "同步：开" } else { "同步：关" })
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                // 读当前值反转（从卡片显示名取状态过于绕，直接查询后反转）
                                let api = this.api.clone();
                                let api_keep = api.clone();
                                let rx = crate::runtime::spawn_tokio(async move {
                                    api.get_account(id).await.map(|a| a.sync_enabled)
                                });
                                let id2 = id;
                                cx.spawn(async move |this, cx| {
                                    if let Ok(Ok(enabled)) = rx.recv().await {
                                        let api2 = api_keep.clone();
                                        let rx = crate::runtime::spawn_tokio(async move {
                                            api2.update_account(UpdateAccountRequest {
                                                id: id2,
                                                name: None,
                                                display_name: None,
                                                color: None,
                                                sync_enabled: Some(!enabled),
                                            })
                                            .await
                                        });
                                        if let Ok(Ok(_)) = rx.recv().await {
                                            this.update(cx, |_, cx| cx.notify()).ok();
                                        }
                                    }
                                })
                                .detach();
                                let _ = (this, e, w);
                            })),
                    )
                    .child(
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
            })
    }
}
