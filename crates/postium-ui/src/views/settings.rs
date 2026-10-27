//! 设置窗口（独立窗口）+ 添加账号四步向导。
//! 移植自 src/routes/settings/+page.svelte 与 AddAccountModal.svelte：
//! ① 服务商选择（预设卡片 / 其他=手动）→ ② 凭据（OAuth2 或密码，
//! 手动模式含 IMAP/SMTP + 加密模式推荐端口）→ ③ 首次同步范围 → 完成。

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
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::domain::providers::{AuthType, ProviderInfo};
use postium_core::domain::sync::InitialSyncRange;
use postium_core::service::account_service::{CreateAccountRequest, UpdateAccountRequest};

use crate::i18n::{t, t_format};
use crate::runtime::spawn_tokio;
use crate::stores::accounts::AccountsStore;

/// 打开设置窗口（居中 760×560，参照旧 open_settings_window）。
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

// ─────────────────────────────────────────────────────────────────────────────
// 添加账号向导状态
// ─────────────────────────────────────────────────────────────────────────────

/// 向导步骤（旧版 step：select | credentials | sync-scope；done 为死代码未复刻）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AddStep {
    Select,
    Credentials,
    SyncScope,
}

/// 服务商卡片的展示数据（ProviderInfo 的裁剪视图）。
#[derive(Clone)]
pub(crate) struct ProviderCard {
    pub id: String,
    pub name: String,
    /// 品牌色（解析后的 0xRRGGBB；缺省靛蓝 #6366f1，对齐旧版）。
    pub color: u32,
    pub oauth2: bool,
}

impl ProviderCard {
    fn from_info(p: &ProviderInfo) -> Self {
        let color = p
            .color
            .as_deref()
            .and_then(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok())
            .unwrap_or(0x6366f1);
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            color,
            oauth2: matches!(p.auth_type, AuthType::OAuth2),
        }
    }

    fn initial(&self) -> String {
        self.name.chars().next().map(|c| c.to_string()).unwrap_or_default()
    }
}

/// SSL/加密模式（对齐旧版 sslModes）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SslMode {
    Tls,
    StartTls,
    None,
}

impl SslMode {
    fn label(self) -> &'static str {
        match self {
            SslMode::Tls => "SSL/TLS",
            SslMode::StartTls => "STARTTLS",
            SslMode::None => "无加密",
        }
    }

    fn api_string(self) -> &'static str {
        match self {
            SslMode::Tls => "Tls",
            SslMode::StartTls => "StartTls",
            SslMode::None => "None",
        }
    }

    /// IMAP 推荐端口。
    fn imap_port(self) -> &'static str {
        match self {
            SslMode::Tls => "993",
            SslMode::StartTls | SslMode::None => "143",
        }
    }

    /// SMTP 推荐端口。
    fn smtp_port(self) -> &'static str {
        match self {
            SslMode::Tls => "465",
            SslMode::StartTls => "587",
            SslMode::None => "25",
        }
    }
}

const SSL_MODES: [SslMode; 3] = [SslMode::Tls, SslMode::StartTls, SslMode::None];

/// 添加账号向导的完整状态。
pub(crate) struct AddFlow {
    pub step: AddStep,
    pub providers: Vec<ProviderCard>,
    pub loading_providers: bool,
    /// 选中的预设服务商（None = 手动配置）。
    selected: Option<ProviderCard>,
    /// OAuth2 服务商下用户切换到密码登录。
    use_password: bool,
    // 表单
    pub email: Entity<InputState>,
    pub display_name: Entity<InputState>,
    pub password: Entity<InputState>,
    // 手动 IMAP/SMTP
    pub imap_host: Entity<InputState>,
    pub imap_port: Entity<InputState>,
    pub imap_ssl: SslMode,
    pub smtp_host: Entity<InputState>,
    pub smtp_port: Entity<InputState>,
    pub smtp_ssl: SslMode,
    // 状态
    pub detected: Option<SharedString>,
    pub error: SharedString,
    pub submitting: bool,
    oauth_polling: bool,
    /// 待选择同步范围的账号 id。
    pub pending_sync: Option<i32>,
    sync_range: InitialSyncRange,
}

impl AddFlow {
    fn new(window: &mut Window, cx: &mut Context<SettingsView>) -> Self {
        let input = |ph: &str, window: &mut Window, cx: &mut Context<SettingsView>| {
            cx.new(|cx| InputState::new(window, cx).placeholder(ph.to_string()))
        };
        let port = |v: &'static str, window: &mut Window, cx: &mut Context<SettingsView>| {
            let st = cx.new(|cx| InputState::new(window, cx));
            st.update(cx, |s, cx| s.set_value(v, window, cx));
            st
        };
        Self {
            step: AddStep::Select,
            providers: Vec::new(),
            loading_providers: true,
            selected: None,
            use_password: false,
            email: input("you@example.com", window, cx),
            display_name: input(&t("account.displayName"), window, cx),
            password: input("", window, cx),
            imap_host: input("imap.example.com", window, cx),
            imap_port: port("993", window, cx),
            imap_ssl: SslMode::Tls,
            smtp_host: input("smtp.example.com", window, cx),
            smtp_port: port("465", window, cx),
            smtp_ssl: SslMode::Tls,
            detected: None,
            error: "".into(),
            submitting: false,
            oauth_polling: false,
            pending_sync: None,
            sync_range: InitialSyncRange::ThreeMonths,
        }
    }

    /// 手动模式（未选预设）。
    fn is_manual(&self) -> bool {
        self.selected.is_none()
    }

    /// OAuth2 路径可见（服务商支持且未切换到密码）。
    fn oauth_visible(&self) -> bool {
        self.selected.as_ref().is_some_and(|p| p.oauth2) && !self.use_password
    }

    fn reset_for_select(&mut self) {
        self.step = AddStep::Select;
        self.selected = None;
        self.use_password = false;
        self.error = "".into();
        self.detected = None;
        self.submitting = false;
        self.oauth_polling = false;
        self.pending_sync = None;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 设置视图
// ─────────────────────────────────────────────────────────────────────────────

/// 设置分区（对齐旧版 settingsSections：通用/外观/账户）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SettingsSection {
    General,
    Appearance,
    Accounts,
}

