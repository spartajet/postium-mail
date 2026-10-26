//! 邮件列表视图（移植自 src/lib/components/email/EmailList.svelte，
//! 虚拟化模式参照 spike 已验证实现）。
//!
//! 交互约定：行选中 = 打开该邮件（写 `EmailsStore.selected` 并异步标已读），
//! 宿主订阅 store 渲染详情。

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::{
    ActiveTheme as _,
    IndexPath,
    Selectable,
    h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;
use postium_core::service::email_service::EmailDto;

use crate::runtime::spawn_tokio;
use crate::stores::emails::EmailsStore;

#[derive(IntoElement)]
pub struct EmailRow {
    base: ListItem,
    email: Rc<EmailDto>,
    selected: bool,
}

impl Selectable for EmailRow {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.base = self.base.selected(selected);
        self
    }
    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for EmailRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let time = chrono::DateTime::from_timestamp(self.email.sent_at, 0)
            .map(|dt| {
                use chrono::Datelike;
                let now = chrono::Utc::now();
                if dt.date_naive() == now.date_naive() {
                    dt.format("%H:%M").to_string()
                } else {
                    dt.format("%m-%d").to_string()
                }
            })
            .unwrap_or_default();

        self.base.py_2().px_3().child(
            v_flex()
                .flex_1()
                .overflow_hidden()
                .gap_0p5()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .when(self.email.is_starred, |this| {
                            this.child(
                                gpui_kit::component::Icon::new(IconName::Star)
                                    .size_3()
                                    .text_color(gpui_kit::rgb(0xf59e0b)),
                            )
                        })
                        .when(self.email.has_attachments, |this| {
                            this.child(
                                gpui_kit::component::Icon::new(IconName::Paperclip).size_3(),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(cx.theme().muted_foreground)
                                .line_clamp(1)
                                .child(
                                    self.email
                                        .sender_name
                                        .clone()
                                        .unwrap_or_else(|| self.email.sender_email.clone()),
                                ),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_size(px(11.5))
                                .text_color(cx.theme().muted_foreground)
                                .child(time),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.5))
                        .text_color(cx.theme().foreground)
                        .when(!self.email.is_read, |this| {
                            this.font_weight(FontWeight::SEMIBOLD)
                        })
                        .line_clamp(1)
                        .child(
                            self.email
                                .subject
                                .clone()
                                .unwrap_or_else(|| "（无主题）".into()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .line_clamp(1)
                        .child(self.email.preview.clone().unwrap_or_default()),
                ),
        )
    }
}

pub struct EmailListDelegate {
    rows: Vec<Rc<EmailDto>>,
    selected_email: Option<i32>,
    store: gpui_kit::WeakEntity<EmailsStore>,
    api: Arc<postium_core::api::CoreApi>,
}

impl ListDelegate for EmailListDelegate {
    type Item = EmailRow;

    fn sections_count(&self, _: &App) -> usize {
        1
    }

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.rows.len()
    }

    fn perform_search(
        &mut self,
        _query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        // 搜索由顶栏搜索框 → EmailsStore::search 驱动，列表内禁用
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_email = ix.and_then(|i| self.rows.get(i.row)).map(|r| r.id);

        // 选中即打开：写回 store，宿主据此渲染详情
        if let Some(store) = self.store.upgrade() {
            let selected = self.selected_email;
            store.update(cx, |store, cx| {
                let changed = store.selected != selected;
                store.selected = selected;
                store.selected_version += 1;
                cx.notify();
                if changed
                    && let Some(id) = selected
                {
                    cx.emit(crate::stores::emails::EmailsEvent::SelectionChanged(id));
                }
            });

            // 异步标记已读
            if let Some((id, unread)) = ix.and_then(|i| self.rows.get(i.row)).map(|r| (r.id, !r.is_read)).filter(|(_, u)| *u) {
                let api = self.api.clone();
                let _ = spawn_tokio(async move { api.mark_as_read(id, true).await });
                let _ = window;
            }
        }
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let email = self.rows.get(ix.row)?;
        Some(EmailRow {
            base: ListItem::new(ix).selected(Some(email.id) == self.selected_email),
            email: email.clone(),
            selected: Some(email.id) == self.selected_email,
        })
    }
}

pub struct EmailListView {
    pub state: Entity<ListState<EmailListDelegate>>,
    pub store: Entity<EmailsStore>,
}

impl Render for EmailListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                h_flex().justify_between().items_center().child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} / {}", store.emails.len(), store.total)),
                ),
            )
            .child(List::new(&self.state).flex_1())
    }
}

/// 用 EmailsStore 数据重建列表状态。
pub fn rebuild_list_state(
    store: &Entity<EmailsStore>,
    api: &Arc<postium_core::api::CoreApi>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ListState<EmailListDelegate>> {
    let rows: Vec<Rc<EmailDto>> = store
        .read(cx)
        .emails
        .iter()
        .cloned()
        .map(Rc::new)
        .collect();
    let selected = store.read(cx).selected;
    let weak = store.downgrade();
    cx.new(|cx| {
        ListState::new(
            EmailListDelegate {
                rows,
                selected_email: selected,
                store: weak,
                api: api.clone(),
            },
            window,
            cx,
        )
    })
}
