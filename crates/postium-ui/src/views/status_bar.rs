//! 底部状态栏（移植自 src/lib/components/layout/StatusBar.svelte）。

use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
};
use gpui_kit::*;
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;

use crate::i18n::t;
use crate::stores::sync::SyncStore;

pub struct StatusBar {
    pub sync: Entity<SyncStore>,
    pub api: Arc<CoreApi>,
    pub accounts: Entity<crate::stores::accounts::AccountsStore>,
}

impl Render for StatusBar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.sync.read(cx);
        let status_text: SharedString = if store.syncing {
            let detail = store
                .progress
                .values()
                .next()
                .cloned()
                .unwrap_or_default();
            format!("{} {detail}", t("sync.syncing")).into()
        } else {
            t("sync.completed")
        };

        h_flex()
            .flex_none()
            .h(px(26.))
            .px_2()
            .gap_2()
            .items_center()
            .border_t_1()
            .border_color(cx.theme().border)
            .text_size(px(11.5))
            .text_color(cx.theme().muted_foreground)
            .child(
                gpui_kit::component::Icon::new(if store.syncing {
                    IconName::LoaderCircle
                } else {
                    IconName::CircleCheck
                })
                .size_3(),
            )
            .child(status_text)
            // 手动同步：仅图标按钮（状态文字已表达"同步完成"，避免文案重复）
            .child(
                Button::new("sync-now")
                    .ghost()
                    .xsmall()
                    .icon(IconName::RefreshCw)
                    .tooltip("立即同步全部账号")
                    .on_click({
                        let api = self.api.clone();
                        let sync = self.sync.clone();
                        move |_, _, cx| {
                            sync.update(cx, |store, cx| {
                                store.sync_all(&api, cx);
                            });
                        }
                    }),
            )
            // 右侧：语言/主题切换 + 上次同步时间
            .child(
                h_flex()
                    .ml_auto()
                    .gap_1()
                    .child(
                        Button::new("status-lang")
                            .ghost()
                            .xsmall()
                            .label("中/EN")
                            .on_click({
                                let this = cx.entity().downgrade();
                                move |_, _, cx| {
                                    crate::i18n::toggle_language(cx);
                                    refresh_after_pref(&this, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("status-theme")
                            .ghost()
                            .xsmall()
                            .label(t("settings.theme"))
                            .on_click({
                                let this = cx.entity().downgrade();
                                move |_, _, cx| {
                                    crate::theme::toggle_theme_mode(cx);
                                    refresh_after_pref(&this, cx);
                                }
                            }),
                    )
                    .child(div().child(last_sync_text(self.accounts.read(cx)))),
            )
    }
}

/// 偏好（语言/主题）切换后刷新主窗口树与状态栏自身。
/// 语言切换不经过任何实体事件，必须显式 notify 才能重绘新文案。
fn refresh_after_pref(this: &gpui_kit::WeakEntity<StatusBar>, cx: &mut App) {
    if let Some(main) = cx.try_global::<crate::app::MainShell>() {
        cx.notify(main.0.entity_id());
    }
    if let Some(this) = this.upgrade() {
        cx.notify(this.entity_id());
    }
}


fn last_sync_text(store: &crate::stores::accounts::AccountsStore) -> SharedString {
    let latest = store
        .groups
        .iter()
        .filter_map(|g| g.account.last_sync_at)
        .max();
    match latest {
        Some(ts) => {
            
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
