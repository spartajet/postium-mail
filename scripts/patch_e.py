# Batch E: Toast 总线 / 主题语言持久化 / 状态栏上次同步 / 关窗到托盘

# ---------- 新文件 notify.rs ----------
notify = r'''//! 全局 Toast 总线：任何上下文发消息，主窗口消费并弹 gpui-kit 通知。

use gpui_kit::component::WindowExt as _;
use gpui_kit::{AnyWindowHandle, App, SharedString};
use std::sync::OnceLock;

static MAIN_WINDOW: OnceLock<AnyWindowHandle> = OnceLock::new();
static TX: OnceLock<smol::channel::Sender<SharedString>> = OnceLock::new();

/// 记录主窗口句柄并启动消费循环（AppShell 装配时调用一次）。
pub fn setup(window: &gpui_kit::Window, cx: &mut App) {
    let _ = MAIN_WINDOW.set(window.window_handle());
    let (tx, mut rx) = smol::channel::bounded::<SharedString>(64);
    let _ = TX.set(tx);
    cx.spawn(async move |cx| {
        while let Ok(msg) = rx.recv().await {
            let Some(handle) = MAIN_WINDOW.get().copied() else { continue };
            cx.update(|cx| {
                let _ = handle.update(cx, |_, window, _| {
                    window.push_notification(msg.clone(), _);
                });
            });
        }
    })
    .detach();
}

/// 发送 Toast（线程安全，任意上下文可用）。
pub fn toast(msg: impl Into<SharedString>) {
    if let Some(tx) = TX.get() {
        let _ = tx.try_send(msg.into());
    }
}
'''
open('crates/postium-ui/src/notify.rs', 'w', encoding='utf-8').write(notify)

# ---------- ui_state.rs（主题/语言持久化） ----------
ui_state = r'''//! UI 偏好持久化（替代旧版 localStorage）：~/.postium/ui-state.json

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct UiState {
    pub theme: Option<String>, // "dark" | "light"
    pub lang: Option<String>,  // "zh" | "en"
}

fn path() -> std::path::PathBuf {
    crate::resolve_data_dir().join("ui-state.json")
}

pub fn load() -> UiState {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(state: &UiState) {
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(path(), json);
    }
}
'''
open('crates/postium-ui/src/ui_state.rs', 'w', encoding='utf-8').write(ui_state)

# ---------- main.rs 挂模块 ----------
p = 'crates/postium-ui/src/main.rs'
s = open(p, encoding='utf-8').read()
s = s.replace('mod nav;\nmod runtime;', 'mod nav;\nmod notify;\nmod runtime;\nmod ui_state;')
open(p, 'w', encoding='utf-8').write(s)

# ---------- app.rs：持久化 + 关窗隐藏 + notify setup ----------
p2 = 'crates/postium-ui/src/app.rs'
s2 = open(p2, encoding='utf-8').read()
s2 = s2.replace('''    fn new(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 默认暗色主题（与旧版默认一致）
        gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
''', '''    fn new(
        api: Arc<CoreApi>,
        sync_rx: SyncEventRx,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 恢复持久化偏好（默认暗色，与旧版一致）
        let ui = crate::ui_state::load();
        let theme_mode = match ui.theme.as_deref() {
            Some("light") => gpui_kit::component::ThemeMode::Light,
            _ => gpui_kit::component::ThemeMode::Dark,
        };
        gpui_kit::component::Theme::change(theme_mode, None, cx);
        if ui.lang.as_deref() == Some("en") {
            crate::i18n::set_lang(crate::i18n::Lang::En, cx);
        }

        // Toast 总线 + 关闭主窗口 → 隐藏到托盘（退出走托盘菜单）
        crate::notify::setup(window, cx);
        window.on_window_should_close(cx, |window, _| {
            window.hide();
            false
        });
''')
# toggle 持久化
s2 = s2.replace('''    fn toggle_lang(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<i18n::I18nGlobal>().lang;
        let next = if current == Lang::Zh { Lang::En } else { Lang::Zh };
        i18n::set_lang(next, cx);
    }''', '''    fn toggle_lang(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let current = cx.global::<i18n::I18nGlobal>().lang;
        let next = if current == Lang::Zh { Lang::En } else { Lang::Zh };
        i18n::set_lang(next, cx);
        let mut ui = crate::ui_state::load();
        ui.lang = Some(match next { Lang::Zh => "zh".into(), Lang::En => "en".into() });
        crate::ui_state::save(&ui);
    }''')
