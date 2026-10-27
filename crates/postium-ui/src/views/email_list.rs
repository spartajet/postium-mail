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
    Sizable as _,
    Selectable,
    button::{Button, ButtonVariants as _},
    h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit_assets::IconName;
use postium_core::api::CoreApi;
use postium_core::service::email_service::{EmailCategory, EmailDto};

use crate::i18n::t;
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::emails::EmailsStore;

#[derive(IntoElement)]
pub struct EmailRow {
    base: ListItem,
    email: Rc<EmailDto>,
    selected: bool,
    /// 右键菜单所需：核心 API 与列表 store（弱引用）
    api: Option<Arc<CoreApi>>,
    store: Option<gpui_kit::WeakEntity<EmailsStore>>,
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
        let menu_ctx = (self.api.clone(), self.store.clone());
        let email_rc = self.email.clone();
        let time = chrono::DateTime::from_timestamp(self.email.sent_at, 0)
            .map(|dt| {
                
                let now = chrono::Utc::now();
                if dt.date_naive() == now.date_naive() {
                    dt.format("%H:%M").to_string()
                } else {
                    dt.format("%m-%d").to_string()
                }
            })
            .unwrap_or_default();

        let row = self.base.py_2().px_3().child(
            v_flex()
                .flex_1()
                .overflow_hidden()
                .gap_1()
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
        );

        // 右键菜单（对齐旧版 EmailContextMenu；旧版"更多操作"为占位未复刻）
        match menu_ctx {
            (Some(api), Some(store)) => div()
                .child(row)
                .context_menu(move |menu, _, _| {
                    build_row_menu(menu, email_rc.clone(), api.clone(), store.clone())
                })
                .into_any_element(),
            _ => row.into_any_element(),
        }
    }
}

/// 行级操作类型。
enum RowAction {
    ToggleStar,
    MarkRead(bool),
    Delete,
}

/// 对指定邮件执行异步操作，完成后刷新列表 store。
fn act_on_email(
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
    id: i32,
    action: RowAction,
    cx: &mut App,
) {
    let api_keep = api.clone();
    let rx = spawn_tokio(async move {
        match action {
            RowAction::ToggleStar => api.toggle_star(id).await.map(|_| ()),
            RowAction::MarkRead(read) => api.mark_as_read(id, read).await,
            RowAction::Delete => api.delete_emails(vec![id]).await.map(|_| ()),
        }
    });
    cx.spawn(async move |cx| {
        if let Ok(Ok(())) = rx.recv().await
            && let Some(store) = store.upgrade()
        {
            let _ = cx.update(|cx: &mut App| {
                store.update(cx, |s, cx| s.refresh_now(&api_keep, cx));
            });
        }
    })
    .detach();
}

/// 回复种类：取详情后打开对应模式的写信窗口。
enum ReplyKind {
    Reply,
    ReplyAll,
    Forward,
}

/// 异步取邮件详情（正文引用/收件人），再打开写信窗口。
fn fetch_and_open_compose(api: Arc<CoreApi>, id: i32, kind: ReplyKind, cx: &mut App) {
    let api_keep = api.clone();
    let rx = spawn_tokio(async move { api.get_email(id).await });
    cx.spawn(async move |cx| {
        if let Ok(Ok(detail)) = rx.recv().await {
            let mode = match kind {
                ReplyKind::Reply => crate::views::compose::ComposeMode::Reply {
                    email_id: detail.email.id,
                    to: detail.email.sender_email.clone(),
                    subject: detail.email.subject.clone().unwrap_or_default(),
                    original_html: detail.body_html.clone().unwrap_or_default(),
                },
                ReplyKind::ReplyAll => {
                    let mut to = vec![detail.email.sender_email.clone()];
                    for r in detail.recipient_emails.split([',', ';']) {
                        let r = r.trim();
                        if !r.is_empty() && !to.iter().any(|x| x.eq_ignore_ascii_case(r)) {
                            to.push(r.to_string());
                        }
                    }
                    crate::views::compose::ComposeMode::ReplyAll {
                        email_id: detail.email.id,
                        to: to.join(", "),
                        subject: detail.email.subject.clone().unwrap_or_default(),
                        original_html: detail.body_html.clone().unwrap_or_default(),
                    }
                }
                ReplyKind::Forward => crate::views::compose::ComposeMode::Forward {
                    email_id: detail.email.id,
                    subject: detail.email.subject.clone().unwrap_or_default(),
                    original_html: detail.body_html.clone().unwrap_or_default(),
                },
            };
            let _ = cx.update(|cx: &mut App| {
                crate::views::compose::open_compose_window(mode, &api_keep, cx);
            });
        }
    })
    .detach();
}

