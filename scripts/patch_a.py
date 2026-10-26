# Batch A: 邮件列表工具条与加载按钮
import sys

p = 'crates/postium-ui/src/views/email_list.rs'
s = open(p, encoding='utf-8').read()

old_view = '''pub struct EmailListView {
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
}'''

new_view = '''pub struct EmailListView {
    pub state: Entity<ListState<EmailListDelegate>>,
    pub store: Entity<EmailsStore>,
    pub api: Arc<CoreApi>,
    older_syncing: bool,
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
                let api2 = api.clone();
                store.update(cx, |store, cx| {
                    store.refresh_now(&api2, cx);
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
        let rx = spawn_tokio(async move { api.get_email(id).await });
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(detail)) = rx.recv().await {
                let api2 = api.clone();
                let subject = detail.email.subject.clone().unwrap_or_default();
                let original_html = detail.body_html.clone().unwrap_or_default();
                let email_id = detail.email.id;
                cx.defer(move |cx| {
                    crate::views::compose::open_compose_window(
                        crate::views::compose::ComposeMode::Forward {
                            email_id,
                            subject,
                            original_html,
                        },
                        &api2,
                        cx,
                    );
                })
                .ok();
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
}'''

assert old_view in s, 'old view not found'
s = s.replace(old_view, new_view)

s = s.replace('''use gpui_kit::component::{
    ActiveTheme as _,
    IndexPath,
    Selectable,
    h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};''', '''use gpui_kit::component::{
    ActiveTheme as _,
    IndexPath,
    Sizable as _,
    Selectable,
    button::{Button, ButtonVariants as _},
    h_flex,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};''')
s = s.replace('''use crate::runtime::spawn_tokio;
use crate::stores::emails::EmailsStore;''', '''use crate::i18n::t;
use crate::nav::NavTarget;
use crate::runtime::spawn_tokio;
use crate::stores::emails::EmailsStore;''')
open(p, 'w', encoding='utf-8').write(s)
print('list view rewritten')