s2 = s2.replace('''        gpui_kit::component::Theme::change(
            if dark {
                gpui_kit::component::ThemeMode::Light
            } else {
                gpui_kit::component::ThemeMode::Dark
            },
            None,
            cx,
        );
    }''', '''        let mode = if dark {
            gpui_kit::component::ThemeMode::Light
        } else {
            gpui_kit::component::ThemeMode::Dark
        };
        gpui_kit::component::Theme::change(mode, None, cx);
        let mut ui = crate::ui_state::load();
        ui.theme = Some(if dark { "light".into() } else { "dark".into() });
        crate::ui_state::save(&ui);
    }''')
open(p2, 'w', encoding='utf-8').write(s2)

# ---------- status_bar.rs：上次同步时间 ----------
p3 = 'crates/postium-ui/src/views/status_bar.rs'
s3 = open(p3, encoding='utf-8').read()
s3 = s3.replace('''pub struct StatusBar {
    pub sync: Entity<SyncStore>,
    pub api: Arc<CoreApi>,
}''', '''pub struct StatusBar {
    pub sync: Entity<SyncStore>,
    pub api: Arc<CoreApi>,
    pub accounts: Entity<crate::stores::accounts::AccountsStore>,
}''')
s3 = s3.replace('''            .child(
                Button::new("sync-now")''', '''            .child(
                div()
                    .ml_auto()
                    .child(last_sync_text(self.accounts.read(cx))),
            )
            .child(
                Button::new("sync-now")''')
s3 += '''

fn last_sync_text(store: &crate::stores::accounts::AccountsStore) -> SharedString {
    let latest = store
        .groups
        .iter()
        .filter_map(|g| g.account.last_sync_at)
        .max();
    match latest {
        Some(ts) => {
            use chrono::Datelike;
            let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
            let now = chrono::Utc::now();
            let date = if dt.date_naive() == now.date_naive() {
                dt.format("%H:%M").to_string()
            } else {
                dt.format("%m-%d %H:%M").to_string()
            };
            format!("{} {}", t("sync.lastSync"), date).into()
        }
        None => format!("{} {}", t("sync.lastSync"), t("sync.never")).into(),
    }
}
'''
open(p3, 'w', encoding='utf-8').write(s3)

# app.rs StatusBar 构造加 accounts
s2b = open(p2, encoding='utf-8').read()
s2b = s2b.replace('''                    .child(cx.new(|_| StatusBar {
                        sync: self.sync_store.clone(),
                        api: self.api.clone(),
                    }).into_any_element()),''', '''                    .child(cx.new(|_| StatusBar {
                        sync: self.sync_store.clone(),
                        api: self.api.clone(),
                        accounts: self.accounts.clone(),
                    }).into_any_element()),''')
open(p2, 'w', encoding='utf-8').write(s2b)

# ---------- compose.rs：发送成功/失败 Toast ----------
p4 = 'crates/postium-ui/src/views/compose.rs'
s4 = open(p4, encoding='utf-8').read()
s4 = s4.replace('''                Ok(Ok(_resp)) => {
                    this.update(cx, |view, cx| {
                        view.status = "已发送 ✓".into();
                        view.close_window(cx);
                        cx.notify();
                    })
                    .ok();
                }''', '''                Ok(Ok(_resp)) => {
                    crate::notify::toast("邮件已发送 ✓");
                    this.update(cx, |view, cx| {
                        view.status = "已发送 ✓".into();
                        view.close_window(cx);
                        cx.notify();
                    })
                    .ok();
                }''')
open(p4, 'w', encoding='utf-8').write(s4)

print('batch E applied')