/// 构造邮件行右键菜单：回复/全部回复/转发 ─ 星标/已读 ─ 删除。
fn build_row_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    email: Rc<EmailDto>,
    api: Arc<CoreApi>,
    store: gpui_kit::WeakEntity<EmailsStore>,
) -> gpui_kit::component::menu::PopupMenu {
    let id = email.id;
    let starred = email.is_starred;
    let unread = !email.is_read;

    let reply_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.reply").to_string()).icon(IconName::Reply).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::Reply, cx);
        })
    };
    let reply_all_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.replyAll").to_string()).icon(IconName::Reply).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::ReplyAll, cx);
        })
    };
    let forward_item = {
        let api = api.clone();
        PopupMenuItem::new(t("email.forward").to_string()).icon(IconName::Forward).on_click(move |_, _, cx| {
            fetch_and_open_compose(api.clone(), id, ReplyKind::Forward, cx);
        })
    };
    let star_item = {
        let (api, store) = (api.clone(), store.clone());
        PopupMenuItem::new(if starred { "取消星标".to_string() } else { "设为星标".to_string() })
            .icon(IconName::Star)
            .on_click(move |_, _, cx| {
                act_on_email(api.clone(), store.clone(), id, RowAction::ToggleStar, cx);
            })
    };
    let read_item = {
        let (api, store) = (api.clone(), store.clone());
        PopupMenuItem::new(if unread { "标为已读".to_string() } else { "标为未读".to_string() })
            .icon(IconName::MailOpen)
            .on_click(move |_, _, cx| {
                act_on_email(api.clone(), store.clone(), id, RowAction::MarkRead(!unread), cx);
            })
    };
    let delete_item = {
        PopupMenuItem::new(t("email.delete").to_string()).icon(IconName::Trash).on_click(move |_, _, cx| {
            act_on_email(api.clone(), store.clone(), id, RowAction::Delete, cx);
        })
    };

    menu
        .item(reply_item)
        .item(reply_all_item)
        .item(forward_item)
        .separator()
        .item(star_item)
        .item(read_item)
        .separator()
        .item(delete_item)
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
            if let Some((id, _unread)) = ix.and_then(|i| self.rows.get(i.row)).map(|r| (r.id, !r.is_read)).filter(|(_, u)| *u) {
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
            api: Some(self.api.clone()),
            store: Some(self.store.clone()),
        })
    }
}

pub struct EmailListView {
    pub state: Entity<ListState<EmailListDelegate>>,
    pub store: Entity<EmailsStore>,
    pub api: Arc<CoreApi>,
    pub older_syncing: bool,
}

enum SelectedAction {
    Star,
    ToggleRead(bool),
    Archive,
    Delete,
    Reload,
}

impl EmailListView {
    /// 对选中邮件执行操作并刷新列表。
    fn act_on_selected(&mut self, action: SelectedAction, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let api = self.api.clone();
        let api_keep = self.api.clone();
        let store = self.store.clone();
        let rx = spawn_tokio(async move {
            match action {
                SelectedAction::Star => api.toggle_star(id).await.map(|_| ()),
                SelectedAction::ToggleRead(read) => api.mark_as_read(id, read).await,
                SelectedAction::Archive => api.archive_email(id).await,
                SelectedAction::Delete => api.delete_emails(vec![id]).await.map(|_| ()),
                SelectedAction::Reload => api.reload_email(id).await.map(|_| ()),
            }
        });
        cx.spawn(async move |this, cx| {
            if let Ok(result) = rx.recv().await {
                let _ = result;
                store.update(cx, |store, cx| {
                    store.refresh_now(&api_keep, cx);
                });
                this.update(cx, |_, cx| cx.notify()).ok();
            }
        })
        .detach();
    }