pub struct SettingsView {
    /// 当前分区
    pub(crate) active_section: SettingsSection,
    pub(crate) accounts: Entity<AccountsStore>,
    api: Arc<CoreApi>,
    /// 主题偏好（dark | light | system）
    pub(crate) theme_pref: String,
    /// 系统外观观察订阅（保持存活）
    _appearance_sub: gpui_kit::Subscription,
    /// 添加账号弹窗的焦点句柄（ESC 关闭 / 焦点圈定）
    modal_focus: gpui_kit::FocusHandle,
    // 添加账号向导（Some = 弹窗打开）
    pub(crate) add: Option<AddFlow>,
    status: SharedString,
    /// 编辑中的账号（名称/显示名）
    editing: Option<i32>,
    edit_name: Entity<InputState>,
    edit_display: Entity<InputState>,
    edit_password_for: Option<i32>,
    edit_password: Entity<InputState>,
}

impl SettingsView {
    pub(crate) fn new(
        accounts: Entity<AccountsStore>,
        api: Arc<CoreApi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut input = |cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx));
        // ESC 关闭添加账号弹窗（对齐旧版模态行为）
        let modal_focus = cx.focus_handle();
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "escape",
            gpui_kit::base::actions::Cancel,
            Some("AddAccountModal"),
        )]);
        let edit_name = input(cx);
        let edit_display = input(cx);
        let edit_password = input(cx);
        // 设置窗口同样跟随系统外观（偏好为 system 时）
        let weak = cx.entity().downgrade();
        let appearance_sub = window.observe_window_appearance(move |window, cx| {
            let pref = crate::ui_state::load().theme.unwrap_or_else(|| "dark".into());
            if pref == "system" {
                let _ = weak.update(cx, |_, cx| {
                    crate::theme::apply_mode(crate::theme::resolve_mode("system", window.appearance()), cx);
                    cx.notify();
                });
            }
        });
        Self {
            active_section: SettingsSection::General,
            accounts,
            api,
            theme_pref: crate::ui_state::load().theme.unwrap_or_else(|| "dark".into()),
            _appearance_sub: appearance_sub,
            modal_focus,
            add: None,
            status: " ".into(),
            editing: None,
            edit_name,
            edit_display,
            edit_password_for: None,
            edit_password,
        }
    }

    fn value(input: &Entity<InputState>, cx: &gpui_kit::App) -> String {
        input.read(cx).value().to_string()
    }

    /// 设置主题偏好（浅色/深色/跟随系统）并持久化。
    pub(crate) fn set_theme_pref(&mut self, pref: &str, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        crate::theme::apply_mode(crate::theme::resolve_mode(pref, window.appearance()), cx);
        self.theme_pref = pref.to_string();
        let mut ui = crate::ui_state::load();
        ui.theme = Some(pref.to_string());
        crate::ui_state::save(&ui);
        refresh_all_windows(cx);
        cx.notify();
    }

    /// 切换语言并持久化（全部窗口刷新以重渲染文案）。
    pub(crate) fn set_lang(&mut self, lang: crate::i18n::Lang, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        crate::i18n::set_lang(lang, cx);
        let mut ui = crate::ui_state::load();
        ui.lang = Some(match lang { crate::i18n::Lang::Zh => "zh".into(), crate::i18n::Lang::En => "en".into() });
        crate::ui_state::save(&ui);
        refresh_all_windows(cx);
        cx.notify();
    }

    // ── 向导控制（pub(crate) 供 e2e 驱动） ──

    /// 打开向导并加载服务商列表。
    pub(crate) fn open_add(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.add = Some(AddFlow::new(window, cx));
        let focus = self.modal_focus.clone();
        window.focus(&focus, cx);
        let api = self.api.clone();
        let rx = spawn_tokio(async move { api.list_providers() });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(providers)) = rx.recv().await {
                this.update(cx, |view, cx| {
                    if let Some(add) = view.add.as_mut() {
                        add.providers = providers.iter().map(ProviderCard::from_info).collect();
                        add.loading_providers = false;
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    /// 关闭向导（X / 背景点击）。
    pub(crate) fn close_add(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.add = None;
        cx.notify();
    }

    /// 选择预设服务商。
    pub(crate) fn select_provider(&mut self, idx: usize, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_mut() else { return };
        let Some(card) = add.providers.get(idx).cloned() else { return };
        add.selected = Some(card);
        add.use_password = false;
        add.step = AddStep::Credentials;
        add.error = "".into();
        add.detected = None;
        cx.notify();
    }

    /// 选择"其他"（手动配置）。
    pub(crate) fn select_manual(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_mut() else { return };
        add.selected = None;
        add.use_password = true;
        add.step = AddStep::Credentials;
        add.error = "".into();
        add.detected = None;
        cx.notify();
    }

    /// 返回第一步。
    pub(crate) fn back_to_select(&mut self, cx: &mut Context<Self>) {
        if let Some(add) = self.add.as_mut() {
            add.reset_for_select();
        }
        cx.notify();
    }

    /// OAuth2 服务商下切换到密码登录。
    pub(crate) fn switch_to_password(&mut self, cx: &mut Context<Self>) {
        if let Some(add) = self.add.as_mut() {
            add.use_password = true;
            add.error = "".into();
        }
        cx.notify();
    }

    /// 检测邮箱服务商（邮箱输入旁按钮）。
    pub(crate) fn detect_provider(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_ref() else { return };
        let email = Self::value(&add.email, cx);
        if !email.contains('@') {
            return;
        }
        let result = self.api.detect_provider(&email);
        let detected = if result.detected {
            format!("{}: {}", t("account.detected"), result.provider_name.unwrap_or_default())
        } else {
            t("account.notDetected").to_string()
        };
        self.add.as_mut().unwrap().detected = Some(detected.into());
        cx.notify();
    }

    /// 切换 IMAP/SMTP 加密模式并推荐端口。
    pub(crate) fn set_ssl_mode(
        &mut self,
        imap: bool,
        mode: SslMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(add) = self.add.as_mut() else { return };
        if imap {
            add.imap_ssl = mode;
            add.imap_port.update(cx, |s, cx| s.set_value(mode.imap_port(), window, cx));
        } else {
            add.smtp_ssl = mode;
            add.smtp_port.update(cx, |s, cx| s.set_value(mode.smtp_port(), window, cx));
        }
        cx.notify();
    }

    /// 提交凭据：密码型直接创建；OAuth 型走 浏览器授权→轮询。
    pub(crate) fn submit_credentials(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_ref() else { return };
        let email = Self::value(&add.email, cx).trim().to_string();
        let password = Self::value(&add.password, cx);
        let display = Self::value(&add.display_name, cx);
        let display = if display.trim().is_empty() { None } else { Some(display.trim().to_string()) };
        let manual = add.is_manual();
        let provider = add.selected.clone();

        // 校验
        if email.is_empty() {
            self.set_add_error(format!("{}: {}", t("common.error"), t("account.email")), cx);
            return;
        }
        let oauth_path = add.oauth_visible();
        if !oauth_path && password.trim().is_empty() {
            self.set_add_error(format!("{}: {}", t("common.error"), t("account.password")), cx);
            return;
        }
        if manual {
            let imap_host = Self::value(&add.imap_host, cx);
            let smtp_host = Self::value(&add.smtp_host, cx);
            if imap_host.trim().is_empty() || smtp_host.trim().is_empty() {
                self.set_add_error(t("account.notDetected").to_string(), cx);
                return;
            }
        }
        let _ = window;

        let api = self.api.clone();
        let accounts_store = self.accounts.clone();

        if oauth_path {
            // ── OAuth2：start → 系统浏览器 → 轮询 → 按邮箱定位账号 ──
            let provider_id = provider.as_ref().map(|p| p.id.clone()).unwrap_or_else(|| "generic".into());
            let api_start = api.clone();
            let rx = spawn_tokio(async move { api_start.start_oauth2(provider_id, email, display).await });
            if let Some(add) = self.add.as_mut() {
                add.oauth_polling = true;
                add.error = "".into();
            }
            cx.notify();
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
                        // 轮询（tokio 侧，每 1.2s，最长 5 分钟）
                        let api_poll = api.clone();
                        let rx = spawn_tokio(async move {
                            for _ in 0..250 {
                                match api_poll.poll_oauth2(&state).await {
                                    Ok(postium_core::infrastructure::auth::oauth2::OAuth2PollResult::Completed(info)) => {
                                        return Some(info.email);
                                    }
                                    Ok(postium_core::infrastructure::auth::oauth2::OAuth2PollResult::Error(e)) => {
                                        return Some(format!("__error__{e}"));
                                    }
                                    Ok(_) => {}
                                    Err(_) => return None,
                                }
                                smol::Timer::after(std::time::Duration::from_millis(1200)).await;
                            }
                            None
                        });
                        match rx.recv().await {
                            Ok(Some(acct_email)) if !acct_email.starts_with("__error__") => {
                                // 按邮箱定位账号（对齐旧版 resolveOAuth2CompletedAccount）
                                let api_list = api.clone();
                                let rx = spawn_tokio(async move { api_list.list_accounts().await.unwrap_or_default() });
                                if let Ok(accs) = rx.recv().await {
                                    let id = accs.iter().find(|a| a.email == acct_email).map(|a| a.id);
                                    accounts_store.update(cx, |store, cx| store.reload(&api, cx));
                                    this.update(cx, |view, cx| {
                                        if let Some(add) = view.add.as_mut() {
                                            add.oauth_polling = false;
                                            match id {
                                                Some(id) => {
                                                    add.pending_sync = Some(id);
                                                    add.step = AddStep::SyncScope;
                                                }
                                                None => {
                                                    add.error = t_format(
                                                        "account.oauthAccountMissing",
                                                        &[("{email}", &acct_email)],
                                                    );
                                                }
                                            }
                                        }
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            }
                            Ok(Some(err)) => {
                                this.update(cx, |view, cx| {
                                    if let Some(add) = view.add.as_mut() {
                                        add.oauth_polling = false;
                                        add.error = err.trim_start_matches("__error__").into();
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                            _ => {
                                this.update(cx, |view, cx| {
                                    if let Some(add) = view.add.as_mut() {
                                        add.oauth_polling = false;
                                        add.error = "OAuth2 授权超时，请重试".into();
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        this.update(cx, |view, cx| {
                            if let Some(add) = view.add.as_mut() {
                                add.oauth_polling = false;
                                add.error = format!("{e}").into();
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                    Err(_) => {}
                }
            })
            .detach();
        } else {
            // ── 密码型：直接创建 ──
            let name = email.split('@').next().unwrap_or(&email).to_string();
            let req = CreateAccountRequest {
                name,
                email,
                display_name: display,
                provider: provider.as_ref().map(|p| p.id.clone()).unwrap_or_else(|| "custom".into()),
                auth_type: "Password".into(),
                password,
                imap_host: if manual { Some(Self::value(&add.imap_host, cx).trim().to_string()) } else { None },
                imap_port: Self::value(&add.imap_port, cx).trim().parse::<i32>().ok(),
                imap_ssl_mode: if manual { Some(add.imap_ssl.api_string().to_string()) } else { None },
                smtp_host: if manual { Some(Self::value(&add.smtp_host, cx).trim().to_string()) } else { None },
                smtp_port: Self::value(&add.smtp_port, cx).trim().parse::<i32>().ok(),
                smtp_ssl_mode: if manual { Some(add.smtp_ssl.api_string().to_string()) } else { None },
                color: provider.as_ref().map(|p| format!("#{:06x}", p.color)),
                account_type: Some("personal".into()),
            };
            if let Some(add) = self.add.as_mut() {
                add.submitting = true;
                add.error = "".into();
            }
            cx.notify();
            let api_keep = api.clone();
            let rx = spawn_tokio(async move { api.create_account(req).await });
            cx.spawn(async move |this, cx| {
                match rx.recv().await {
                    Ok(Ok(acc)) => {
                        accounts_store.update(cx, |store, cx| {
                            store.reload(&api_keep, cx);
                        });
                        this.update(cx, |view, cx| {
                            if let Some(add) = view.add.as_mut() {
                                add.submitting = false;
                                add.pending_sync = Some(acc.id);
                                add.step = AddStep::SyncScope;
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                    Ok(Err(e)) => {
                        this.update(cx, |view, cx| {
                            if let Some(add) = view.add.as_mut() {
                                add.submitting = false;
                                add.error = format!("{e}").into();
                            }
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

    /// 选择同步范围选项。
    pub(crate) fn set_sync_range(&mut self, range: InitialSyncRange, cx: &mut Context<Self>) {
        if let Some(add) = self.add.as_mut() {
            add.sync_range = range;
        }
        cx.notify();
    }

    /// 开始首次同步：后台同步 + 刷新账号列表 + 关闭向导与设置窗口。
    pub(crate) fn start_initial_sync(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_sync(cx);
        self.add = None;
        cx.notify();
        // 延迟关窗：事件分发栈上同步移除自身窗口不安全（对齐 compose 的异步关窗模式）
        let handle = window.window_handle();
        cx.defer(move |cx| {
            let alive = cx.windows().iter().any(|w| w.window_id() == handle.window_id());
            if alive {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        });
    }

    /// 触发后台首次同步并刷新账号列表（不关窗；供向导与测试复用）。
    pub(crate) fn begin_sync(&mut self, cx: &mut Context<Self>) {
        let Some(add) = self.add.as_ref() else { return };
        let Some(id) = add.pending_sync else { return };
        let range = add.sync_range;
        let api = self.api.clone();
        let api_sync = self.api.clone();
        let accounts_store = self.accounts.clone();
        // 后台首次同步（事件通道丢弃，进度经主窗口 SyncStore 状态栏感知）
        let (tx, _rx) = smol::channel::bounded(256);
        let _ = spawn_tokio(async move {
            let _ = api_sync.sync_account_with_range(tx, id, range).await;
        });
        accounts_store.update(cx, |store, cx| store.reload(&api, cx));
    }

    fn set_add_error(&mut self, msg: impl Into<SharedString>, cx: &mut Context<Self>) {
        if let Some(add) = self.add.as_mut() {
            add.error = msg.into();
        }
        cx.notify();
    }

    // ── 账号管理 ──

    fn start_edit(&mut self, id: i32, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.editing = Some(id);
        self.edit_password_for = None;
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

/// 添加账号四步向导。
/// 移植自 src/lib/components/settings/AddAccountModal.svelte。

// ─────────────────────────────────────────────────────────────────────────────
// 渲染
// ─────────────────────────────────────────────────────────────────────────────

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let groups = self.accounts.read(cx).groups.clone();
        let mut account_cards: Vec<AnyElement> = Vec::new();
        for g in &groups {
            account_cards.push(render_account_card(g, cx).into_any_element());
        }

        // 左侧分区导航（对齐旧版侧栏：设置图标+标题，三导航项）
        let mut nav = v_flex()
            .w(px(180.))
            .flex_none()
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .p_3()
            .gap_1()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_2()
                    .child(
                        gpui_kit::component::Icon::new(IconName::Settings)
                            .size_4()
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t("settings.title")),
                    ),
            );
        for (key, section, icon, label) in [
            ("nav-general", SettingsSection::General, IconName::SlidersHorizontal, t("settings.general")),
            ("nav-appearance", SettingsSection::Appearance, IconName::Palette, t("settings.appearance")),
            ("nav-accounts", SettingsSection::Accounts, IconName::UserRound, t("settings.accounts")),
        ] {
            let sel = self.active_section == section;
            let sec = section;
            nav = nav.child(
                div()
                    .id(key)
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py(px(7.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .when(sel, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
                    .when(!sel, |d| {
                        d.text_color(theme.muted_foreground)
                            .hover(|s| s.bg(theme.accent.opacity(0.10)).text_color(theme.foreground))
                    })
                    .child(gpui_kit::component::Icon::new(icon).size_4())
                    .child(label)
                    .on_click(cx.listener(move |v: &mut SettingsView, _, _, cx| {
                        v.active_section = sec;
                        cx.notify();
                    })),
            );
        }

        // 右侧面板：按分区渲染（旧版 max-w-3xl 卡片风格）
        let panel: AnyElement = match self.active_section {
            SettingsSection::General => v_flex()
                .p_5()
                .gap_2()
                .w(px(640.))
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius)
                .child(
                    h_flex().items_center().gap_2().child(
                        gpui_kit::component::Icon::new(IconName::SlidersHorizontal)
                            .size_4()
                            .text_color(theme.accent),
                    )
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(t("settings.generalTitle")),
                    ),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme.muted_foreground)
                        .child(t("settings.generalDescription")),
                )
                .into_any_element(),
            SettingsSection::Appearance => v_flex()
                .p_5()
                .gap_3()
                .w(px(640.))
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius)
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(
                            gpui_kit::component::Icon::new(IconName::Palette)
                                .size_4()
                                .text_color(theme.accent),
                        )
                        .child(
                            div()
                                .text_size(px(13.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(t("settings.appearance")),
                        ),
                )
                .child(render_theme_row(self, cx))
                .child(render_language_row(cx))
                .into_any_element(),
            SettingsSection::Accounts => v_flex()
                .p_5()
                .gap_3()
                .w(px(640.))
                .border_1()
                .border_color(theme.border)
                .rounded(theme.radius)
                .child(
                    h_flex()
                        .items_center()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .child(
                                    gpui_kit::component::Icon::new(IconName::UserRound)
                                        .size_4()
                                        .text_color(theme.accent),
                                )
                                .child(
                                    div()
                                        .text_size(px(13.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(t("settings.accounts")),
                                ),
                        )
                        .child(
                            Button::new("add")
                                .primary()
                                .xsmall()
                                .label(t("account.add"))
                                .on_click(cx.listener(Self::open_add)),
                        ),
                )
                .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(self.status.clone()))
                .children(account_cards)
                .into_any_element(),
        };

        h_flex()
            .relative()
            .size_full()
            .child(nav)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .p_5()
                    .id("settings-scroll")
                    .overflow_y_scroll()
                    .child(panel),
            )
            // 编辑表单
            .when_some(self.editing, |this, _| {
                this.child(
                    h_flex()
                        .gap_2()
                        .p_2()
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius)
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
            .when_some(self.edit_password_for, |this, _| {
                this.child(
                    h_flex()
                        .gap_2()
                        .p_2()
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius)
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
            // 添加账号向导（模态覆盖层）
            .when(self.add.is_some(), |this| this.child(render_add_modal(self, window, cx)))
    }
}

/// 账号管理卡片（同步开关 / 编辑 / 改密码 / 删除）。
fn render_account_card(g: &crate::stores::accounts::AccountNode, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let acc = &g.account;
    let id = acc.id;
    let theme = cx.theme().clone();
    // 首字圆形头像（对齐旧版 account.email.charAt(0) 大写）
    let initial = acc
        .email
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();
    h_flex()
        .p_3()
        .gap_2()
        .items_center()
        .border_1()
        .border_color(theme.border)
        .rounded(theme.radius)
        .child(
            div()
                .w(px(30.))
                .h(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(theme.accent.opacity(0.15))
                .text_color(theme.accent)
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .child(initial),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_clamp(1)
                        .child(acc.email.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme.muted_foreground)
                        .child(acc.provider.clone()),
                ),
        )
        .child(
            Button::new(format!("sync-toggle-{id}"))
                .ghost()
                .xsmall()
                .label(if acc.sync_enabled { "同步：开" } else { "同步：关" })
                .on_click(cx.listener(move |this: &mut SettingsView, e, w, cx| {
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
                .on_click(cx.listener(move |this: &mut SettingsView, e, w, cx| {
                    this.start_edit(id, e, w, cx);
                })),
        )
        .child(
            Button::new(format!("pw-{id}"))
                .ghost()
                .xsmall()
                .label("改密码")
                .on_click(cx.listener(move |this: &mut SettingsView, e, w, cx| {
                    this.start_pw(id, e, w, cx);
                })),
        )
        .child(
            Button::new(format!("del-{id}"))
                .ghost()
                .xsmall()
                .label(t("account.delete"))
                .on_click(cx.listener(move |this: &mut SettingsView, e, w, cx| {
                    this.delete_account(id, e, w, cx);
                })),
        )
}

/// 添加账号模态覆盖层（背景点击关闭 + 卡片体阻断穿透）。
fn render_add_modal(view: &mut SettingsView, _window: &mut Window, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let add = view.add.as_ref().expect("render_add_modal 无向导");

    // 头部：标题 + 步骤指示器 + 关闭
    let step = add.step;
    let step_item = |n: usize, label: SharedString, active: bool| -> AnyElement {
        div()
            .text_size(px(12.))
            .when(active, |d| d.font_weight(FontWeight::MEDIUM).text_color(theme.accent))
            .when(!active, |d| d.text_color(theme.muted_foreground))
            .child(format!("{n}. {label}"))
            .into_any_element()
    };
    let header = h_flex()
        .items_center()
        .px_5()
        .py_3()
        .border_b_1()
        .border_color(theme.border)
        .child(
            v_flex()
                .flex_1()
                .gap_1()
                .child(div().text_size(px(15.)).font_weight(FontWeight::BOLD).child(t("account.add")))
                .child(
                    h_flex()
                        .gap_1p5()
                        .items_center()
                        .child(step_item(1, t("account.selectProvider"), step == AddStep::Select))
                        .child(chevron(&theme))
                        .child(step_item(2, t("account.step2"), step == AddStep::Credentials))
                        .child(chevron(&theme))
                        .child(step_item(3, t("account.startSync"), step == AddStep::SyncScope))
                        .child(chevron(&theme))
                        .child(step_item(4, t("account.step3"), false)),
                ),
        )
        .child(
            Button::new("add-close")
                .ghost()
                .icon(IconName::X)
                .on_click(cx.listener(SettingsView::close_add)),
        );

    let body: AnyElement = match step {
        AddStep::Select => render_step_select(add, cx).into_any_element(),
        AddStep::Credentials => render_step_credentials(view, cx).into_any_element(),
        AddStep::SyncScope => render_step_sync_scope(view, cx).into_any_element(),
    };

    // 模态卡片：体阻断穿透（学日历浮层教训）
    let card = v_flex()
        .id("add-account-modal")
        .w(px(500.))
        .rounded(px(12.))
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .child(header)
        .child(body);

    let modal_focus = view.modal_focus.clone();
    gpui_kit::base::FocusTrapElement::focus_trap(
        div()
            .id("add-account-backdrop")
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::black().opacity(0.5))
            .track_focus(&modal_focus)
            .key_context("AddAccountModal")
            .on_action(cx.listener(|v: &mut SettingsView, _: &gpui_kit::base::actions::Cancel, w, cx| {
                if v.add.is_some() {
                    v.close_add(&gpui_kit::ClickEvent::default(), w, cx);
                }
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .child(card),
        "add-account-modal-trap",
        &modal_focus,
    )
}

fn chevron(theme: &gpui_kit::component::Theme) -> impl IntoElement {
    gpui_kit::component::Icon::new(IconName::ChevronRight)
        .size_3()
        .text_color(theme.muted_foreground.opacity(0.6))
}

/// 第一步：服务商网格 + 其他。
fn render_step_select(add: &AddFlow, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();

    let mut rows = v_flex().gap_2();
    if add.loading_providers {
        rows = rows.child(
            h_flex()
                .items_center()
                .justify_center()
                .gap_2()
                .py_8()
                .child(gpui_kit::component::Icon::new(IconName::LoaderCircle).size_5().text_color(theme.muted_foreground))
                .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(t("email.loading"))),
        );
    } else {
        // 4 列网格
        let mut row = h_flex().gap_2();
        let mut count = 0;
        for (idx, p) in add.providers.iter().enumerate() {
            let card = v_flex()
                .id(format!("provider-{idx}"))
                .flex_1()
                .items_center()
                .gap_2()
                .py_3()
                .rounded(px(8.))
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.border_color(theme.accent.opacity(0.5)).bg(theme.accent.opacity(0.08)))
                .child(
                    div()
                        .w(px(36.))
                        .h(px(36.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .bg(gpui_kit::rgb(p.color))
                        .text_color(gpui_kit::white())
                        .text_size(px(14.))
                        .font_weight(FontWeight::BOLD)
                        .child(p.initial()),
                )
                .child(div().text_size(px(12.)).child(p.name.clone()))
                .on_click({
                    cx.listener(move |view: &mut SettingsView, _, _, cx| {
                        view.select_provider(idx, cx);
                    })
                });
            row = row.child(card);
            count += 1;
            if count % 4 == 0 {
                rows = rows.child(row);
                row = h_flex().gap_2();
            }
        }
        // "其他"卡片（虚线框 + 地球图标）
        let other = v_flex()
            .id("provider-other")
            .flex_1()
            .items_center()
            .gap_2()
            .py_3()
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .cursor_pointer()
            .hover(|s| s.border_color(theme.accent.opacity(0.5)).bg(theme.accent.opacity(0.08)))
            .child(
                div()
                    .w(px(36.))
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(theme.muted)
                    .child(
                        gpui_kit::component::Icon::new(IconName::Globe)
                            .size_4()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(t("account.other")))
            .on_click(cx.listener(|view: &mut SettingsView, _, _, cx| {
                view.select_manual(cx);
            }));
        row = row.child(other);
        rows = rows.child(row);
    }

    v_flex()
        .p_5()
        .gap_3()
        .id("add-select-scroll")
        .h(px(420.))
        .overflow_y_scroll()
        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(t("account.selectProviderHint")))
        .child(rows)
}

/// 表单字段块（label 上置 + 输入行）。
fn field_block(
    key: &str,
    label: SharedString,
    input: &Entity<InputState>,
    trailing: Option<impl IntoElement>,
) -> impl IntoElement {
    let mut row = h_flex().flex_1().gap_2().items_center().child(Input::new(input));
    if let Some(extra) = trailing {
        row = row.child(extra);
    }
    v_flex()
        .id(format!("field-{key}"))
        .gap_1()
        .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child(label))
        .child(row)
}

/// 第二步：凭据（OAuth2 / 密码 / 手动 IMAP·SMTP）。
fn render_step_credentials(view: &mut SettingsView, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let add = view.add.as_ref().expect("credentials 无向导");
    let manual = add.is_manual();
    let oauth = add.oauth_visible();

    // 服务商信息卡 / 手动提示
    let provider_card = if let Some(p) = &add.selected {
        h_flex()
            .gap_3()
            .items_center()
            .px_3()
            .py_2()
            .rounded(px(8.))
            .bg(theme.muted.opacity(0.5))
            .child(
                div()
                    .w(px(28.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(gpui_kit::rgb(p.color))
                    .text_color(gpui_kit::white())
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .child(p.initial()),
            )
            .child(
                v_flex().child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(p.name.clone())).child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child(if p.oauth2 { "OAuth2 / Password" } else { "Password" }),
                ),
            )
            .into_any_element()
    } else {
        v_flex()
            .px_3()
            .py_2()
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(t("account.otherHint")),
            )
            .into_any_element()
    };

    // 邮箱行（带检测按钮）
    let email_block = field_block(
        "email",
        t("account.email"),
        &add.email,
        Some(
            Button::new("detect")
                .ghost()
                .xsmall()
                .label(t("account.detected"))
                .on_click(cx.listener(SettingsView::detect_provider)),
        ),
    );
    let detect_hint = add.detected.clone().map(|d| {
        div()
            .text_size(px(11.))
            .text_color(if d.contains(t("account.detected").as_ref()) {
                gpui_kit::Hsla::from(gpui_kit::rgb(0x10b981))
            } else {
                theme.muted_foreground
            })
            .child(d)
    });

    let display_block = field_block("display", t("account.displayName"), &add.display_name, None::<Button>);

    // 底部：返回 + 确认
    let footer = h_flex()
        .pt_1()
        .gap_2()
        .child(
            Button::new("add-back")
                .ghost()
                .icon(IconName::ChevronLeft)
                .label(t("account.step1"))
                .on_click(cx.listener(|view: &mut SettingsView, _, _, cx| {
                    view.back_to_select(cx);
                })),
        )
        .child(
            div()
                .flex_1()
                .when_some(
                    (!add.error.is_empty()).then(|| add.error.clone()),
                    |this, e| this.child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .text_color(gpui_kit::rgb(0xf43f5e))
                            .line_clamp(2)
                            .child(e),
                    ),
                ),
        )
        .child(if add.submitting {
            div()
                .h(px(32.))
                .flex()
                .items_center()
                .px_4()
                .rounded(px(8.))
                .bg(theme.muted)
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child("正在验证...")
                .into_any_element()
        } else {
            crate::theme::compose_button(t("common.confirm"), {
                cx.listener(|view: &mut SettingsView, e, w, cx| {
                    view.submit_credentials(e, w, cx);
                })
            })
            .into_any_element()
        });

    let mut body = v_flex().p_5().gap_3();

    if oauth {
        // ── OAuth2 路径 ──
        let oauth_btn = if add.oauth_polling || SettingsView::value(&add.email, cx).trim().is_empty() {
            let label = if add.oauth_polling { "等待授权中..." } else { "使用 OAuth2 登录" };
            h_flex()
                .id("oauth-btn-disabled")
                .w_full()
                .h(px(36.))
                .items_center()
                .justify_center()
                .gap_2()
                .rounded(px(8.))
                .bg(theme.muted)
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child(gpui_kit::component::Icon::new(IconName::LoaderCircle).size_4())
                .child(label)
                .into_any_element()
        } else {
            h_flex()
                .id("oauth-btn")
                .w_full()
                .h(px(36.))
                .items_center()
                .justify_center()
                .gap_2()
                .rounded(px(8.))
                .cursor_pointer()
                .bg(gpui_kit::linear_gradient(
                    135.0,
                    gpui_kit::linear_color_stop(gpui_kit::rgb(0x7c3aed), 0.0),
                    gpui_kit::linear_color_stop(gpui_kit::rgb(0xa78bfa), 1.0),
                ))
                .text_color(gpui_kit::white())
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .child(gpui_kit::component::Icon::new(IconName::ExternalLink).size_4())
                .child("使用 OAuth2 登录")
                .on_click(cx.listener(|view: &mut SettingsView, e, w, cx| {
                    view.submit_credentials(e, w, cx);
                }))
                .into_any_element()
        };

        body = body
            .child(provider_card)
            .child(email_block)
            .when_some(detect_hint, |this, h| this.child(h))
            .child(display_block)
            .child(oauth_btn)
            .when(add.oauth_polling, |this| {
                this.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child("已在浏览器中打开授权页面，完成授权后自动继续"),
                )
            });
        // "或"分隔 + 切换密码登录
        body = body.child(or_divider(&theme)).child(
            Button::new("switch-password")
                .outline()
                .w_full()
                .icon(IconName::Shield)
                .label("使用密码登录")
                .on_click(cx.listener(|view: &mut SettingsView, _, _, cx| {
                    view.switch_to_password(cx);
                })),
        );
        // 切换后显示密码表单
        if add.use_password {
            body = body
                .child(field_block("password", t("account.password"), &add.password, None::<Button>))
                .child(footer);
        } else {
            body = body.child(footer);
        }
    } else {
        // ── 密码 / 手动路径 ──
        body = body
            .child(provider_card)
            .child(email_block)
            .when_some(detect_hint, |this, h| this.child(h))
            .child(display_block)
            .child(field_block("password", t("account.password"), &add.password, None::<Button>));

        if manual {
            body = body.child(manual_server_block(add, cx));
        }
        body = body.child(footer);
    }

    body
        .id("add-credentials-scroll")
        .h(px(420.))
        .overflow_y_scroll()
}

/// "或"分隔线。
fn or_divider(theme: &gpui_kit::component::Theme) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_2()
        .child(div().flex_1().h(px(1.)).bg(theme.border))
        .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child("或"))
        .child(div().flex_1().h(px(1.)).bg(theme.border))
}

/// 手动 IMAP/SMTP 配置块（加密模式分段选择 + 推荐端口）。
fn manual_server_block(add: &AddFlow, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();

    // 加密模式分段控件
    let ssl_group = |key: &'static str, imap: bool, current: SslMode| -> AnyElement {
        let mut group = h_flex().gap_1();
        for (i, mode) in SSL_MODES.iter().enumerate() {
            let sel = *mode == current;
            group = group.child(
                div()
                    .id(format!("{key}-{i}"))
                    .px(px(8.))
                    .py(px(3.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_size(px(11.))
                    .when(sel, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
                    .when(!sel, |d| {
                        d.bg(theme.muted)
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.bg(theme.accent.opacity(0.15)))
                    })
                    .child(mode.label())
                    .on_click({
                        cx.listener(move |view: &mut SettingsView, _, window, cx| {
                            view.set_ssl_mode(imap, *mode, window, cx);
                        })
                    }),
            );
        }
        group.into_any_element()
    };

    let small_label = |txt: &str| div().text_size(px(11.)).text_color(theme.muted_foreground).child(txt.to_string());

    v_flex()
        .gap_3()
        .p_3()
        .rounded(px(8.))
        .border_1()
        .border_color(theme.border)
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(t("account.manualConfig")))
        // IMAP 行
        .child(
            h_flex()
                .gap_2()
                .items_end()
                .child(v_flex().flex_1().gap_1().child(small_label(&t("account.imapHost"))).child(Input::new(&add.imap_host)))
                .child(v_flex().w(px(80.)).gap_1().child(small_label(&t("account.imapPort"))).child(Input::new(&add.imap_port)))
                .child(v_flex().gap_1().child(small_label("IMAP 加密")).child(ssl_group("imap-ssl", true, add.imap_ssl))),
        )
        // SMTP 行
        .child(
            h_flex()
                .gap_2()
                .items_end()
                .child(v_flex().flex_1().gap_1().child(small_label(&t("account.smtpHost"))).child(Input::new(&add.smtp_host)))
                .child(v_flex().w(px(80.)).gap_1().child(small_label(&t("account.smtpPort"))).child(Input::new(&add.smtp_port)))
                .child(v_flex().gap_1().child(small_label("SMTP 加密")).child(ssl_group("smtp-ssl", false, add.smtp_ssl))),
        )
}

/// 第三步：首次同步范围。
fn render_step_sync_scope(view: &mut SettingsView, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let add = view.add.as_ref().expect("sync-scope 无向导");
    let current = add.sync_range;

    let options: [(InitialSyncRange, SharedString, Option<SharedString>); 5] = [
        (InitialSyncRange::Week, t("account.syncRangeWeek"), None),
        (InitialSyncRange::Month, t("account.syncRangeMonth"), None),
        (InitialSyncRange::ThreeMonths, t("account.syncRangeThreeMonths"), None),
        (InitialSyncRange::Year, t("account.syncRangeYear"), None),
        (InitialSyncRange::All, t("account.syncRangeAll"), Some(t("account.syncRangeAllHint"))),
    ];

    let mut list = v_flex().gap_2();
    for (i, (range, label, hint)) in options.iter().enumerate() {
        let sel = *range == current;
        let range = *range;
        list = list.child(
            div()
                .id(format!("sync-opt-{i}"))
                .flex()
                .items_start()
                .gap_3()
                .px_3()
                .py(px(8.))
                .rounded(px(8.))
                .border_1()
                .cursor_pointer()
                .when(sel, |d| d.border_color(theme.accent).bg(theme.accent.opacity(0.10)))
                .when(!sel, |d| {
                    d.border_color(theme.border)
                        .hover(|s| s.bg(theme.accent.opacity(0.06)))
                })
                .on_click(cx.listener(move |view: &mut SettingsView, _, _, cx| {
                    view.set_sync_range(range, cx);
                }))
                // 单选圆点
                .child(
                    div()
                        .mt(px(2.))
                        .w(px(14.))
                        .h(px(14.))
                        .rounded_full()
                        .border_1()
                        .border_color(if sel { theme.accent } else { theme.border })
                        .when(sel, |d| {
                            d.bg(theme.accent)
                                .border_color(theme.accent)
                                .child(div().size_full().flex().items_center().justify_center().child(
                                    div().w(px(6.)).h(px(6.)).rounded_full().bg(theme.accent_foreground),
                                ))
                        }),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .gap_0p5()
                        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(label.clone()))
                        .when_some(hint.clone(), |this, h| {
                            this.child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(h))
                        }),
                ),
        );
    }

    v_flex()
        .p_5()
        .gap_3()
        .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child(t("account.syncScopeTitle")))
        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(t("account.syncScopeHint")))
        .child(list)
        .child(crate::theme::compose_button(t("account.startSync"), {
            cx.listener(|view: &mut SettingsView, e, w, cx| {
                view.start_initial_sync(e, w, cx);
            })
        }))
}

/// 通知所有存活窗口重渲染（全局主题/语言变更后跨窗口生效）。
fn refresh_all_windows(cx: &mut Context<SettingsView>) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| window.refresh());
    }
}

/// 主题行：Monitor 图标 + "主题" + 三段（浅色/深色/跟随系统）。
fn render_theme_row(view: &SettingsView, cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let mut seg = h_flex().gap_1().p(px(2.)).rounded(px(8.)).border_1().border_color(theme.border);
    for (key, pref, label, icon) in [
        ("pref-light", "light", t("settings.light"), IconName::Sun),
        ("pref-dark", "dark", t("settings.dark"), IconName::Moon),
        ("pref-system", "system", t("settings.system"), IconName::Monitor),
    ] {
        let sel = view.theme_pref == pref;
        seg = seg.child(
            div()
                .id(key)
                .flex()
                .items_center()
                .gap_1()
                .px(px(10.))
                .py(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.))
                .when(sel, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
                .when(!sel, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.bg(theme.accent.opacity(0.12)))
                })
                .child(gpui_kit::component::Icon::new(icon).size_3())
                .child(label)
                .on_click(cx.listener(move |v: &mut SettingsView, e, w, cx| {
                    v.set_theme_pref(pref, e, w, cx);
                })),
        );
    }
    h_flex()
        .items_center()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .flex_1()
                .child(gpui_kit::component::Icon::new(IconName::Monitor).size_4().text_color(theme.muted_foreground))
                .child(div().text_size(px(13.)).child(t("settings.theme"))),
        )
        .child(seg)
}

/// 语言行：Globe 图标 + "语言" + 两段（中文/English）。
fn render_language_row(cx: &mut Context<SettingsView>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let current = crate::i18n::current_lang();
    let mut seg = h_flex().gap_1().p(px(2.)).rounded(px(8.)).border_1().border_color(theme.border);
    for (key, lang, label) in [
        ("lang-zh", crate::i18n::Lang::Zh, "中文"),
        ("lang-en", crate::i18n::Lang::En, "English"),
    ] {
        let sel = current == lang;
        seg = seg.child(
            div()
                .id(key)
                .px(px(10.))
                .py(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_size(px(12.))
                .when(sel, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
                .when(!sel, |d| {
                    d.text_color(theme.muted_foreground)
                        .hover(|s| s.bg(theme.accent.opacity(0.12)))
                })
                .child(label)
                .on_click(cx.listener(move |v: &mut SettingsView, e, w, cx| {
                    v.set_lang(lang, e, w, cx);
                })),
        );
    }
    h_flex()
        .items_center()
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .flex_1()
                .child(gpui_kit::component::Icon::new(IconName::Globe).size_4().text_color(theme.muted_foreground))
                .child(div().text_size(px(13.)).child(t("settings.language"))),
        )
        .child(seg)
}