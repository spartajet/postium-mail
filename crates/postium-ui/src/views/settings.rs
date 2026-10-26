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
use crate::stores::accounts::{AccountsEvent, AccountsStore};

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
        }
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
                                this.update(cx, |view, cx| {
                                    view.status = "账号已添加 ✓".into();
                                    view.add_open = false;
                                    cx.notify();
                                })
                                .ok();
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
                imap_host: None,
                imap_port: None,
                imap_ssl_mode: None,
                smtp_host: None,
                smtp_port: None,
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
        let store = self.accounts.read(cx);

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
            // 账号卡片
            .children(store.groups.iter().map(|g| {
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
                        Button::new(format!("del-{id}"))
                            .ghost()
                            .xsmall()
                            .label(t("account.delete"))
                            .on_click(cx.listener(move |this: &mut Self, e, w, cx| {
                                this.delete_account(id, e, w, cx)
                            })),
                    )
            }))
    }
}