    fn load_more(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| {
            store.load_more(&api, cx);
        });
        cx.notify();
    }

    /// 加载更早邮件（sync_older_emails，对当前范围账号）。
    fn sync_older(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.older_syncing {
            return;
        }
        self.older_syncing = true;
        cx.notify();
        let api = self.api.clone();
        let nav = self.store.read(cx).nav.clone();
        let rx = spawn_tokio(async move {
            match &nav {
                NavTarget::AccountFolder { account_id, .. } => {
                    api.sync_older_emails(*account_id, EmailCategory::Inbox).await.map(|_| ())
                }
                NavTarget::Category(c) => {
                    let accounts = api.list_accounts().await.unwrap_or_default();
                    for a in accounts {
                        let _ = api.sync_older_emails(a.id, c.clone()).await;
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        });
        let store = self.store.clone();
        let api2 = self.api.clone();
        cx.spawn(async move |this, cx| {
            if rx.recv().await.is_ok() {
                store.update(cx, |store, cx| {
                    store.refresh_now(&api2, cx);
                });
                this.update(cx, |view, cx| {
                    view.older_syncing = false;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_unread_filter(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let api = self.api.clone();
        self.store.update(cx, |store, cx| {
            store.unread_only = !store.unread_only;
            store.refresh_now(&api, cx);
        });
        cx.notify();
    }

    fn forward_selected(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.store.read(cx).selected else { return };
        let api = self.api.clone();
        let api2 = self.api.clone();
        let rx = spawn_tokio(async move { api.get_email(id).await });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(detail)) = rx.recv().await {
                let subject = detail.email.subject.clone().unwrap_or_default();
                let original_html = detail.body_html.clone().unwrap_or_default();
                let email_id = detail.email.id;
                cx.update(|cx| {
                    crate::views::compose::open_compose_window(
                        crate::views::compose::ComposeMode::Forward {
                            email_id,
                            subject,
                            original_html,
                        },
                        &api2,
                        cx,
                    );
                });
            }
        })
        .detach();
    }
}

impl Render for EmailListView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let has_selected = store.selected.is_some();
        let selected_unread = store
            .emails
            .iter()
            .find(|e| Some(e.id) == store.selected)
            .map(|e| !e.is_read)
            .unwrap_or(false);
        let can_more = (store.emails.len() as u64) < store.total;

        let mut toolbar = h_flex()
            .gap_1()
            .items_center()
            .flex_none()
            .child(
                Button::new("unread-filter")
                    .ghost()
                    .xsmall()
                    .label(if store.unread_only { "全部" } else { "未读" })
                    .tooltip("切换未读/全部")
                    .on_click(cx.listener(Self::toggle_unread_filter)),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} / {}", store.emails.len(), store.total)),
            );

        if has_selected {
            toolbar = toolbar
                .child(
                    Button::new("sel-star")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Star)
                        .tooltip("星标切换")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Star, cx);
                        })),
                )
                .child(
                    Button::new("sel-read")
                        .ghost()
                        .xsmall()
                        .label(if selected_unread { "标已读" } else { "标未读" })
                        .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::ToggleRead(!selected_unread), cx);
                        })),
                )
                .child(
                    Button::new("sel-reload")
                        .ghost()
                        .xsmall()
                        .icon(IconName::RotateCw)
                        .tooltip("重新加载")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Reload, cx);
                        })),
                )
                .child(
                    Button::new("sel-forward")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Forward)
                        .tooltip("转发")
                        .on_click(cx.listener(Self::forward_selected)),
                )
                .child(
                    Button::new("sel-archive")
                        .ghost()
                        .xsmall()
                        .label("归档")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Archive, cx);
                        })),
                )
                .child(
                    Button::new("sel-delete")
                        .ghost()
                        .xsmall()
                        .label("删除")
                        .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                            this.act_on_selected(SelectedAction::Delete, cx);
                        })),
                );
        }

        v_flex()
            .size_full()
            .p_2()
            .gap_2()
            .child(toolbar)
            .child(List::new(&self.state).flex_1())
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .justify_center()
                    .when(can_more, |this| {
                        this.child(
                            Button::new("load-more")
                                .ghost()
                                .xsmall()
                                .label(t("email.loadMore"))
                                .on_click(cx.listener(Self::load_more)),
                        )
                    })
                    .child(
                        Button::new("sync-older")
                            .ghost()
                            .xsmall()
                            .label(if self.older_syncing {
                                t("email.syncingOlder")
                            } else {
                                t("email.syncOlder")
                            })
                            .on_click(cx.listener(Self::sync_older)),
                    ),
            )
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

/// 供 e2e 驱动右键菜单动作的测试钩子（不经选中态，直连行级动作链路）。
#[cfg(test)]
pub(crate) mod test_hooks {
    use super::*;

    pub(crate) enum RowActionForTest {
        ToggleStar,
        MarkRead(bool),
        Delete,
    }

    pub(crate) fn act_on_email_for_test(
        api: Arc<CoreApi>,
        id: i32,
        action: RowActionForTest,
        cx: &mut App,
    ) {
        let mapped = match action {
            RowActionForTest::ToggleStar => RowAction::ToggleStar,
            RowActionForTest::MarkRead(read) => RowAction::MarkRead(read),
            RowActionForTest::Delete => RowAction::Delete,
        };
        // 悬空 store：DB 动作仍执行，仅跳过列表刷新
        act_on_email(api, gpui_kit::WeakEntity::new_invalid(), id, mapped, cx);
    }
}
